use nie_input::InputCommand;
use thiserror::Error;

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Packet {
    /// Opens a session. `manifest_hash` must match on both peers.
    Hello {
        protocol: u16,
        slot: u8,
        manifest_hash: u64,
    },
    /// Input of `slot` for simulation tick `tick`.
    Input {
        tick: u32,
        slot: u8,
        command: InputCommand,
    },
    /// Checksum of the world after `tick` was simulated.
    Checksum {
        tick: u32,
        value: u64,
    },
    Bye,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PacketError {
    #[error("empty packet")]
    Empty,
    #[error("unknown packet tag {0}")]
    UnknownTag(u8),
    #[error("packet truncated: {got} bytes, expected {want}")]
    Truncated { got: usize, want: usize },
}

impl Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16);
        match *self {
            Packet::Hello {
                protocol,
                slot,
                manifest_hash,
            } => {
                out.push(1);
                out.extend_from_slice(&protocol.to_le_bytes());
                out.push(slot);
                out.extend_from_slice(&manifest_hash.to_le_bytes());
            }
            Packet::Input {
                tick,
                slot,
                command,
            } => {
                out.push(2);
                out.extend_from_slice(&tick.to_le_bytes());
                out.push(slot);
                out.extend_from_slice(&command.to_bytes());
            }
            Packet::Checksum { tick, value } => {
                out.push(3);
                out.extend_from_slice(&tick.to_le_bytes());
                out.extend_from_slice(&value.to_le_bytes());
            }
            Packet::Bye => out.push(4),
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PacketError> {
        let (&tag, body) = bytes.split_first().ok_or(PacketError::Empty)?;
        let need = |want: usize| {
            if body.len() >= want {
                Ok(())
            } else {
                Err(PacketError::Truncated {
                    got: body.len() + 1,
                    want: want + 1,
                })
            }
        };
        match tag {
            1 => {
                need(11)?;
                Ok(Packet::Hello {
                    protocol: u16::from_le_bytes([body[0], body[1]]),
                    slot: body[2],
                    manifest_hash: u64::from_le_bytes(body[3..11].try_into().unwrap_or_default()),
                })
            }
            2 => {
                need(10)?;
                Ok(Packet::Input {
                    tick: u32::from_le_bytes([body[0], body[1], body[2], body[3]]),
                    slot: body[4],
                    command: InputCommand::from_bytes([
                        body[5], body[6], body[7], body[8], body[9],
                    ]),
                })
            }
            3 => {
                need(12)?;
                Ok(Packet::Checksum {
                    tick: u32::from_le_bytes([body[0], body[1], body[2], body[3]]),
                    value: u64::from_le_bytes(body[4..12].try_into().unwrap_or_default()),
                })
            }
            4 => Ok(Packet::Bye),
            other => Err(PacketError::UnknownTag(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_packet_round_trips() {
        let packets = [
            Packet::Hello {
                protocol: PROTOCOL_VERSION,
                slot: 1,
                manifest_hash: 0xDEAD_BEEF_0123_4567,
            },
            Packet::Input {
                tick: 99,
                slot: 0,
                command: InputCommand::from_analog([1.0, -0.5], [0.0, 1.0], 0.9),
            },
            Packet::Checksum {
                tick: 7,
                value: u64::MAX,
            },
            Packet::Bye,
        ];
        for p in packets {
            assert_eq!(Packet::decode(&p.encode()), Ok(p));
        }
    }

    #[test]
    fn bad_packets_are_errors() {
        assert_eq!(Packet::decode(&[]), Err(PacketError::Empty));
        assert_eq!(Packet::decode(&[9]), Err(PacketError::UnknownTag(9)));
        assert!(matches!(
            Packet::decode(&[2, 1]),
            Err(PacketError::Truncated { .. })
        ));
    }
}
