use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_math::Vec2;
use nie_model::{DetRng, Side};
use nie_physics::{BallParams, BallState, Body, Pitch};

use crate::components::{BallBody, Command, Cooldown, Footballer, PlayerBody};
use crate::report::MatchReport;
use crate::resources::{
    BallTuning, Clock, HumanInputs, MatchConfig, Phase, PitchRes, Score, SimRng,
};
use crate::systems;

/// One fixed simulation step. Run it once per tick, from any driver.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SimTick;

pub struct MatchPlugin {
    config: MatchConfig,
}

impl MatchPlugin {
    pub fn new(config: MatchConfig) -> Self {
        Self { config }
    }
}

impl Plugin for MatchPlugin {
    fn build(&self, app: &mut App) {
        let rules = &self.config.rules;
        let pitch = Pitch::new(rules.pitch_length, rules.pitch_width, rules.goal_width);
        app.insert_resource(self.config.clone())
            .insert_resource(PitchRes(pitch))
            .insert_resource(BallTuning(BallParams::default()))
            .insert_resource(Score::default())
            .insert_resource(Clock {
                tick: 0,
                total: rules.total_ticks(),
                rate: rules.tick_rate,
            })
            .insert_resource(Phase::Playing)
            .insert_resource(SimRng(DetRng::new(rules.seed)))
            .init_resource::<HumanInputs>()
            .init_resource::<crate::systems::GoalLog>();

        app.world_mut()
            .spawn(BallBody(BallState::at_rest(0.0, 0.0)));
        for side in Side::BOTH {
            for (slot, player) in self
                .config
                .team(side)
                .formation
                .slots()
                .into_iter()
                .zip(&self.config.team(side).players)
            {
                let dir = side.attack_dir();
                let lateral = dir * slot.y * pitch.half_width * 0.85;
                let anchor = Vec2::new(
                    dir * (slot.x * rules.pitch_length - pitch.half_length),
                    lateral,
                );
                let kickoff = Vec2::new(
                    dir * (slot.x * pitch.half_length - pitch.half_length),
                    lateral,
                );
                app.world_mut().spawn((
                    Footballer {
                        side,
                        number: player.number,
                        name: player.name.clone(),
                        role: player.role,
                        attributes: player.attributes,
                        anchor,
                        kickoff,
                    },
                    PlayerBody(Body {
                        pos: kickoff,
                        vel: Vec2::ZERO,
                    }),
                    Command::default(),
                    Cooldown::default(),
                ));
            }
        }

        app.add_systems(
            SimTick,
            (
                systems::think.run_if(systems::playing),
                systems::movement.run_if(systems::playing),
                systems::kicks.run_if(systems::playing),
                systems::ball.run_if(systems::playing),
                systems::clock.run_if(systems::unfinished),
            )
                .chain(),
        );
    }
}

/// Plays a whole match without a window or a renderer. `ticks` caps the run, `None` plays the
/// regulation length.
pub fn run_headless(config: MatchConfig, ticks: Option<u32>) -> MatchReport {
    let total = ticks.unwrap_or(config.rules.total_ticks());
    let mut app = App::new();
    app.add_plugins(MatchPlugin::new(config));
    for _ in 0..total {
        app.world_mut().run_schedule(SimTick);
    }
    MatchReport::capture(app.world_mut())
}
