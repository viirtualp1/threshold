use crate::math::Vec3;

pub const ROOMS: [Room; 3] = [
    Room {
        name: "VESTIBULE",
        half: [5.0, 5.0],
        height: 4.0,
        portals: 0..3,
        props: 0..6,
    },
    Room {
        name: "KITCHEN",
        half: [5.0, 5.0],
        height: 4.0,
        portals: 3..5,
        props: 6..21,
    },
    Room {
        name: "GALLERY",
        half: [3.4, 3.6],
        height: 3.5,
        portals: 5..6,
        props: 21..26,
    },
];

pub const PORTALS: [Portal; 6] = [
    Portal::new(
        0,
        Vec3(-2.25, 0.0, 5.0),
        Vec3(0.0, 0.0, -1.0),
        [0.99, 2.9],
        3,
        0,
    ),
    Portal::new(
        0,
        Vec3(2.25, 0.0, 5.0),
        Vec3(0.0, 0.0, -1.0),
        [0.99, 2.9],
        4,
        1,
    ),
    Portal::new(
        0,
        Vec3(-5.0, 0.0, -0.1),
        Vec3(1.0, 0.0, 0.0),
        [0.99, 2.9],
        5,
        2,
    ),
    Portal::new(
        1,
        Vec3(-2.25, 0.0, -5.0),
        Vec3(0.0, 0.0, 1.0),
        [0.99, 2.9],
        0,
        0,
    ),
    Portal::new(
        1,
        Vec3(2.25, 0.0, 5.0),
        Vec3(0.0, 0.0, -1.0),
        [0.099, 0.29],
        1,
        1,
    ),
    Portal::new(
        2,
        Vec3(3.4, 0.0, -0.1),
        Vec3(-1.0, 0.0, 0.0),
        [0.99, 2.9],
        2,
        2,
    ),
];

// Geometry, collision, and the GPU use the same room-local bounds.
pub const PROPS: [Prop; 26] = [
    Prop::new([0.24, 0.0, 0.03], [0.94, 0.86, 0.73], 5),
    Prop::new([0.18, 0.86, -0.03], [1.0, 0.92, 0.79], 6),
    Prop::new([-4.72, 0.0, 2.1], [-3.71, 0.46, 4.44], 5),
    Prop::new([-4.77, 0.46, 2.05], [-3.66, 0.56, 4.49], 7),
    Prop::new([4.42, 0.0, -1.88], [4.98, 1.04, 2.4], 4),
    Prop::new([4.37, 1.04, -1.94], [5.0, 1.11, 2.46], 6),
    Prop::new([-4.96, 0.0, -2.8], [-3.99, 0.88, 3.64], 8),
    Prop::new([-4.99, 0.88, -2.84], [-3.93, 1.0, 3.7], 6),
    Prop::new([-4.98, 1.94, -2.8], [-4.35, 2.87, 3.64], 8),
    Prop::new([-4.99, 0.0, 3.79], [-3.97, 2.96, 4.95], 9),
    Prop::new([-4.99, 0.0, 4.02], [-3.94, 0.13, 4.84], 5),
    Prop::new([-2.11, 0.0, 0.1], [0.45, 0.88, 1.77], 8),
    Prop::new([-2.21, 0.88, -0.02], [0.55, 1.0, 1.89], 6),
    Prop::new([-1.96, 1.0, 0.41], [-1.1, 1.065, 1.27], 4),
    Prop::new([-1.67, 1.06, 0.63], [-1.39, 1.29, 0.9], 7),
    Prop::new([3.97, 0.0, 0.64], [4.98, 0.89, 3.62], 8),
    Prop::new([3.91, 0.89, 0.58], [5.0, 0.99, 3.69], 6),
    Prop::new([3.98, 1.96, 0.64], [4.98, 2.87, 3.62], 8),
    Prop::new([4.13, 0.0, -3.87], [4.98, 0.42, -1.73], 5),
    Prop::new([-1.76, 3.37, 0.68], [-0.05, 3.47, 1.11], 10),
    Prop::new([-0.94, 3.46, 0.87], [-0.88, 4.0, 0.93], 5),
    Prop::new([-2.1, 0.0, -1.04], [-0.5, 0.72, 1.04], 5),
    Prop::new([-1.93, 0.72, -0.87], [-0.66, 1.29, 0.87], 7),
    Prop::new([-1.75, 1.29, -0.68], [-0.84, 1.92, 0.68], 9),
    Prop::new([-1.57, 1.92, -0.52], [-1.01, 2.37, 0.52], 7),
    Prop::new([-1.41, 2.37, -0.38], [-1.17, 2.65, 0.38], 10),
];

pub struct Room {
    pub name: &'static str,
    pub half: [f32; 2],
    pub height: f32,
    pub portals: std::ops::Range<usize>,
    pub props: std::ops::Range<usize>,
}

impl Room {
    pub fn min(&self) -> Vec3 {
        Vec3(-self.half[0], 0.0, -self.half[1])
    }

    pub fn max(&self) -> Vec3 {
        Vec3(self.half[0], self.height, self.half[1])
    }
}

#[derive(Clone, Copy)]
pub struct Portal {
    pub room: usize,
    pub bottom: Vec3,
    pub inward: Vec3,
    pub span: [f32; 2],
    pub link: usize,
    pub style: usize,
}

impl Portal {
    const fn new(
        room: usize,
        bottom: Vec3,
        inward: Vec3,
        span: [f32; 2],
        link: usize,
        style: usize,
    ) -> Self {
        Self {
            room,
            bottom,
            inward,
            span,
            link,
            style,
        }
    }

    pub fn right(self) -> Vec3 {
        Vec3(self.inward.2, 0.0, -self.inward.0)
    }

    pub fn destination(self) -> Self {
        PORTALS[self.link]
    }

    pub fn ratio(self) -> f32 {
        self.destination().span[0] / self.span[0]
    }

    pub fn vector(self, vector: Vec3) -> Vec3 {
        let exit = self.destination();

        exit.right() * -self.right().dot(vector) + Vec3::Y * vector.1
            - exit.inward * self.inward.dot(vector)
    }

    pub fn point(self, point: Vec3) -> Vec3 {
        self.destination().bottom + self.vector(point - self.bottom) * self.ratio()
    }

    pub fn fits(self, center: Vec3, radius: f32, below: f32, above: f32) -> bool {
        let relative = center - self.bottom;
        self.right().dot(relative).abs() + radius <= self.span[0]
            && relative.1 - below >= -0.00001
            && relative.1 + above <= self.span[1]
    }
}

#[derive(Clone, Copy)]
pub struct Prop {
    pub min: Vec3,
    pub max: Vec3,
    pub material: u32,
}

impl Prop {
    const fn new(min: [f32; 3], max: [f32; 3], material: u32) -> Self {
        Self {
            min: Vec3(min[0], min[1], min[2]),
            max: Vec3(max[0], max[1], max[2]),
            material,
        }
    }

    pub fn blocks_ray(self, origin: Vec3, heading: Vec3, reach: f32) -> bool {
        let starts = [origin.0, origin.1, origin.2];
        let directions = [heading.0, heading.1, heading.2];
        let lower = [self.min.0, self.min.1, self.min.2];
        let upper = [self.max.0, self.max.1, self.max.2];
        let mut enter: f32 = 0.0;
        let mut leave = reach;

        for axis in 0..3 {
            let direction = directions[axis];
            if direction.abs() < 1.0e-6 {
                if starts[axis] < lower[axis] || starts[axis] > upper[axis] {
                    return false;
                }
                continue;
            }

            let first = (lower[axis] - starts[axis]) / direction;
            let last = (upper[axis] - starts[axis]) / direction;
            enter = enter.max(first.min(last));
            leave = leave.min(first.max(last));
        }

        enter < leave
    }
}
