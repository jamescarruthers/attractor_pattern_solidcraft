//! Small 3D maths: vectors, rotations, rigid motions and frames.

use std::ops::{Add, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub const X: V3 = V3::new(1.0, 0.0, 0.0);
pub const Y: V3 = V3::new(0.0, 1.0, 0.0);
pub const Z: V3 = V3::new(0.0, 0.0, 1.0);
pub const ZERO: V3 = V3::new(0.0, 0.0, 0.0);

impl V3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn from_array(a: [f64; 3]) -> Self {
        Self::new(a[0], a[1], a[2])
    }
    pub fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
    pub fn dot(self, o: V3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        V3::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    pub fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn dist(self, o: V3) -> f64 {
        (self - o).len()
    }
    /// The unit vector along this one, or `None` when it is (nearly) zero or not finite.
    pub fn norm(self) -> Option<V3> {
        let l = self.len();
        (l.is_finite() && l > 1e-12).then(|| self * (1.0 / l))
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
    /// The part of this vector at right angles to the unit vector `n`.
    pub fn reject(self, n: V3) -> V3 {
        self - n * self.dot(n)
    }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        V3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        V3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f64> for V3 {
    type Output = V3;
    fn mul(self, k: f64) -> V3 {
        V3::new(self.x * k, self.y * k, self.z * k)
    }
}
impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        V3::new(-self.x, -self.y, -self.z)
    }
}

/// A 3x3 matrix stored as its three columns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct M3 {
    pub c0: V3,
    pub c1: V3,
    pub c2: V3,
}

impl M3 {
    pub const IDENTITY: M3 = M3 { c0: X, c1: Y, c2: Z };

    pub fn apply(&self, v: V3) -> V3 {
        self.c0 * v.x + self.c1 * v.y + self.c2 * v.z
    }
    pub fn mul(&self, o: &M3) -> M3 {
        M3 { c0: self.apply(o.c0), c1: self.apply(o.c1), c2: self.apply(o.c2) }
    }
    pub fn transpose(&self) -> M3 {
        M3 {
            c0: V3::new(self.c0.x, self.c1.x, self.c2.x),
            c1: V3::new(self.c0.y, self.c1.y, self.c2.y),
            c2: V3::new(self.c0.z, self.c1.z, self.c2.z),
        }
    }
    /// Element at row `r`, column `c` (both 0..3).
    fn at(&self, r: usize, c: usize) -> f64 {
        let col = match c {
            0 => self.c0,
            1 => self.c1,
            _ => self.c2,
        };
        match r {
            0 => col.x,
            1 => col.y,
            _ => col.z,
        }
    }

    /// A right-handed turn of `angle` radians about the unit vector `k` (Rodrigues).
    pub fn rotation(k: V3, angle: f64) -> M3 {
        let (s, c) = angle.sin_cos();
        let t = 1.0 - c;
        M3 {
            c0: V3::new(t * k.x * k.x + c, t * k.x * k.y + s * k.z, t * k.x * k.z - s * k.y),
            c1: V3::new(t * k.x * k.y - s * k.z, t * k.y * k.y + c, t * k.y * k.z + s * k.x),
            c2: V3::new(t * k.x * k.z + s * k.y, t * k.y * k.z - s * k.x, t * k.z * k.z + c),
        }
    }

    /// The axis and angle (radians, 0..=pi) of a rotation matrix; `None` when it does not turn.
    pub fn axis_angle(&self) -> Option<(V3, f64)> {
        let trace = self.at(0, 0) + self.at(1, 1) + self.at(2, 2);
        let angle = ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos();
        if !angle.is_finite() || angle < 1e-12 {
            return None;
        }
        let skew = V3::new(self.at(2, 1) - self.at(1, 2), self.at(0, 2) - self.at(2, 0), self.at(1, 0) - self.at(0, 1));
        if angle < std::f64::consts::PI - 1e-6 {
            return skew.norm().map(|k| (k, angle));
        }
        // Near a half turn the skew part vanishes; read the axis from the symmetric part.
        let xx = ((self.at(0, 0) + 1.0) / 2.0).max(0.0).sqrt();
        let yy = ((self.at(1, 1) + 1.0) / 2.0).max(0.0).sqrt();
        let zz = ((self.at(2, 2) + 1.0) / 2.0).max(0.0).sqrt();
        let k = if xx >= yy && xx >= zz {
            V3::new(xx, (self.at(0, 1) + self.at(1, 0)) / (4.0 * xx), (self.at(0, 2) + self.at(2, 0)) / (4.0 * xx))
        } else if yy >= zz {
            V3::new((self.at(0, 1) + self.at(1, 0)) / (4.0 * yy), yy, (self.at(1, 2) + self.at(2, 1)) / (4.0 * yy))
        } else {
            V3::new((self.at(0, 2) + self.at(2, 0)) / (4.0 * zz), (self.at(1, 2) + self.at(2, 1)) / (4.0 * zz), zz)
        };
        k.norm().map(|k| (k, angle))
    }
}

/// A rigid motion: `p -> r p + t`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rigid {
    pub r: M3,
    pub t: V3,
}

impl Rigid {
    pub const IDENTITY: Rigid = Rigid { r: M3::IDENTITY, t: ZERO };

    pub fn apply(&self, p: V3) -> V3 {
        self.r.apply(p) + self.t
    }
    /// `self` after `first`: `p -> self(first(p))`.
    pub fn then_after(&self, first: &Rigid) -> Rigid {
        Rigid { r: self.r.mul(&first.r), t: self.r.apply(first.t) + self.t }
    }
    pub fn inverse(&self) -> Rigid {
        let rt = self.r.transpose();
        Rigid { r: rt, t: -rt.apply(self.t) }
    }
    pub fn is_identity(&self) -> bool {
        self.t.len() < 1e-12 && self.r.axis_angle().is_none()
    }
}

/// A right-handed orthonormal frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: V3,
    pub x: V3,
    pub y: V3,
    pub z: V3,
}

impl Frame {
    pub const WORLD: Frame = Frame { origin: ZERO, x: X, y: Y, z: Z };

    /// A frame from an origin, a normal (`z`) and a direction that `x` should follow; `x` is
    /// made square to `z`, and a world axis is used when the hint is parallel to `z`.
    pub fn from_z_x(origin: V3, z: V3, x_hint: V3) -> Option<Frame> {
        let z = z.norm()?;
        let x = x_hint.reject(z).norm().or_else(|| any_perpendicular(z))?;
        Some(Frame { origin, x, y: z.cross(x), z })
    }

    /// Local coordinates to world.
    pub fn to_world(&self) -> Rigid {
        Rigid { r: M3 { c0: self.x, c1: self.y, c2: self.z }, t: self.origin }
    }
    /// World coordinates to local.
    pub fn from_world(&self) -> Rigid {
        self.to_world().inverse()
    }
}

/// A unit vector at right angles to the unit vector `n`.
pub fn any_perpendicular(n: V3) -> Option<V3> {
    let helper = if n.x.abs() < 0.9 { X } else { Y };
    helper.reject(n).norm()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: V3, b: V3) -> bool {
        a.dist(b) < 1e-9
    }

    #[test]
    fn axis_angle_round_trips() {
        for (k, a) in [(Z, 0.3), (V3::new(1.0, 2.0, 3.0), 2.0), (X, std::f64::consts::PI), (V3::new(1.0, 1.0, 0.0), 3.1)] {
            let k = k.norm().unwrap();
            let m = M3::rotation(k, a);
            let (k2, a2) = m.axis_angle().unwrap();
            let m2 = M3::rotation(k2, a2);
            for v in [X, Y, Z] {
                assert!(close(m.apply(v), m2.apply(v)), "{k:?} {a}");
            }
        }
        assert!(M3::IDENTITY.axis_angle().is_none());
    }

    #[test]
    fn frames_invert() {
        let f = Frame::from_z_x(V3::new(1.0, 2.0, 3.0), V3::new(0.0, 1.0, 1.0), X).unwrap();
        let p = V3::new(4.0, -5.0, 6.0);
        assert!(close(f.from_world().apply(f.to_world().apply(p)), p));
        assert!((f.x.cross(f.y).dot(f.z) - 1.0).abs() < 1e-12);
    }
}
