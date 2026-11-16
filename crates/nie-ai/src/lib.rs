//! Role based football AI.
//!
//! [`decide`] is a pure function of a [`Situation`] and a random stream: no hidden state, so a
//! match replays exactly and the same code can drive a remote or a human-assist controller.

use bevy_math::{Vec2, Vec3};
use nie_input::InputCommand;
use nie_model::{Attributes, DetRng, Role};
use nie_physics::Pitch;

/// How close a player must be to the ball to play it.
pub const PLAY_RANGE: f32 = 1.1;
/// Shooting distance from the opposing goal.
const SHOOT_RANGE: f32 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Agent {
    pub number: u8,
    pub role: Role,
    pub pos: Vec2,
}

/// Everything one player perceives on one tick.
#[derive(Clone, Copy, Debug)]
pub struct Situation<'a> {
    pub pitch: Pitch,
    /// +1 when the team attacks towards +x, -1 otherwise.
    pub attack_dir: f32,
    pub me: Agent,
    pub attributes: Attributes,
    /// World position of the player's formation slot.
    pub anchor: Vec2,
    /// The whole team, the player included.
    pub team: &'a [Agent],
    pub ball_pos: Vec3,
    pub ball_vel: Vec3,
}

pub fn decide(s: &Situation, rng: &mut DetRng) -> InputCommand {
    let ball = s.ball_pos.truncate();
    match s.me.role {
        Role::Goalkeeper => goalkeeper(s, ball, rng),
        _ if is_chaser(s, ball) => chase(s, ball, rng),
        _ => hold(s, ball),
    }
}

fn is_chaser(s: &Situation, ball: Vec2) -> bool {
    let key = |a: &Agent| (a.pos.distance_squared(ball), a.number);
    let mine = key(&s.me);
    s.team
        .iter()
        .filter(|a| a.role != Role::Goalkeeper)
        .all(|a| key(a) >= mine)
}

fn chase(s: &Situation, ball: Vec2, rng: &mut DetRng) -> InputCommand {
    if s.me.pos.distance(ball) <= PLAY_RANGE && s.ball_pos.z < 1.2 {
        let goal = Vec2::new(s.attack_dir * s.pitch.half_length, 0.0);
        let to_goal = s.me.pos.distance(goal);
        let inaccuracy = f32::from(100 - s.attributes.control.min(100)) * 0.05;
        let corner = rng.signed() * s.pitch.goal_half_width * 0.8;
        let target = Vec2::new(goal.x, corner + rng.signed() * inaccuracy);
        let strength = if to_goal < SHOOT_RANGE { 1.0 } else { 0.5 };
        return command(seek(s.me.pos, ball, 0.3), unit(target - s.me.pos), strength);
    }
    let lead = ball + s.ball_vel.truncate() * 0.25;
    command(seek(s.me.pos, lead, 0.2), Vec2::ZERO, 0.0)
}

fn hold(s: &Situation, ball: Vec2) -> InputCommand {
    let shift = Vec2::new(
        ((ball.x - s.anchor.x) * 0.3).clamp(-6.0, 6.0),
        (ball.y - s.anchor.y) * 0.2,
    );
    command(
        seek(s.me.pos, s.pitch.clamp(s.anchor + shift), 0.4),
        Vec2::ZERO,
        0.0,
    )
}

fn goalkeeper(s: &Situation, ball: Vec2, rng: &mut DetRng) -> InputCommand {
    if s.me.pos.distance(ball) <= PLAY_RANGE && s.ball_pos.z < 1.5 {
        let aim = Vec2::new(s.attack_dir, rng.signed() * 0.6);
        return command(Vec2::ZERO, unit(aim), 0.8);
    }
    let line_x = -s.attack_dir * (s.pitch.half_length - 1.2);
    let reach = s.pitch.goal_half_width * 0.9;
    let target = Vec2::new(line_x, (ball.y * 0.7).clamp(-reach, reach));
    command(seek(s.me.pos, target, 0.15), Vec2::ZERO, 0.0)
}

/// Movement wish towards `target`, easing off inside two meters.
fn seek(from: Vec2, target: Vec2, dead_zone: f32) -> Vec2 {
    let delta = target - from;
    let len = delta.length();
    if len <= dead_zone {
        Vec2::ZERO
    } else {
        delta * (len.min(2.0) / (2.0 * len))
    }
}

fn unit(v: Vec2) -> Vec2 {
    let len = v.length();
    if len == 0.0 { Vec2::ZERO } else { v / len }
}

fn command(wish: Vec2, aim: Vec2, kick: f32) -> InputCommand {
    InputCommand::from_analog([wish.x, wish.y], [aim.x, aim.y], kick)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nie_model::{Formation, generate_team};

    fn situation<'a>(team: &'a [Agent], me: usize, ball: Vec3) -> Situation<'a> {
        let t = generate_team(3, &Formation::balanced(team.len() as u8));
        Situation {
            pitch: Pitch::new(40.0, 24.0, 6.0),
            attack_dir: 1.0,
            me: team[me],
            attributes: t.players[me].attributes,
            anchor: team[me].pos,
            team,
            ball_pos: ball,
            ball_vel: Vec3::ZERO,
        }
    }

    fn team() -> Vec<Agent> {
        let spots = [
            (-18.0, 0.0),
            (-10.0, -4.0),
            (-10.0, 4.0),
            (-2.0, 0.0),
            (4.0, 0.0),
        ];
        let roles = [
            Role::Goalkeeper,
            Role::Defender,
            Role::Defender,
            Role::Midfielder,
            Role::Forward,
        ];
        spots
            .iter()
            .zip(roles)
            .enumerate()
            .map(|(i, (&(x, y), role))| Agent {
                number: i as u8 + 1,
                role,
                pos: Vec2::new(x, y),
            })
            .collect()
    }

    #[test]
    fn nearest_outfielder_chases_others_hold() {
        let team = team();
        let ball = Vec3::new(6.0, 0.0, 0.0);
        let mut rng = DetRng::new(1);
        let chaser = decide(&situation(&team, 4, ball), &mut rng);
        assert!(chaser.move_x > 0);
        let back = decide(&situation(&team, 1, ball), &mut rng);
        assert_eq!(back.kick, 0);
    }

    #[test]
    fn player_on_the_ball_shoots_forward() {
        let team = team();
        let ball = Vec3::new(4.4, 0.0, 0.0);
        let cmd = decide(&situation(&team, 4, ball), &mut DetRng::new(2));
        assert!(cmd.kick > 0 && cmd.aim_x > 0);
    }

    #[test]
    fn same_inputs_same_decision() {
        let team = team();
        let s = situation(&team, 4, Vec3::new(4.4, 0.0, 0.0));
        assert_eq!(
            decide(&s, &mut DetRng::new(5)),
            decide(&s, &mut DetRng::new(5))
        );
    }
}
