//! Bevy rendering of a [`nie_sim`] match.
//!
//! [`RenderPlugin`] decorates the simulation entities with meshes and materials, keeps their
//! transforms in step with the simulation, follows the action with a camera and shows a
//! scoreboard. It never writes to simulation state, so rendering cannot change a match result.

use bevy::prelude::*;
use nie_craft::{Art, ArtStyle, CameraStyle};
use nie_model::Side;
use nie_sim::{BallBody, Clock, Footballer, MatchConfig, PitchRes, PlayerBody, Score};

/// Ball radius in meters.
const BALL_RADIUS: f32 = 0.22;
const PLAYER_RADIUS: f32 = 0.35;
const PLAYER_BODY: f32 = 0.9;

pub struct RenderPlugin {
    pub art: Art,
}

#[derive(Resource, Clone)]
struct Look(Art);

#[derive(Component)]
struct Scoreboard;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Look(self.art.clone()))
            .insert_resource(ClearColor(Color::srgb(0.05, 0.07, 0.1)))
            .add_systems(Startup, (decorate_sim, build_stage, build_hud))
            .add_systems(Update, (sync_transforms, follow_camera, update_hud));
    }
}

/// Maps simulation coordinates (x, y, height) to Bevy world space (x, up, z).
pub fn to_world(x: f32, y: f32, height: f32) -> Vec3 {
    Vec3::new(x, height, y)
}

fn rgb(c: [u8; 3]) -> Color {
    Color::srgb_u8(c[0], c[1], c[2])
}

fn material(look: &Art, color: Color) -> StandardMaterial {
    match look.style {
        ArtStyle::Flat => StandardMaterial {
            base_color: color,
            unlit: true,
            ..default()
        },
        ArtStyle::Lowpoly => StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.95,
            ..default()
        },
        ArtStyle::Toon => StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.3,
            reflectance: 0.6,
            ..default()
        },
    }
}

/// Gives every simulation entity its visual: players as capsules in team colors, the ball as a
/// sphere (faceted in the low-poly style).
fn decorate_sim(
    mut commands: Commands,
    look: Res<Look>,
    cfg: Res<MatchConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    players: Query<(Entity, &Footballer, &PlayerBody)>,
    ball: Query<(Entity, &BallBody)>,
) {
    let capsule = meshes.add(Capsule3d::new(PLAYER_RADIUS, PLAYER_BODY));
    let kits = Side::BOTH.map(|side| {
        let colors = cfg.team(side).colors;
        let kit = materials.add(material(&look.0, rgb(colors.primary)));
        let keeper = materials.add(material(&look.0, rgb(colors.secondary)));
        (kit, keeper)
    });
    for (entity, footballer, body) in &players {
        let (kit, keeper) = &kits[usize::from(footballer.side == Side::Away)];
        let paint = if footballer.role == nie_model::Role::Goalkeeper {
            keeper
        } else {
            kit
        };
        commands.entity(entity).insert((
            Mesh3d(capsule.clone()),
            MeshMaterial3d(paint.clone()),
            Transform::from_translation(
                to_world(body.0.pos.x, body.0.pos.y, 0.0)
                    + Vec3::Y * (PLAYER_BODY * 0.5 + PLAYER_RADIUS),
            ),
        ));
    }

    let ball_mesh = match look.0.style {
        ArtStyle::Lowpoly => Sphere::new(BALL_RADIUS)
            .mesh()
            .ico(1)
            .unwrap_or_else(|_| Sphere::new(BALL_RADIUS).into()),
        _ => Sphere::new(BALL_RADIUS).into(),
    };
    let ball_mesh = meshes.add(ball_mesh);
    let ball_material = materials.add(material(&look.0, Color::WHITE));
    for (entity, body) in &ball {
        commands.entity(entity).insert((
            Mesh3d(ball_mesh.clone()),
            MeshMaterial3d(ball_material.clone()),
            Transform::from_translation(to_world(
                body.0.pos.x,
                body.0.pos.y,
                body.0.pos.z + BALL_RADIUS,
            )),
        ));
    }
}

/// Pitch, markings, goals, light and camera.
fn build_stage(
    mut commands: Commands,
    look: Res<Look>,
    pitch: Res<PitchRes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let p = pitch.0;
    let (length, width) = (p.half_length * 2.0, p.half_width * 2.0);

    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(length + 6.0, width + 6.0))),
        MeshMaterial3d(materials.add(material(&look.0, rgb(look.0.pitch_color)))),
    ));

    let chalk = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        ..default()
    });
    let mut bar = |size: Vec3, at: Vec3| {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(chalk.clone()),
            Transform::from_translation(at),
        ));
    };
    const LINE: f32 = 0.12;
    const LIFT: f32 = 0.01;
    bar(
        Vec3::new(length, LIFT, LINE),
        Vec3::new(0.0, LIFT, p.half_width),
    );
    bar(
        Vec3::new(length, LIFT, LINE),
        Vec3::new(0.0, LIFT, -p.half_width),
    );
    bar(
        Vec3::new(LINE, LIFT, width),
        Vec3::new(p.half_length, LIFT, 0.0),
    );
    bar(
        Vec3::new(LINE, LIFT, width),
        Vec3::new(-p.half_length, LIFT, 0.0),
    );
    bar(Vec3::new(LINE, LIFT, width), Vec3::new(0.0, LIFT, 0.0));
    let (post, bar_height) = (0.15, 2.2);
    for end in [-1.0_f32, 1.0] {
        let x = end * p.half_length;
        for side in [-1.0_f32, 1.0] {
            bar(
                Vec3::new(post, bar_height, post),
                Vec3::new(x, bar_height * 0.5, side * p.goal_half_width),
            );
        }
        bar(
            Vec3::new(post, post, p.goal_half_width * 2.0 + post),
            Vec3::new(x, bar_height, 0.0),
        );
    }

    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: look.0.style != ArtStyle::Flat,
            ..default()
        },
        Transform::default().looking_to(Vec3::new(-0.4, -1.0, -0.3), Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    });
    commands.spawn((
        Camera3d::default(),
        camera_pose(look.0.camera, 0.0, p.half_length),
    ));
}

fn camera_pose(style: CameraStyle, focus_x: f32, half_length: f32) -> Transform {
    match style {
        CameraStyle::Broadcast => Transform::from_xyz(focus_x, 15.0, 19.0)
            .looking_at(Vec3::new(focus_x, 0.0, 0.0), Vec3::Y),
        CameraStyle::TopDown => {
            Transform::from_xyz(0.0, half_length * 2.4, 0.01).looking_at(Vec3::ZERO, Vec3::NEG_Z)
        }
    }
}

type PlayerFilter = (With<Footballer>, Without<BallBody>);

fn sync_transforms(
    mut players: Query<(&PlayerBody, &mut Transform), PlayerFilter>,
    mut ball: Query<(&BallBody, &mut Transform), Without<Footballer>>,
) {
    for (body, mut transform) in &mut players {
        transform.translation = to_world(body.0.pos.x, body.0.pos.y, 0.0)
            + Vec3::Y * (PLAYER_BODY * 0.5 + PLAYER_RADIUS);
    }
    for (body, mut transform) in &mut ball {
        transform.translation = to_world(body.0.pos.x, body.0.pos.y, body.0.pos.z + BALL_RADIUS);
    }
}

fn follow_camera(
    look: Res<Look>,
    pitch: Res<PitchRes>,
    ball: Query<&BallBody>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let (Ok(ball), Ok(mut camera)) = (ball.single(), camera.single_mut()) else {
        return;
    };
    let focus = (ball.0.pos.x * 0.6).clamp(-pitch.0.half_length * 0.5, pitch.0.half_length * 0.5);
    let target = camera_pose(look.0.camera, focus, pitch.0.half_length);
    camera.translation = camera.translation.lerp(target.translation, 0.08);
    camera.rotation = camera.rotation.slerp(target.rotation, 0.08);
}

fn build_hud(mut commands: Commands) {
    commands.spawn((
        Scoreboard,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(30.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(10),
            left: px(14),
            ..default()
        },
    ));
}

fn update_hud(
    cfg: Res<MatchConfig>,
    score: Res<Score>,
    clock: Res<Clock>,
    mut text: Query<&mut Text, With<Scoreboard>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let seconds = clock.seconds() as u32;
    text.0 = format!(
        "{}  {} - {}  {}    {:02}:{:02}",
        cfg.home.name,
        score.home,
        score.away,
        cfg.away.name,
        seconds / 60,
        seconds % 60
    );
}
