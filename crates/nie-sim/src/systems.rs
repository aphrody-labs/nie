use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_math::Vec2;
use nie_ai::{Agent, PLAY_RANGE, Situation, decide};
use nie_input::InputCommand;
use nie_model::Side;
use nie_physics::{Body, GoalLine, kick_velocity, step_ball, step_body};

use crate::components::{BallBody, Command, Cooldown, Footballer, PlayerBody};
use crate::report::GoalEvent;
use crate::resources::{
    BallTuning, Clock, HumanInputs, MatchConfig, Phase, PitchRes, PlayerKey, Score, SimRng,
};

/// Ticks a player waits between kicks.
const KICK_COOLDOWN: u16 = 18;
/// Distance within which a moving player carries the ball.
const DRIBBLE_RANGE: f32 = 0.7;

#[derive(Resource, Default)]
pub struct GoalLog(pub Vec<GoalEvent>);

pub fn playing(phase: Res<Phase>) -> bool {
    *phase == Phase::Playing
}

pub fn unfinished(phase: Res<Phase>) -> bool {
    *phase != Phase::Finished
}

/// Chooses a command for every footballer: a human or remote input when present, else the AI.
pub fn think(
    pitch: Res<PitchRes>,
    humans: Res<HumanInputs>,
    mut rng: ResMut<SimRng>,
    ball: Query<&BallBody>,
    mut players: Query<(Entity, &Footballer, &PlayerBody, &mut Command)>,
) {
    let Ok(ball) = ball.single() else { return };
    let mut roster: Vec<(Entity, Footballer, Vec2)> = players
        .iter()
        .map(|(entity, f, body, _)| (entity, f.clone(), body.0.pos))
        .collect();
    roster.sort_by_key(|(_, f, _)| (f.side, f.number));

    let team_of = |side: Side| -> Vec<Agent> {
        roster
            .iter()
            .filter(|(_, f, _)| f.side == side)
            .map(|(_, f, pos)| Agent {
                number: f.number,
                role: f.role,
                pos: *pos,
            })
            .collect()
    };
    let teams = [team_of(Side::Home), team_of(Side::Away)];

    let mut decided = Vec::with_capacity(roster.len());
    for (entity, f, pos) in &roster {
        let key = PlayerKey {
            side: f.side,
            number: f.number,
        };
        let command = match humans.0.get(&key) {
            Some(command) => *command,
            None => {
                let team = &teams[usize::from(f.side == Side::Away)];
                decide(
                    &Situation {
                        pitch: pitch.0,
                        attack_dir: f.side.attack_dir(),
                        me: Agent {
                            number: f.number,
                            role: f.role,
                            pos: *pos,
                        },
                        attributes: f.attributes,
                        anchor: f.anchor,
                        team,
                        ball_pos: ball.0.pos,
                        ball_vel: ball.0.vel,
                    },
                    &mut rng.0,
                )
            }
        };
        decided.push((*entity, command));
    }
    for (entity, command) in decided {
        if let Ok((.., mut slot)) = players.get_mut(entity) {
            slot.0 = command;
        }
    }
}

pub fn movement(
    cfg: Res<MatchConfig>,
    pitch: Res<PitchRes>,
    mut players: Query<(&Footballer, &Command, &mut PlayerBody)>,
) {
    let dt = 1.0 / cfg.rules.tick_rate as f32;
    for (f, command, mut body) in &mut players {
        let [x, y] = command.0.movement();
        let speed = 3.5 + 4.5 * f32::from(f.attributes.pace) / 100.0;
        step_body(&mut body.0, Vec2::new(x, y), speed, 28.0, &pitch.0, dt);
    }
}

/// Kicks and dribbling. Players act in (side, number) order so overlapping claims on the ball
/// always resolve the same way.
pub fn kicks(
    mut ball: Query<&mut BallBody>,
    mut players: Query<(&Footballer, &PlayerBody, &Command, &mut Cooldown)>,
) {
    let Ok(mut ball) = ball.single_mut() else {
        return;
    };
    let mut order: Vec<_> = players
        .iter_mut()
        .map(|(f, body, command, cooldown)| {
            ((f.side, f.number), f.clone(), body.0, command.0, cooldown)
        })
        .collect();
    order.sort_by_key(|(key, ..)| *key);

    for (_, f, body, command, mut cooldown) in order {
        cooldown.0 = cooldown.0.saturating_sub(1);
        let state = &mut ball.0;
        if body.pos.distance(state.xy()) > PLAY_RANGE || state.pos.z >= 1.2 {
            continue;
        }
        let strength = command.kick_strength();
        if strength > 0.0 && cooldown.0 == 0 {
            let [ax, ay] = command.aim();
            let aim = if ax == 0.0 && ay == 0.0 {
                Vec2::new(f.side.attack_dir(), 0.0)
            } else {
                Vec2::new(ax, ay)
            };
            let speed = 6.0 + 18.0 * strength * (0.6 + 0.4 * f32::from(f.attributes.power) / 100.0);
            state.vel = kick_velocity(aim, speed, 0.1 + 0.15 * strength);
            cooldown.0 = KICK_COOLDOWN;
        } else if body.pos.distance(state.xy()) <= DRIBBLE_RANGE {
            let grip = 0.2 + 0.3 * f32::from(f.attributes.control) / 100.0;
            let target = body.vel * 1.15;
            state.vel.x += (target.x - state.vel.x) * grip;
            state.vel.y += (target.y - state.vel.y) * grip;
        }
    }
}

/// Match state that a goal updates.
#[derive(SystemParam)]
pub struct Referee<'w> {
    clock: Res<'w, Clock>,
    score: ResMut<'w, Score>,
    phase: ResMut<'w, Phase>,
    log: ResMut<'w, GoalLog>,
}

pub fn ball(
    cfg: Res<MatchConfig>,
    pitch: Res<PitchRes>,
    tuning: Res<BallTuning>,
    mut referee: Referee,
    mut ball: Query<&mut BallBody>,
) {
    let Ok(mut ball) = ball.single_mut() else {
        return;
    };
    let dt = 1.0 / cfg.rules.tick_rate as f32;
    if let Some(line) = step_ball(&mut ball.0, &pitch.0, &tuning.0, dt) {
        let side = match line {
            GoalLine::Positive => Side::Home,
            GoalLine::Negative => Side::Away,
        };
        referee.score.add(side);
        let tick = referee.clock.tick;
        referee.log.0.push(GoalEvent { tick, side });
        *referee.phase = Phase::GoalPause {
            remaining: cfg.rules.tick_rate,
        };
    }
}

/// Advances the clock, counts down goal pauses and restarts play with a kickoff.
pub fn clock(
    mut clock: ResMut<Clock>,
    mut phase: ResMut<Phase>,
    mut ball: Query<&mut BallBody>,
    mut players: Query<(&Footballer, &mut PlayerBody, &mut Command, &mut Cooldown)>,
) {
    clock.tick += 1;
    if let Phase::GoalPause { remaining } = *phase {
        if remaining <= 1 {
            for mut ball in &mut ball {
                ball.0 = nie_physics::BallState::at_rest(0.0, 0.0);
            }
            for (f, mut body, mut command, mut cooldown) in &mut players {
                body.0 = Body {
                    pos: f.kickoff,
                    vel: Vec2::ZERO,
                };
                command.0 = InputCommand::IDLE;
                cooldown.0 = 0;
            }
            *phase = Phase::Playing;
        } else {
            *phase = Phase::GoalPause {
                remaining: remaining - 1,
            };
        }
    }
    if clock.tick >= clock.total {
        *phase = Phase::Finished;
    }
}
