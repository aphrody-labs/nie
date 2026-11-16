use std::collections::BTreeMap;

use nie_input::InputCommand;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NetError {
    #[error("slot {0} is out of range")]
    Slot(u8),
    #[error("input for tick {tick} arrived after tick {next} was already simulated")]
    Stale { tick: u32, next: u32 },
    #[error("desync at tick {tick}: local checksum {local:#018x}, remote {remote:#018x}")]
    Desync { tick: u32, local: u64, remote: u64 },
}

/// Input buffer of a lockstep session with a fixed input delay.
///
/// A tick is released by [`Lockstep::poll`] only once every slot has an input for it, which is
/// what keeps all peers in step.
pub struct Lockstep {
    slots: u8,
    delay: u32,
    next_tick: u32,
    frames: BTreeMap<u32, Vec<Option<InputCommand>>>,
    local_sums: BTreeMap<u32, u64>,
    remote_sums: BTreeMap<u32, u64>,
}

impl Lockstep {
    /// A session whose first `delay` ticks run on idle input, so nobody waits at kickoff.
    pub fn new(slots: u8, delay: u32) -> Self {
        let mut session = Self {
            slots,
            delay,
            next_tick: 0,
            frames: BTreeMap::new(),
            local_sums: BTreeMap::new(),
            remote_sums: BTreeMap::new(),
        };
        for tick in 0..delay {
            for slot in 0..slots {
                session
                    .frames
                    .entry(tick)
                    .or_insert_with(|| vec![None; usize::from(slots)])[usize::from(slot)] =
                    Some(InputCommand::IDLE);
            }
        }
        session
    }

    /// The tick a local input sampled now should be stamped with.
    pub fn target_tick(&self) -> u32 {
        self.next_tick + self.delay
    }

    pub fn submit(&mut self, tick: u32, slot: u8, command: InputCommand) -> Result<(), NetError> {
        if slot >= self.slots {
            return Err(NetError::Slot(slot));
        }
        if tick < self.next_tick {
            return Err(NetError::Stale {
                tick,
                next: self.next_tick,
            });
        }
        self.frames
            .entry(tick)
            .or_insert_with(|| vec![None; usize::from(self.slots)])[usize::from(slot)] =
            Some(command);
        Ok(())
    }

    /// Releases the next tick and its inputs, ordered by slot, when all of them are known.
    pub fn poll(&mut self) -> Option<(u32, Vec<InputCommand>)> {
        let frame = self.frames.get(&self.next_tick)?;
        if frame.iter().any(Option::is_none) {
            return None;
        }
        let commands = frame.iter().flatten().copied().collect();
        self.frames.remove(&self.next_tick);
        let tick = self.next_tick;
        self.next_tick += 1;
        Some((tick, commands))
    }

    pub fn record_local(&mut self, tick: u32, checksum: u64) -> Result<(), NetError> {
        self.local_sums.insert(tick, checksum);
        self.compare(tick)
    }

    pub fn record_remote(&mut self, tick: u32, checksum: u64) -> Result<(), NetError> {
        self.remote_sums.insert(tick, checksum);
        self.compare(tick)
    }

    fn compare(&mut self, tick: u32) -> Result<(), NetError> {
        if let (Some(&local), Some(&remote)) =
            (self.local_sums.get(&tick), self.remote_sums.get(&tick))
        {
            self.local_sums.remove(&tick);
            self.remote_sums.remove(&tick);
            if local != remote {
                return Err(NetError::Desync {
                    tick,
                    local,
                    remote,
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Loopback, Packet, Transport};

    fn scripted(tick: u32, slot: u8) -> InputCommand {
        InputCommand::from_analog(
            [((tick + u32::from(slot)) % 3) as f32 - 1.0, 0.0],
            [0.0, 0.0],
            if tick.is_multiple_of(7) { 1.0 } else { 0.0 },
        )
    }

    #[test]
    fn waits_for_every_slot() {
        let mut s = Lockstep::new(2, 0);
        s.submit(0, 0, InputCommand::IDLE).unwrap();
        assert!(s.poll().is_none());
        s.submit(0, 1, InputCommand::IDLE).unwrap();
        assert_eq!(s.poll().map(|(t, c)| (t, c.len())), Some((0, 2)));
    }

    #[test]
    fn rejects_bad_slot_and_stale_tick() {
        let mut s = Lockstep::new(2, 0);
        assert_eq!(s.submit(0, 2, InputCommand::IDLE), Err(NetError::Slot(2)));
        s.submit(0, 0, InputCommand::IDLE).unwrap();
        s.submit(0, 1, InputCommand::IDLE).unwrap();
        s.poll().unwrap();
        assert!(matches!(
            s.submit(0, 0, InputCommand::IDLE),
            Err(NetError::Stale { .. })
        ));
    }

    #[test]
    fn detects_desync() {
        let mut s = Lockstep::new(2, 0);
        s.record_local(3, 10).unwrap();
        assert!(matches!(
            s.record_remote(3, 11),
            Err(NetError::Desync { tick: 3, .. })
        ));
    }

    /// Two peers over a loopback pipe release identical frames on identical ticks.
    #[test]
    fn two_peers_stay_in_lockstep() {
        let (mut wire_a, mut wire_b) = Loopback::pair();
        let mut a = Lockstep::new(2, 2);
        let mut b = Lockstep::new(2, 2);
        let mut seen_a = Vec::new();
        let mut seen_b = Vec::new();

        for frame in 0..120 {
            for (session, wire, slot) in [(&mut a, &mut wire_a, 0u8), (&mut b, &mut wire_b, 1u8)] {
                let tick = session.target_tick();
                let command = scripted(tick, slot);
                session.submit(tick, slot, command).unwrap();
                wire.send(Packet::Input {
                    tick,
                    slot,
                    command,
                });
            }
            for (session, wire) in [(&mut a, &mut wire_a), (&mut b, &mut wire_b)] {
                while let Some(Packet::Input {
                    tick,
                    slot,
                    command,
                }) = wire.recv()
                {
                    session.submit(tick, slot, command).unwrap();
                }
            }
            if let Some(frame) = a.poll() {
                seen_a.push(frame);
            }
            if let Some(frame) = b.poll() {
                seen_b.push(frame);
            }
            assert_eq!(seen_a.len(), seen_b.len(), "frame {frame}");
        }
        assert_eq!(seen_a, seen_b);
        assert!(seen_a.len() >= 100);
    }
}
