use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3(pub f32, pub f32, pub f32);

impl Vec3 {
    pub const ZERO: Self = Self(0.0, 0.0, 0.0);
    pub const Y: Self = Self(0.0, 1.0, 0.0);

    pub fn dot(self, other: Self) -> f32 {
        self.0 * other.0 + self.1 * other.1 + self.2 * other.2
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Self {
        self / self.length().max(f32::EPSILON)
    }

    pub fn clamped(self, min: Self, max: Self) -> Self {
        Self(
            self.0.clamp(min.0, max.0),
            self.1.clamp(min.1, max.1),
            self.2.clamp(min.2, max.2),
        )
    }

    pub fn with(self, w: f32) -> [f32; 4] {
        [self.0, self.1, self.2, w]
    }
}

impl Add for Vec3 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0, self.1 + other.1, self.2 + other.2)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl Sub for Vec3 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0, self.1 - other.1, self.2 - other.2)
    }
}

impl SubAssign for Vec3 {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl Neg for Vec3 {
    type Output = Self;

    fn neg(self) -> Self {
        Self(-self.0, -self.1, -self.2)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, factor: f32) -> Self {
        Self(self.0 * factor, self.1 * factor, self.2 * factor)
    }
}

impl Div<f32> for Vec3 {
    type Output = Self;

    fn div(self, divisor: f32) -> Self {
        Self(self.0 / divisor, self.1 / divisor, self.2 / divisor)
    }
}
