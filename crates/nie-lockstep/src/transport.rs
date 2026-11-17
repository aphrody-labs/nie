use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::Packet;

/// A reliable, ordered packet pipe to one peer.
pub trait Transport {
    fn send(&mut self, packet: Packet);
    fn recv(&mut self) -> Option<Packet>;
}

type Queue = Rc<RefCell<VecDeque<Packet>>>;

/// In-process transport: whatever one end sends, the other receives.
pub struct Loopback {
    outbox: Queue,
    inbox: Queue,
}

impl Loopback {
    pub fn pair() -> (Self, Self) {
        let (a, b): (Queue, Queue) = Default::default();
        (
            Self {
                outbox: a.clone(),
                inbox: b.clone(),
            },
            Self {
                outbox: b,
                inbox: a,
            },
        )
    }
}

impl Transport for Loopback {
    fn send(&mut self, packet: Packet) {
        self.outbox.borrow_mut().push_back(packet);
    }

    fn recv(&mut self) -> Option<Packet> {
        self.inbox.borrow_mut().pop_front()
    }
}
