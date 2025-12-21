use std::any::Any;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use approx::ulps_eq;
use bevy::prelude::{Component, Reflect};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Reflect, Component, Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Default)]
#[derive(Eq)]
pub struct Id(pub u32);

#[derive(Component, Serialize, Deserialize, Default, Clone, Copy, Debug)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Component, Serialize, Deserialize, Default, Clone, Copy, Debug)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x,
            y,
            z
        }
    }
}

impl Into<bevy::math::Vec3> for Vec3 {
    fn into(self) -> bevy::math::Vec3 {
        bevy::math::Vec3::new(self.x, self.y, self.z)
    }
}

impl Into<Vec3> for bevy::math::Vec3 {
    fn into(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
        }
    }
}

impl Into<bevy::math::Vec2> for Vec2 {
    fn into(self) -> bevy::math::Vec2 {
        bevy::math::Vec2::new(self.x, self.y)
    }
}

impl Into<Vec2> for bevy::math::Vec2 {
    fn into(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}

impl AddAssign<Vec2> for Vec2 {
    fn add_assign(&mut self, rhs: Vec2) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub<Vec2> for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Self::Output {
        Self::Output {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl PartialEq for Vec3 {
    fn eq(&self, other: &Self) -> bool {
        ulps_eq!(self.x, other.x) && ulps_eq!(self.y, other.y) && ulps_eq!(self.z, other.z)
    }
}

impl PartialEq for Vec2 {
    fn eq(&self, other: &Self) -> bool {
        ulps_eq!(self.x, other.x) && ulps_eq!(self.y, other.y)
    }
}