//! Replays tests/parity.json (produced by the TypeScript implementation with
//! `bun crates/achillea-core/tests/gen-parity.ts`) against the Rust port.

use achillea_core::brackets::*;
use achillea_core::clans::*;
use achillea_core::elo::*;
use achillea_core::league::*;
use achillea_core::ranked::*;
use serde_json::Value;

fn fixtures() -> Value {
    serde_json::from_str(include_str!("parity.json")).expect("parity.json")
}

fn f(v: &Value) -> f64 {
    v.as_f64().expect("number")
}

fn u(v: &Value) -> u64 {
    v.as_u64().expect("unsigned")
}

fn s(v: &Value) -> String {
    v.as_str().expect("string").to_owned()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * a.abs().max(1.0)
}

#[test]
fn elo() {
    let d = fixtures();
    for c in d["expected"].as_array().unwrap() {
        assert!(
            close(expected_score(f(&c["p"]), f(&c["o"])), f(&c["r"])),
            "{c}"
        );
    }
    for c in d["delta"].as_array().unwrap() {
        assert_eq!(
            elo_delta(f(&c["p"]), f(&c["o"]), f(&c["a"]), f(&c["k"])),
            f(&c["r"]),
            "{c}"
        );
    }
    for c in d["progressive"].as_array().unwrap() {
        assert_eq!(
            progressive_elo_delta(f(&c["p"]), f(&c["o"]), f(&c["a"]), f(&c["t"]), f(&c["k"])),
            f(&c["r"]),
            "{c}"
        );
    }
    for c in d["match"].as_array().unwrap() {
        let r = compute_match_elo(f(&c["w"]), f(&c["l"]), f(&c["k"]));
        assert_eq!(
            (r.new_winner, r.new_loser, r.delta),
            (
                f(&c["r"]["newWinner"]),
                f(&c["r"]["newLoser"]),
                f(&c["r"]["delta"])
            ),
            "{c}"
        );
    }
    for c in d["prize"].as_array().unwrap() {
        assert_eq!(
            u64::from(placement_prize_ap(f(&c["s"]), f(&c["rk"]))),
            u(&c["r"]),
            "{c}"
        );
    }
}

#[test]
fn ranked() {
    let d = fixtures();
    for c in d["window"].as_array().unwrap() {
        assert_eq!(
            elo_window(f(&c["b"]) as i64, f(&c["w"]) as i64) as f64,
            f(&c["r"]),
            "{c}"
        );
    }
    for c in d["clamp"].as_array().unwrap() {
        assert_eq!(f64::from(clamp_score(f(&c["v"]))), f(&c["r"]), "{c}");
    }
    for c in d["mode"].as_array().unwrap() {
        assert_eq!(f64::from(parse_mode(f(&c["v"]))), f(&c["r"]), "{c}");
    }
    for c in d["queue"].as_array().unwrap() {
        let q: Vec<QueueCandidate> = c["q"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| QueueCandidate {
                user_id: s(&x["userId"]),
                elo: f(&x["elo"]) as i64,
                enqueued_at: f(&x["enqueuedAt"]) as i64,
            })
            .collect();
        let got = select_queue_pair(&q, f(&c["now"]) as i64, f(&c["base"]) as i64);
        let want = c["r"]
            .as_array()
            .map(|a| (u(&a[0]) as usize, u(&a[1]) as usize));
        assert_eq!(got, want, "{c}");
    }
    let side = |x: &Value| ScoreInput {
        user_id: s(&x["userId"]),
        my_score: u(&x["myScore"]) as u32,
        opp_score: u(&x["oppScore"]) as u32,
        submitted: x["submitted"].as_bool().unwrap(),
    };
    for c in d["scores"].as_array().unwrap() {
        let got = resolve_scores(
            &side(&c["home"]),
            &side(&c["away"]),
            c["forced"].as_bool().unwrap(),
        );
        let r = &c["r"];
        let want = match r["kind"].as_str().unwrap() {
            "wait" => ScoreOutcome::Wait,
            "agree" => ScoreOutcome::Agree {
                s1: u(&r["s1"]) as u32,
                s2: u(&r["s2"]) as u32,
            },
            "forfeit" => ScoreOutcome::Forfeit {
                s1: u(&r["s1"]) as u32,
                s2: u(&r["s2"]) as u32,
                forfeit_user_id: s(&r["forfeitUserId"]),
            },
            _ => ScoreOutcome::Dispute,
        };
        assert_eq!(got, want, "{c}");
    }
}

#[test]
fn brackets() {
    let d = fixtures();
    for c in d["brackets"].as_array().unwrap() {
        let n = u(&c["n"]);
        assert_eq!(bye_count(n), u(&c["bye"]), "{c}");
        assert_eq!(single_elimination_rounds(n), u(&c["se"]), "{c}");
        assert_eq!(double_elimination_rounds(n), u(&c["de"]), "{c}");
        assert_eq!(round_robin_rounds(n, false), u(&c["rr"]), "{c}");
        assert_eq!(round_robin_rounds(n, true), u(&c["rr2"]), "{c}");
        assert_eq!(
            stage_match_count(StageType::SingleElimination, n, false),
            u(&c["mc_se"]),
            "{c}"
        );
        assert_eq!(
            stage_match_count(StageType::DoubleElimination, n, false),
            u(&c["mc_de"]),
            "{c}"
        );
        assert_eq!(
            stage_match_count(StageType::RoundRobin, n, false),
            u(&c["mc_rr"]),
            "{c}"
        );
        assert_eq!(
            stage_match_count(StageType::RoundRobin, n, true),
            u(&c["mc_rr2"]),
            "{c}"
        );
    }
    for c in d["seeds"].as_array().unwrap() {
        let want: Vec<u64> = c["r"].as_array().unwrap().iter().map(u).collect();
        assert_eq!(standard_seed_order(u(&c["s"])), want, "{c}");
    }
}

#[test]
fn league() {
    let d = fixtures();
    for c in d["fixtures"].as_array().unwrap() {
        let ids: Vec<String> = c["ids"].as_array().unwrap().iter().map(s).collect();
        let want: Vec<(u64, String, String)> = c["r"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| (u(&x["matchday"]), s(&x["homeId"]), s(&x["awayId"])))
            .collect();
        let got: Vec<(u64, String, String)> = generate_single_round_robin(&ids)
            .into_iter()
            .map(|x| (u64::from(x.matchday), x.home_id, x.away_id))
            .collect();
        assert_eq!(got, want, "{c}");
    }
    for c in d["table"].as_array().unwrap() {
        let ids: Vec<String> = c["ids"].as_array().unwrap().iter().map(s).collect();
        let results: Vec<LeagueMatchResult> = c["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| LeagueMatchResult {
                home_id: s(&x["homeId"]),
                away_id: s(&x["awayId"]),
                home_score: f(&x["homeScore"]) as i32,
                away_score: f(&x["awayScore"]) as i32,
                winner_id: None,
                overtime: x["overtime"].as_bool().unwrap(),
            })
            .collect();
        let cfg = if c["cfg"] == "asytral" {
            ASYTRAL_POINTS
        } else {
            FOOTBALL_POINTS
        };
        let got: Vec<(String, usize, i32, i32, i32)> = compute_league_table(&ids, &results, cfg)
            .into_iter()
            .map(|r| (r.team_id, r.rank, r.points, r.map_diff, r.played))
            .collect();
        let want: Vec<(String, usize, i32, i32, i32)> = c["r"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| {
                (
                    s(&x["teamId"]),
                    u(&x["rank"]) as usize,
                    f(&x["points"]) as i32,
                    f(&x["mapDiff"]) as i32,
                    f(&x["played"]) as i32,
                )
            })
            .collect();
        assert_eq!(got, want, "{c}");
    }
}

#[test]
fn clans() {
    let d = fixtures();
    for c in d["distinction"].as_array().unwrap() {
        assert_eq!(
            u64::from(distinction_priority(&s(&c["d"]))),
            u(&c["r"]),
            "{c}"
        );
    }
    for c in d["clanrole"].as_array().unwrap() {
        let a = ClanMemberRole::from_i32(f(&c["a"]) as i32).unwrap();
        let b = ClanMemberRole::from_i32(f(&c["b"]) as i32).unwrap();
        assert_eq!(has_role_at_least(a, b), c["atLeast"].as_bool().unwrap());
        assert_eq!(can_act_on_member(a, b), c["act"].as_bool().unwrap());
        assert_eq!(can_manage_recruitment(a), c["recruit"].as_bool().unwrap());
        assert_eq!(can_dissolve_clan(a), c["dissolve"].as_bool().unwrap());
    }
}
