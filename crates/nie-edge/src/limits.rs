//! Per-family rate and concurrency limits on pingora-limits: a sliding-window [`Rate`] per client
//! and two [`Inflight`] counters (per client, whole edge) for every decode-heavy family.

use std::time::Duration;

use pingora_limits::inflight::{Guard, Inflight};
use pingora_limits::rate::Rate;

use crate::config::{Limits as LimitsConfig, Rule};
use crate::route::Limited;

/// Why a request was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denied {
    /// The client exceeded its request rate: `429`.
    Rate,
    /// Too many requests in flight, for this client or for the edge: `503` with `Retry-After`.
    Busy,
}

/// Holds the concurrency slots of one admitted request; dropping it frees them.
#[derive(Debug)]
pub struct Permit {
    _client: Guard,
    _global: Guard,
}

struct Family {
    rule: Rule,
    rate: Rate,
    client: Inflight,
    global: Inflight,
}

/// Limits of every family.
pub struct Limits {
    families: [Family; 4],
}

impl Limits {
    /// Build the counters from the configuration.
    #[must_use]
    pub fn new(config: &LimitsConfig) -> Self {
        let family = |f: Limited| Family {
            rule: config.rule(f),
            rate: Rate::new(Duration::from_secs(1)),
            client: Inflight::new(),
            global: Inflight::new(),
        };
        Self {
            families: Limited::ALL.map(family),
        }
    }

    /// Admit a request of `family` from `client`, or say why not.
    pub fn acquire(&self, family: Limited, client: &str) -> Result<Permit, Denied> {
        let f = &self.families[family.index()];
        if f.rule.rate_per_sec > 0 && f.rate.observe(&client, 1) > isize_of(f.rule.rate_per_sec) {
            return Err(Denied::Rate);
        }
        let (client_guard, client_now) = f.client.incr(client, 1);
        let (global_guard, global_now) = f.global.incr(family.name(), 1);
        let over = |limit: u32, now: isize| limit > 0 && now > isize_of(limit);
        if over(f.rule.client_inflight, client_now) || over(f.rule.global_inflight, global_now) {
            return Err(Denied::Busy);
        }
        Ok(Permit {
            _client: client_guard,
            _global: global_guard,
        })
    }
}

fn isize_of(n: u32) -> isize {
    isize::try_from(n).unwrap_or(isize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(rule: Rule) -> Limits {
        Limits::new(&LimitsConfig {
            model: rule,
            ..LimitsConfig::default()
        })
    }

    #[test]
    fn concurrency_is_per_client_and_global() {
        let l = limits(Rule {
            rate_per_sec: 0,
            client_inflight: 2,
            global_inflight: 3,
        });
        let a1 = l.acquire(Limited::Model, "a").unwrap();
        let _a2 = l.acquire(Limited::Model, "a").unwrap();
        assert_eq!(l.acquire(Limited::Model, "a").unwrap_err(), Denied::Busy);
        let _b1 = l.acquire(Limited::Model, "b").unwrap();
        assert_eq!(l.acquire(Limited::Model, "c").unwrap_err(), Denied::Busy);
        drop(a1);
        l.acquire(Limited::Model, "c").unwrap();
        // another family is untouched
        l.acquire(Limited::Export, "a").unwrap();
    }

    #[test]
    fn rate_limit_answers_429_class() {
        let l = limits(Rule {
            rate_per_sec: 3,
            client_inflight: 0,
            global_inflight: 0,
        });
        for _ in 0..3 {
            drop(l.acquire(Limited::Model, "a").unwrap());
        }
        assert_eq!(l.acquire(Limited::Model, "a").unwrap_err(), Denied::Rate);
        l.acquire(Limited::Model, "b").unwrap();
    }
}
