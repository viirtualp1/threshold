use crate::math::Vec3;
use crate::scene::{PORTALS, PROPS, Portal, Prop, ROOMS};

pub const STEP: f32 = 1.0 / 240.0;
const EYE_HEIGHT: f32 = 1.58;
const BODY_HEIGHT: f32 = 1.78;
const BODY_RADIUS: f32 = 0.23;
const BALL_RADIUS: f32 = 0.16;
const SKIN: f32 = 0.00002;
const WALLS: [Vec3; 4] = [
    Vec3(1.0, 0.0, 0.0),
    Vec3(-1.0, 0.0, 0.0),
    Vec3(0.0, 0.0, 1.0),
    Vec3(0.0, 0.0, -1.0),
];

#[derive(Clone, Copy)]
pub struct Player {
    pub room: usize,
    pub feet: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub size: f32,
}

impl Player {
    pub fn head(self) -> Vec3 {
        self.feet + Vec3::Y * (EYE_HEIGHT * self.size)
    }

    pub fn forward(self) -> Vec3 {
        let (x, z) = self.yaw.sin_cos();
        let (y, horizontal) = self.pitch.sin_cos();
        Vec3(x * horizontal, y, z * horizontal)
    }

    pub fn right(self) -> Vec3 {
        let (sine, cosine) = self.yaw.sin_cos();
        Vec3(cosine, 0.0, -sine)
    }

    pub fn up(self) -> Vec3 {
        let (x, z) = self.yaw.sin_cos();
        let (y, horizontal) = self.pitch.sin_cos();
        Vec3(-x * y, horizontal, -z * y)
    }

    pub fn turn(&mut self, yaw: f32, pitch: f32) {
        self.yaw = (self.yaw + yaw).rem_euclid(std::f32::consts::TAU);
        self.pitch = (self.pitch + pitch).clamp(-1.5, 1.5);
    }
}

#[derive(Clone, Copy)]
pub struct Ball {
    pub room: usize,
    pub center: Vec3,
    pub velocity: Vec3,
    pub size: f32,
    pub held: bool,
}

impl Ball {
    pub fn radius(self) -> f32 {
        BALL_RADIUS * self.size
    }
}

pub struct Simulation {
    pub player: Player,
    pub ball: Ball,
}

impl Default for Simulation {
    fn default() -> Self {
        Self {
            player: Player {
                room: 0,
                feet: Vec3(0.0, 0.0, -3.95),
                yaw: 0.0,
                pitch: -0.055,
                size: 1.0,
            },
            ball: Ball {
                room: 0,
                center: Vec3(0.59, 0.92 + BALL_RADIUS, 0.38),
                velocity: Vec3::ZERO,
                size: 1.0,
                held: false,
            },
        }
    }
}

impl Simulation {
    pub fn step(&mut self, sideways: f32, forward: f32, sprint: bool) {
        let facing = Vec3(self.player.yaw.sin(), 0.0, self.player.yaw.cos());
        let motion = self.player.right() * sideways + facing * forward;
        let speed = if sprint { 6.6 } else { 3.8 };
        let travel = motion / motion.length().max(1.0) * (speed * self.player.size * STEP);
        self.move_player(travel);

        if self.ball.held {
            self.place_held_ball();
        } else {
            self.move_ball();
        }
    }

    pub fn can_grab(&self) -> bool {
        if self.ball.room != self.player.room || self.ball.held {
            return false;
        }

        let eye = self.player.head();
        let direction = self.player.forward();
        let offset = self.ball.center - eye;
        let along = offset.dot(direction);
        let aim_radius = self.ball.radius() + 0.07 * self.player.size;
        let miss = offset - direction * along;

        if along <= 0.0
            || along > 2.8 * self.player.size
            || miss.dot(miss) > aim_radius * aim_radius
        {
            return false;
        }

        let near = along - (aim_radius * aim_radius - miss.dot(miss)).sqrt();
        !ROOMS[self.player.room]
            .props
            .clone()
            .any(|index| PROPS[index].blocks_ray(eye, direction, near))
    }

    pub fn interact(&mut self) {
        if self.ball.held {
            self.ball.held = false;
            self.ball.velocity = (self.player.forward() * 12.0 + Vec3::Y * 1.6) * self.player.size;
        } else if self.can_grab() {
            self.ball.held = true;
            self.ball.velocity = Vec3::ZERO;
            self.place_held_ball();
        }
    }

    fn move_player(&mut self, travel: Vec3) {
        let player = &mut self.player;
        let start = player.feet;
        let mut end = start + travel;

        if let Some(portal) = crossing(
            player.room,
            start,
            end,
            BODY_RADIUS * player.size,
            0.0,
            BODY_HEIGHT * player.size,
            player.size,
        ) {
            end = portal.point(end) + portal.destination().inward * SKIN;
            player.size *= portal.ratio();
            player.room = portal.destination().room;
            let heading = portal.vector(player.forward());
            player.yaw = heading.0.atan2(heading.2);
            if self.ball.held {
                let radius = self.ball.radius();
                if portal.fits(self.ball.center, radius, radius, radius)
                    && scale_allowed(portal, self.ball.size)
                {
                    self.ball.size *= portal.ratio();
                } else {
                    self.ball.held = false;
                }
            }
        }

        let radius = BODY_RADIUS * player.size;
        let height = BODY_HEIGHT * player.size;
        let room = &ROOMS[player.room];

        for _ in 0..2 {
            for normal in WALLS {
                let depth = wall_depth(player.room, end, normal);
                if depth < radius
                    && (depth < 0.0
                        || !opening(player.room, normal, end, radius, 0.0, height, player.size))
                {
                    end += normal * (radius - depth);
                }
            }

            for index in room.props.clone() {
                let prop = PROPS[index];
                if prop.min.1 >= height {
                    continue;
                }

                let shape = Prop {
                    min: Vec3(prop.min.0, -1.0, prop.min.2),
                    max: Vec3(prop.max.0, 1.0, prop.max.2),
                    ..prop
                };
                let (normal, penetration) = sphere_contact(end, radius, shape);
                end += normal * penetration;
            }
        }

        player.feet = end;
    }

    fn place_held_ball(&mut self) {
        let player = self.player;
        let eye = player.head();
        let reach = 0.84 * player.size + self.ball.radius();
        let mut target = eye + player.forward() * reach;
        target += player.right() * (0.2 * player.size) - Vec3::Y * (0.18 * player.size);

        let room = &ROOMS[player.room];
        for index in room.props.clone() {
            let ray = target - eye;
            let distance = ray.length();
            if PROPS[index].blocks_ray(eye, ray.normalized(), distance) {
                target = eye + ray * 0.4;
            }
        }

        let radius = self.ball.radius();
        let offsets = Vec3(radius, radius, radius);
        self.ball.room = player.room;
        self.ball.center = target.clamped(room.min() + offsets, room.max() - offsets);

        for index in room.props.clone() {
            let (normal, depth) = sphere_contact(self.ball.center, radius, PROPS[index]);
            self.ball.center += normal * depth;
        }
        self.ball.center = self
            .ball
            .center
            .clamped(room.min() + offsets, room.max() - offsets);
    }

    fn move_ball(&mut self) {
        let ball = &mut self.ball;
        ball.velocity.1 -= 9.81 * ball.size * STEP;
        let start = ball.center;
        let mut end = start + ball.velocity * STEP;
        end.1 = end.1.max(ball.radius());

        if let Some(portal) = crossing(
            ball.room,
            start,
            end,
            ball.radius(),
            ball.radius(),
            ball.radius(),
            ball.size,
        ) {
            end = portal.point(end) + portal.destination().inward * SKIN;
            ball.velocity = portal.vector(ball.velocity) * portal.ratio();
            ball.size *= portal.ratio();
            ball.room = portal.destination().room;
        }

        ball.center = end;
        let radius = ball.radius();
        let room = &ROOMS[ball.room];

        for _ in 0..2 {
            if ball.center.1 <= radius {
                bounce(ball, Vec3::Y, radius - ball.center.1);
            }
            if ball.center.1 + radius > room.height {
                bounce(ball, -Vec3::Y, ball.center.1 + radius - room.height);
            }

            for normal in WALLS {
                let depth = wall_depth(ball.room, ball.center, normal);
                if depth < radius
                    && (depth < 0.0
                        || !opening(
                            ball.room,
                            normal,
                            ball.center,
                            radius,
                            radius,
                            radius,
                            ball.size,
                        ))
                {
                    bounce(ball, normal, radius - depth);
                }
            }

            for index in room.props.clone() {
                let (normal, penetration) = sphere_contact(ball.center, radius, PROPS[index]);
                if penetration > 0.0 {
                    bounce(ball, normal, penetration);
                }
            }

            for index in room.portals.clone() {
                let portal = PORTALS[index];
                let depth = (ball.center - portal.bottom).dot(portal.inward);
                if depth >= radius || !portal.fits(ball.center, radius, radius, radius) {
                    continue;
                }

                let exit = portal.destination();
                let ratio = portal.ratio();
                for index in ROOMS[exit.room].props.clone() {
                    let (normal, penetration) =
                        sphere_contact(portal.point(ball.center), radius * ratio, PROPS[index]);
                    if penetration > 0.0 {
                        bounce(ball, exit.vector(normal), penetration / ratio);
                    }
                }
            }
        }
    }
}

fn crossing(
    room: usize,
    start: Vec3,
    end: Vec3,
    radius: f32,
    below: f32,
    above: f32,
    size: f32,
) -> Option<Portal> {
    ROOMS[room]
        .portals
        .clone()
        .filter_map(|index| {
            let portal = PORTALS[index];
            let from = (start - portal.bottom).dot(portal.inward);
            let to = (end - portal.bottom).dot(portal.inward);

            if from >= 0.0 && to < 0.0 && scale_allowed(portal, size) {
                let fraction = from / (from - to);
                let hit = start + (end - start) * fraction;
                if portal.fits(hit, radius, below, above) {
                    return Some((fraction, portal));
                }
            }

            None
        })
        .min_by(|first, second| first.0.total_cmp(&second.0))
        .map(|(_, portal)| portal)
}

fn scale_allowed(portal: Portal, size: f32) -> bool {
    (0.00095..=12.0).contains(&(size * portal.ratio()))
}

fn opening(
    room: usize,
    normal: Vec3,
    position: Vec3,
    radius: f32,
    below: f32,
    above: f32,
    size: f32,
) -> bool {
    ROOMS[room].portals.clone().any(|index| {
        let portal = PORTALS[index];
        portal.inward == normal
            && scale_allowed(portal, size)
            && portal.fits(position, radius, below, above)
    })
}

fn wall_depth(room: usize, point: Vec3, normal: Vec3) -> f32 {
    let [x, z] = ROOMS[room].half;
    point.dot(normal) + normal.0.abs() * x + normal.2.abs() * z
}

fn sphere_contact(center: Vec3, radius: f32, prop: Prop) -> (Vec3, f32) {
    let offset = center - center.clamped(prop.min, prop.max);
    let distance_squared = offset.dot(offset);
    if distance_squared >= radius * radius {
        return (Vec3::ZERO, 0.0);
    }
    if distance_squared > 0.0 {
        let distance = distance_squared.sqrt();
        return (offset / distance, radius - distance);
    }

    let faces = [
        (center.0 - prop.min.0, Vec3(-1.0, 0.0, 0.0)),
        (prop.max.0 - center.0, Vec3(1.0, 0.0, 0.0)),
        (center.1 - prop.min.1, -Vec3::Y),
        (prop.max.1 - center.1, Vec3::Y),
        (center.2 - prop.min.2, Vec3(0.0, 0.0, -1.0)),
        (prop.max.2 - center.2, Vec3(0.0, 0.0, 1.0)),
    ];
    let (depth, normal) = faces
        .into_iter()
        .min_by(|first, second| first.0.total_cmp(&second.0))
        .unwrap();
    (normal, radius + depth)
}

fn bounce(ball: &mut Ball, normal: Vec3, depth: f32) {
    ball.center += normal * depth;
    let impact = ball.velocity.dot(normal);

    if impact < 0.0 {
        let restitution = if impact < -0.55 * ball.size {
            0.68
        } else {
            0.0
        };
        ball.velocity -= normal * ((1.0 + restitution) * impact);
        if normal.1 > 0.0 {
            let drag = if restitution > 0.0 { 0.06 } else { 3.1 * STEP };
            ball.velocity -= (ball.velocity - normal * ball.velocity.dot(normal)) * drag;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_doors_round_trip_points_and_velocities() {
        for door in PORTALS {
            let exit = door.destination();
            let point = door.bottom + door.right() * 0.12 + Vec3::Y * 0.21 + door.inward * 0.32;
            let vector = Vec3(1.8, -3.7, 8.6);
            assert!((exit.point(door.point(point)) - point).length() < 0.0001);
            assert!((exit.vector(door.vector(vector)) - vector).length() < 0.0001);
            assert!((door.ratio() * exit.ratio() - 1.0).abs() < 0.0001);
        }
    }

    #[test]
    fn both_vestibule_doors_lead_to_the_same_room() {
        for (door_id, scale) in [(0, 1.0), (1, 0.1)] {
            let door = PORTALS[door_id];
            let mut world = Simulation::default();
            world.player.feet = door.bottom + door.inward * 0.4;
            for _ in 0..60 {
                world.step(0.0, 1.0, false);
            }
            assert_eq!(world.player.room, 1);
            assert!((world.player.size - scale).abs() < 0.0001);

            for _ in 0..63 {
                world.step(0.0, -1.0, false);
            }
            assert_eq!(world.player.room, 0);
            assert!((world.player.size - 1.0).abs() < 0.0001);
        }
    }

    #[test]
    fn stepping_into_the_gallery_carries_the_ball() {
        let mut world = Simulation::default();
        world.player.feet = PORTALS[2].bottom + PORTALS[2].inward * 0.25;
        world.player.yaw = -std::f32::consts::FRAC_PI_2;
        world.ball.held = true;

        for _ in 0..30 {
            world.step(0.0, 1.0, false);
        }
        assert_eq!(world.player.room, 2);
        assert_eq!(world.ball.room, 2);
        assert!(world.ball.held);
        assert!((world.ball.size - 1.0).abs() < 0.0001);
    }

    #[test]
    fn shot_through_blue_door_scales_ball_and_reverses_its_heading() {
        let mut world = Simulation::default();
        world.player.feet = Vec3(2.25, 0.0, 3.30);
        world.player.pitch = 0.0;
        world.ball.held = true;
        world.step(0.0, 0.0, false);
        world.interact();

        for _ in 0..40 {
            world.step(0.0, 0.0, false);
        }

        assert!(!world.ball.held);
        assert_eq!(world.ball.room, 1);
        assert!((world.ball.radius() - 0.016).abs() < 0.00001);
        assert!((world.ball.velocity.2 + 1.2).abs() < 0.0001);
    }

    #[test]
    fn rolling_ball_can_cross_an_opening_flush_with_the_floor() {
        let mut world = Simulation::default();
        world.ball.center = Vec3(-2.25, BALL_RADIUS, 4.65);
        world.ball.velocity = Vec3(0.0, 0.0, 8.0);
        for _ in 0..30 {
            world.step(0.0, 0.0, false);
        }
        assert_eq!(world.ball.room, 1);
        assert!((world.ball.center.1 - BALL_RADIUS).abs() < 0.00001);
    }

    #[test]
    fn full_size_player_collides_with_the_tiny_exit() {
        let mut world = Simulation::default();
        world.player.room = 1;
        world.player.feet = Vec3(2.25, 0.0, 4.03);
        for _ in 0..120 {
            world.step(0.0, 1.0, false);
        }
        assert_eq!(world.player.room, 1);
        assert!((world.player.feet.2 - (5.0 - BODY_RADIUS)).abs() < 0.00001);
    }

    #[test]
    fn a_held_ball_that_cannot_fit_stays_behind() {
        let mut world = Simulation::default();
        world.player.room = 1;
        world.player.feet = Vec3(2.25, 0.0, 4.95);
        world.player.size = 0.1;
        world.ball.room = 1;
        world.ball.held = true;

        for _ in 0..60 {
            world.step(0.0, 1.0, false);
        }

        assert_eq!(world.player.room, 0);
        assert_eq!(world.ball.room, 1);
        assert!(!world.ball.held);
        assert!((world.ball.radius() - BALL_RADIUS).abs() < 0.00001);
    }

    #[test]
    fn pickup_hits_the_ball_only_within_reach_and_line_of_sight() {
        let mut world = Simulation::default();
        assert!(!world.can_grab());
        world.player.feet = Vec3(0.59, 0.0, -1.23);
        let to_ball = world.ball.center - world.player.head();
        world.player.pitch = to_ball.1.atan2(to_ball.2);
        assert!(world.can_grab());
        world.interact();
        assert!(world.ball.held);
        world.interact();
        assert!(!world.ball.held);
        assert!(world.ball.velocity.2 > 10.0);

        world.ball.center = Vec3(0.59, 0.16, 1.0);
        let blocked = world.ball.center - world.player.head();
        world.player.pitch = blocked.1.atan2(blocked.2);
        assert!(!world.can_grab());
    }
}
