use bevy_ecs::prelude::*;
use nie_model::Side;
use serde::Serialize;

use crate::components::{BallBody, Footballer, PlayerBody};
use crate::resources::{Clock, Phase, Score};
use crate::systems::GoalLog;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct GoalEvent {
    pub tick: u32,
    pub side: Side,
}

/// Outcome of a (partial) match.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MatchReport {
    pub home: u16,
    pub away: u16,
    pub ticks: u32,
    pub finished: bool,
    pub goals: Vec<GoalEvent>,
    /// FNV-1a over the whole world state; identical runs produce identical values.
    pub checksum: u64,
}

impl MatchReport {
    pub fn capture(world: &mut World) -> Self {
        let score = *world.resource::<Score>();
        Self {
            home: score.home,
            away: score.away,
            ticks: world.resource::<Clock>().tick,
            finished: *world.resource::<Phase>() == Phase::Finished,
            goals: world.resource::<GoalLog>().0.clone(),
            checksum: checksum(world),
        }
    }

    pub fn score_line(&self) -> String {
        format!("{}-{}", self.home, self.away)
    }
}

/// FNV-1a 64 over the tick, the score, the ball and every footballer in (side, number) order.
pub fn checksum(world: &mut World) -> u64 {
    let mut hash = Fnv::new();
    hash.u32(world.resource::<Clock>().tick);
    let score = *world.resource::<Score>();
    hash.u32(u32::from(score.home) << 16 | u32::from(score.away));
    if let Some(ball) = world.query::<&BallBody>().iter(world).next() {
        for v in [ball.0.pos.to_array(), ball.0.vel.to_array()].concat() {
            hash.u32(v.to_bits());
        }
    }
    let mut players: Vec<_> = world
        .query::<(&Footballer, &PlayerBody)>()
        .iter(world)
        .map(|(f, b)| ((f.side, f.number), b.0))
        .collect();
    players.sort_by_key(|(key, _)| *key);
    for (_, body) in players {
        for v in [body.pos.x, body.pos.y, body.vel.x, body.vel.y] {
            hash.u32(v.to_bits());
        }
    }
    hash.0
}

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Self(0xCBF2_9CE4_8422_2325)
    }

    fn u32(&mut self, v: u32) {
        for byte in v.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
}
