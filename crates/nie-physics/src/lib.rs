//! Ball and player physics on a fixed time step.
//!
//! The pitch lies in the x/y plane (x along the touchline, y across); the ball also carries a
//! height `z`. Only `+ - * /` and `sqrt` are used, all exactly rounded under IEEE 754, so a
//! match replays bit for bit on every platform that Rust supports.

use bevy_math::{Vec2, Vec3};

/// Pitch geometry, centered on the origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pitch {
    pub half_length: f32,
    pub half_width: f32,
    pub goal_half_width: f32,
}

impl Pitch {
    pub fn new(length: f32, width: f32, goal_width: f32) -> Self {
        Self {
            half_length: length * 0.5,
            half_width: width * 0.5,
            goal_half_width: goal_width * 0.5,
        }
    }

    pub fn clamp(&self, p: Vec2) -> Vec2 {
        Vec2::new(
            p.x.clamp(-self.half_length, self.half_length),
            p.y.clamp(-self.half_width, self.half_width),
        )
    }
}

/// Which goal line a ball crossed between the posts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoalLine {
    /// The goal at `x = +half_length`.
    Positive,
    /// The goal at `x = -half_length`.
    Negative,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BallParams {
    pub gravity: f32,
    /// Fraction of vertical speed kept after a bounce.
    pub restitution: f32,
    /// Fraction of horizontal speed lost per second while rolling.
    pub rolling_drag: f32,
    /// Fraction of horizontal speed lost per second while airborne.
    pub air_drag: f32,
    /// Fraction of speed kept when the ball hits a boundary wall.
    pub wall_restitution: f32,
    pub goal_height: f32,
}

impl Default for BallParams {
    fn default() -> Self {
        Self {
            gravity: 9.81,
            restitution: 0.55,
            rolling_drag: 0.9,
            air_drag: 0.05,
            wall_restitution: 0.6,
            goal_height: 2.2,
        }
    }
}

/// Ball position (x, y, height) and velocity.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BallState {
    pub pos: Vec3,
    pub vel: Vec3,
}

impl BallState {
    pub fn at_rest(x: f32, y: f32) -> Self {
        Self {
            pos: Vec3::new(x, y, 0.0),
            vel: Vec3::ZERO,
        }
    }

    pub fn xy(&self) -> Vec2 {
        self.pos.truncate()
    }

    pub fn vel_xy(&self) -> Vec2 {
        self.vel.truncate()
    }
}

/// Advances the ball by `dt`. Returns the goal line crossed this step, if any. The ball bounces
/// off the boundary everywhere else, so it never leaves play.
pub fn step_ball(
    ball: &mut BallState,
    pitch: &Pitch,
    params: &BallParams,
    dt: f32,
) -> Option<GoalLine> {
    ball.vel.z -= params.gravity * dt;
    ball.pos += ball.vel * dt;

    if ball.pos.z < 0.0 {
        ball.pos.z = 0.0;
        ball.vel.z = if ball.vel.z < -1.0 {
            -ball.vel.z * params.restitution
        } else {
            0.0
        };
    }

    let drag = if ball.pos.z > 0.0 {
        params.air_drag
    } else {
        params.rolling_drag
    };
    let keep = (1.0 - drag * dt).max(0.0);
    ball.vel.x *= keep;
    ball.vel.y *= keep;
    if ball.vel_xy().length_squared() < 0.0025 && ball.pos.z == 0.0 {
        ball.vel.x = 0.0;
        ball.vel.y = 0.0;
    }

    if ball.pos.x.abs() > pitch.half_length {
        let between_posts =
            ball.pos.y.abs() < pitch.goal_half_width && ball.pos.z < params.goal_height;
        if between_posts {
            return Some(if ball.pos.x > 0.0 {
                GoalLine::Positive
            } else {
                GoalLine::Negative
            });
        }
        ball.pos.x = pitch.half_length.copysign(ball.pos.x);
        ball.vel.x = -ball.vel.x * params.wall_restitution;
    }
    if ball.pos.y.abs() > pitch.half_width {
        ball.pos.y = pitch.half_width.copysign(ball.pos.y);
        ball.vel.y = -ball.vel.y * params.wall_restitution;
    }
    None
}

/// Planar kinematics of one player.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub pos: Vec2,
    pub vel: Vec2,
}

/// Accelerates a player towards `wish` (a vector of length 0 to 1) and integrates, clamped to
/// the pitch.
pub fn step_body(body: &mut Body, wish: Vec2, max_speed: f32, accel: f32, pitch: &Pitch, dt: f32) {
    let target = wish * max_speed;
    let delta = target - body.vel;
    let max_delta = accel * dt;
    let len_sq = delta.length_squared();
    body.vel += if len_sq > max_delta * max_delta {
        delta * (max_delta / len_sq.sqrt())
    } else {
        delta
    };
    body.pos = pitch.clamp(body.pos + body.vel * dt);
}

/// Launch velocity of a kick along `dir` (any length, zero means no kick) with total `speed` in
/// m/s; `loft` in `[0, 1]` is the sine of the launch elevation.
pub fn kick_velocity(dir: Vec2, speed: f32, loft: f32) -> Vec3 {
    let len_sq = dir.length_squared();
    if len_sq == 0.0 {
        return Vec3::ZERO;
    }
    let flat = dir * (speed * (1.0 - loft * loft).sqrt() / len_sq.sqrt());
    Vec3::new(flat.x, flat.y, speed * loft)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn pitch() -> Pitch {
        Pitch::new(40.0, 24.0, 6.0)
    }

    #[test]
    fn dropped_ball_bounces_and_settles() {
        let mut ball = BallState {
            pos: Vec3::new(0.0, 0.0, 3.0),
            vel: Vec3::ZERO,
        };
        let mut apex_after_bounce = 0.0_f32;
        let mut bounced = false;
        for _ in 0..600 {
            step_ball(&mut ball, &pitch(), &BallParams::default(), DT);
            if bounced {
                apex_after_bounce = apex_after_bounce.max(ball.pos.z);
            }
            bounced |= ball.pos.z == 0.0;
        }
        assert!(apex_after_bounce > 0.2 && apex_after_bounce < 3.0);
        assert_eq!(ball.pos.z, 0.0);
    }

    #[test]
    fn rolling_ball_stops() {
        let mut ball = BallState {
            pos: Vec3::ZERO,
            vel: Vec3::new(5.0, 0.0, 0.0),
        };
        for _ in 0..1200 {
            step_ball(&mut ball, &pitch(), &BallParams::default(), DT);
        }
        assert_eq!(ball.vel, Vec3::ZERO);
        assert!(ball.pos.x > 1.0);
    }

    #[test]
    fn ball_between_posts_scores() {
        let mut ball = BallState {
            pos: Vec3::new(19.9, 0.5, 0.0),
            vel: Vec3::new(20.0, 0.0, 0.0),
        };
        assert_eq!(
            step_ball(&mut ball, &pitch(), &BallParams::default(), DT),
            Some(GoalLine::Positive)
        );
    }

    #[test]
    fn ball_outside_posts_bounces() {
        let mut ball = BallState {
            pos: Vec3::new(19.9, 8.0, 0.0),
            vel: Vec3::new(20.0, 0.0, 0.0),
        };
        assert_eq!(
            step_ball(&mut ball, &pitch(), &BallParams::default(), DT),
            None
        );
        assert!(ball.vel.x < 0.0);
        assert_eq!(ball.pos.x, 20.0);
    }

    #[test]
    fn body_respects_speed_and_pitch() {
        let mut body = Body::default();
        for _ in 0..600 {
            step_body(&mut body, Vec2::X, 6.0, 20.0, &pitch(), DT);
        }
        assert_eq!(body.pos.x, 20.0);
        assert!(body.vel.length() <= 6.0 + 1e-4);
    }

    #[test]
    fn kick_has_requested_speed() {
        let v = kick_velocity(Vec2::new(3.0, 4.0), 10.0, 0.2);
        assert!((v.length() - 10.0).abs() < 1e-3);
        assert_eq!(kick_velocity(Vec2::ZERO, 10.0, 0.2), Vec3::ZERO);
    }
}
