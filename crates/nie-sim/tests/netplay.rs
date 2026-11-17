//! Two peers play the same match over a lockstep session that exchanges only inputs. If the
//! simulation were not deterministic the checksums would diverge within a few ticks.

use bevy_app::App;
use nie_input::InputCommand;
use nie_lockstep::{Lockstep, Loopback, Packet, Transport};
use nie_model::{Rules, Side};
use nie_sim::{HumanInputs, MatchConfig, MatchPlugin, MatchReport, PlayerKey, SimTick, checksum};

struct Peer {
    app: App,
    session: Lockstep,
    wire: Loopback,
    slot: u8,
    keys: [PlayerKey; 2],
}

impl Peer {
    fn new(config: &MatchConfig, wire: Loopback, slot: u8) -> Self {
        let mut app = App::new();
        app.add_plugins(MatchPlugin::new(config.clone()));
        let key = |side: Side| PlayerKey {
            side,
            number: config.team(side).players[1].number,
        };
        Self {
            app,
            session: Lockstep::new(2, 3),
            wire,
            slot,
            keys: [key(Side::Home), key(Side::Away)],
        }
    }
}

/// What the human on `slot` does at `tick`: run towards the ball's side and kick now and then.
fn scripted(tick: u32, slot: u8) -> InputCommand {
    let dir = if slot == 0 { 1.0 } else { -1.0 };
    let wobble = ((tick / 20 + u32::from(slot)) % 3) as f32 - 1.0;
    InputCommand::from_analog(
        [dir, wobble * 0.5],
        [dir, 0.0],
        if tick.is_multiple_of(25) { 1.0 } else { 0.0 },
    )
}

#[test]
fn lockstep_peers_never_desync() {
    let config = MatchConfig::generated(Rules {
        seed: 11,
        ..Rules::default()
    });
    let (wire_a, wire_b) = Loopback::pair();
    let mut peers = [Peer::new(&config, wire_a, 0), Peer::new(&config, wire_b, 1)];

    for _ in 0..900 {
        for peer in &mut peers {
            let tick = peer.session.target_tick();
            let command = scripted(tick, peer.slot);
            peer.session.submit(tick, peer.slot, command).unwrap();
            peer.wire.send(Packet::Input {
                tick,
                slot: peer.slot,
                command,
            });
        }
        for peer in &mut peers {
            while let Some(packet) = peer.wire.recv() {
                match packet {
                    Packet::Input {
                        tick,
                        slot,
                        command,
                    } => {
                        peer.session.submit(tick, slot, command).unwrap();
                    }
                    Packet::Checksum { tick, value } => {
                        peer.session.record_remote(tick, value).unwrap()
                    }
                    other => panic!("unexpected {other:?}"),
                }
            }
        }
        for peer in &mut peers {
            let Some((tick, commands)) = peer.session.poll() else {
                continue;
            };
            let mut humans = peer.app.world_mut().resource_mut::<HumanInputs>();
            humans.0.clear();
            for (key, command) in peer.keys.iter().zip(commands) {
                humans.0.insert(*key, command);
            }
            peer.app.world_mut().run_schedule(SimTick);
            let value = checksum(peer.app.world_mut());
            peer.session.record_local(tick, value).unwrap();
            peer.wire.send(Packet::Checksum { tick, value });
        }
    }

    let [a, b] = &mut peers;
    let (ra, rb) = (
        MatchReport::capture(a.app.world_mut()),
        MatchReport::capture(b.app.world_mut()),
    );
    assert_eq!(ra, rb);
    assert!(ra.ticks >= 890, "peers advanced {} ticks", ra.ticks);
}
