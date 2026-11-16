//! Lockstep netcode for the deterministic nie simulation.
//!
//! Peers exchange only inputs. Every peer feeds the same [`InputCommand`]s to the same
//! simulation on the same tick, so the world stays identical without ever being sent. Peers
//! also swap a world checksum per tick to detect a desync as soon as it happens.
//!
//! The crate is transport agnostic: implement [`Transport`] over UDP, WebRTC or a relay. A
//! [`Loopback`] pair is provided for tests and local split screen.

mod lockstep;
mod packet;
mod transport;

pub use lockstep::{Lockstep, NetError};
pub use packet::{PROTOCOL_VERSION, Packet, PacketError};
pub use transport::{Loopback, Transport};
