//! `nie-game`: the native 3D frontend. Reads a craft manifest and plays it.
//!
//! The simulation advances on Bevy's fixed timestep at the manifest's tick rate; the keyboard
//! feeds the human-controlled team, the AI plays the rest.

use std::path::PathBuf;

use bevy::prelude::*;
use clap::Parser;
use nie_craft::{Controller, Manifest};
use nie_input::{InputCommand, KeyScheme};
use nie_model::Side;
use nie_render::RenderPlugin;
use nie_sim::{BallBody, Footballer, HumanInputs, MatchPlugin, PlayerBody, PlayerKey, SimTick};

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Craft manifest to play.
    #[arg(long, default_value = "games/pocket-derby.toml")]
    manifest: PathBuf,
    /// Play without a window and print the match report as JSON.
    #[arg(long)]
    headless: bool,
    /// Cap the headless run at this many ticks.
    #[arg(long)]
    ticks: Option<u32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let manifest = Manifest::load(&args.manifest)?;

    if args.headless {
        println!(
            "{}",
            serde_json::to_string_pretty(&manifest.play(args.ticks)?)?
        );
        return Ok(());
    }

    let config = manifest.match_config(0)?;
    let human = (manifest.teams.home.controller == Controller::Human).then_some(Side::Home);
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: manifest.game.name.clone(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(Time::<Fixed>::from_hz(f64::from(config.rules.tick_rate)))
    .add_plugins(MatchPlugin::new(config))
    .add_plugins(RenderPlugin {
        art: manifest.art.clone(),
    })
    .insert_resource(Keys(KeyScheme::wasd()))
    .add_systems(FixedUpdate, |world: &mut World| world.run_schedule(SimTick));
    if let Some(side) = human {
        app.insert_resource(Controlled(side)).add_systems(
            RunFixedMainLoop,
            read_keyboard.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
        );
    }
    app.run();
    Ok(())
}

#[derive(Resource)]
struct Keys(KeyScheme);

#[derive(Resource)]
struct Controlled(Side);

fn key(name: &str) -> Option<KeyCode> {
    Some(match name {
        "KeyW" => KeyCode::KeyW,
        "KeyA" => KeyCode::KeyA,
        "KeyS" => KeyCode::KeyS,
        "KeyD" => KeyCode::KeyD,
        "Space" => KeyCode::Space,
        "ArrowUp" => KeyCode::ArrowUp,
        "ArrowDown" => KeyCode::ArrowDown,
        "ArrowLeft" => KeyCode::ArrowLeft,
        "ArrowRight" => KeyCode::ArrowRight,
        "Enter" => KeyCode::Enter,
        _ => return None,
    })
}

/// Hands the keyboard to the human team's outfielder closest to the ball.
fn read_keyboard(
    keys: Res<Keys>,
    controlled: Res<Controlled>,
    pressed: Res<ButtonInput<KeyCode>>,
    ball: Query<&BallBody>,
    players: Query<(&Footballer, &PlayerBody)>,
    mut humans: ResMut<HumanInputs>,
) {
    humans.0.retain(|k, _| k.side != controlled.0);
    let Ok(ball) = ball.single() else { return };
    let nearest = players
        .iter()
        .filter(|(f, _)| f.side == controlled.0 && f.role != nie_model::Role::Goalkeeper)
        .min_by(|(fa, a), (fb, b)| {
            let (da, db) = (
                a.0.pos.distance_squared(ball.0.xy()),
                b.0.pos.distance_squared(ball.0.xy()),
            );
            da.total_cmp(&db).then(fa.number.cmp(&fb.number))
        });
    let Some((footballer, _)) = nearest else {
        return;
    };
    let held = |name: &str| key(name).is_some_and(|k| pressed.pressed(k));
    let s = &keys.0;
    let command = s.command(
        held(&s.up),
        held(&s.down),
        held(&s.left),
        held(&s.right),
        held(&s.kick),
    );
    if command != InputCommand::IDLE {
        humans.0.insert(
            PlayerKey {
                side: footballer.side,
                number: footballer.number,
            },
            command,
        );
    }
}
