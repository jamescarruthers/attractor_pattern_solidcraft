//! Faces as the plugin sees them, rebuilt from `model.faces` and `model.edges`.
//!
//! The engine has no command that evaluates a face at a (u, v) parameter, so the plugin builds
//! its own parameterisation: planar faces (with their real outline and holes) and cylindrical
//! faces (as an untrimmed patch between their edges). Curved edges are known only by their
//! start, middle and end points, so they are traced as circular arcs through those points.

use crate::vec::{Frame, V3, X, Y, any_perpendicular};
use serde::Deserialize;
use std::f64::consts::TAU;

/// Points closer than this (mm) are the same point.
const TOL: f64 = 1e-3;

/// One edge as `model.edges` lists it.
#[derive(Clone, Debug, Deserialize)]
pub struct EdgeInfo {
    pub mid: [f64; 3],
    #[serde(default)]
    pub length: f64,
    #[serde(default)]
    pub start: Option<[f64; 3]>,
    #[serde(default)]
    pub end: Option<[f64; 3]>,
}

/// One face as `model.faces` lists it.
#[derive(Clone, Debug, Deserialize)]
pub struct FaceInfo {
    pub index: usize,
    #[serde(default)]
    pub name: Option<String>,
    pub centroid: [f64; 3],
    #[serde(default)]
    pub plane_normal: Option<[f64; 3]>,
    #[serde(default)]
    pub point: Option<[f64; 3]>,
    #[serde(default)]
    pub cylinder: Option<CylinderInfo>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CylinderInfo {
    pub origin: [f64; 3],
    pub axis: [f64; 3],
    pub radius: f64,
}

/// An edge traced as a polyline.
#[derive(Clone, Debug)]
pub struct Traced {
    pub points: Vec<V3>,
    pub straight: bool,
}

impl Traced {
    pub fn start(&self) -> Option<V3> {
        self.points.first().copied()
    }
    pub fn end(&self) -> Option<V3> {
        self.points.last().copied()
    }
    pub fn distance_to(&self, p: V3) -> f64 {
        polyline_distance(&self.points, p)
    }
}

/// Trace an edge. `normal` is the plane a full circle lies in, when known.
pub fn trace_edge(e: &EdgeInfo, normal: Option<V3>) -> Option<Traced> {
    let m = V3::from_array(e.mid);
    let (s, t) = match (e.start, e.end) {
        (Some(s), Some(t)) => (V3::from_array(s), V3::from_array(t)),
        _ => return None,
    };
    if !(s.is_finite() && t.is_finite() && m.is_finite()) {
        return None;
    }
    if s.dist(t) < TOL {
        // A closed edge: a full circle with `mid` opposite its start.
        let c = (s + m) * 0.5;
        let r = s.dist(m) * 0.5;
        let n = normal?;
        let e1 = (s - c).reject(n).norm()?;
        let e2 = n.cross(e1);
        let points = (0..=72).map(|i| c + (e1 * (i as f64 * TAU / 72.0).cos() + e2 * (i as f64 * TAU / 72.0).sin()) * r).collect();
        return Some(Traced { points, straight: false });
    }
    let chord = t - s;
    let off_line = (m - s).reject(chord.norm()?).len();
    if off_line <= 1e-6 * chord.len().max(1.0) {
        return Some(Traced { points: vec![s, t], straight: true });
    }
    let points = arc_through(s, m, t).unwrap_or_else(|| vec![s, m, t]);
    Some(Traced { points, straight: false })
}

/// Points along the circular arc from `a` through `b` to `c`.
fn arc_through(a: V3, b: V3, c: V3) -> Option<Vec<V3>> {
    let ab = b - a;
    let ac = c - a;
    let n = ab.cross(ac);
    let n2 = n.dot(n);
    if n2 < 1e-18 {
        return None;
    }
    let centre = a + (n.cross(ab) * ac.dot(ac) + ac.cross(n) * ab.dot(ab)) * (1.0 / (2.0 * n2));
    let k = n.norm()?;
    let r = a.dist(centre);
    let e1 = (a - centre).norm()?;
    let e2 = k.cross(e1);
    let angle = |p: V3| {
        let d = p - centre;
        d.dot(e2).atan2(d.dot(e1)).rem_euclid(TAU)
    };
    // `k` is chosen so that a -> b -> c runs counter-clockwise about it.
    let sweep = angle(c);
    let steps = ((sweep / (5.0_f64).to_radians()).ceil() as usize).clamp(4, 72);
    Some((0..=steps).map(|i| centre + (e1 * (sweep * i as f64 / steps as f64).cos() + e2 * (sweep * i as f64 / steps as f64).sin()) * r).collect())
}

/// Distance from `p` to a polyline.
pub fn polyline_distance(points: &[V3], p: V3) -> f64 {
    match points {
        [] => f64::INFINITY,
        [only] => only.dist(p),
        _ => points.windows(2).filter_map(|w| match w {
            [a, b] => Some(segment_distance(*a, *b, p)),
            _ => None,
        }).fold(f64::INFINITY, f64::min),
    }
}

pub fn segment_distance(a: V3, b: V3, p: V3) -> f64 {
    let d = b - a;
    let l2 = d.dot(d);
    if l2 < 1e-24 {
        return a.dist(p);
    }
    let t = ((p - a).dot(d) / l2).clamp(0.0, 1.0);
    (a + d * t).dist(p)
}

/// A point on a face with its tangent frame: `z` along the face's normal, `x` along `u`.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub frame: Frame,
    /// How far the point is from the face's boundary (mm).
    pub border: f64,
}

#[derive(Clone, Debug)]
pub enum Surface {
    Plane(PlaneFace),
    Cylinder(CylinderFace),
}

impl Surface {
    /// The face at parameters (u, v) in 0..1, or `None` when that point is off the face.
    pub fn sample(&self, u: f64, v: f64) -> Option<Sample> {
        match self {
            Surface::Plane(p) => p.sample(u, v),
            Surface::Cylinder(c) => c.sample(u, v),
        }
    }
    /// The shortest distance from `p` to the face.
    pub fn distance_to(&self, p: V3) -> f64 {
        match self {
            Surface::Plane(f) => f.distance_to(p),
            Surface::Cylinder(f) => f.distance_to(p),
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Surface::Plane(_) => "plane",
            Surface::Cylinder(_) => "cylinder",
        }
    }
}

/// Build the surface of a face from its listing and its body's edges. `None` for a face that
/// is neither planar nor cylindrical, or whose outline cannot be traced.
pub fn build(face: &FaceInfo, edges: &[EdgeInfo], flip_normal: bool) -> Result<Surface, String> {
    let label = face.name.clone().unwrap_or_else(|| format!("face {}", face.index));
    if let Some(n) = face.plane_normal {
        let n = V3::from_array(n).norm().ok_or_else(|| format!("{label} has no usable normal"))?;
        let n = if flip_normal { -n } else { n };
        let pick = face.point.map(V3::from_array).unwrap_or_else(|| V3::from_array(face.centroid));
        return PlaneFace::build(n, pick, edges).map(Surface::Plane).ok_or_else(|| format!("could not trace the outline of {label}"));
    }
    if let Some(c) = &face.cylinder {
        return CylinderFace::build(c, edges, flip_normal).map(Surface::Cylinder).ok_or_else(|| format!("could not find the extent of cylindrical {label}"));
    }
    Err(format!("{label} is neither planar nor cylindrical; only those can be patterned on"))
}

/// A planar face: its plane, an outer loop and holes, in the plane's 2D coordinates.
#[derive(Clone, Debug)]
pub struct PlaneFace {
    /// Origin of the 2D coordinates, on the plane.
    base: V3,
    u: V3,
    v: V3,
    n: V3,
    outer: Vec<[f64; 2]>,
    holes: Vec<Vec<[f64; 2]>>,
    /// The outer loop's bounding box: min and max of (a, b).
    lo: [f64; 2],
    hi: [f64; 2],
}

impl PlaneFace {
    fn build(n: V3, pick: V3, edges: &[EdgeInfo]) -> Option<PlaneFace> {
        let on_plane = |p: [f64; 3]| (V3::from_array(p) - pick).dot(n).abs() < TOL;
        let traced: Vec<Traced> = edges
            .iter()
            .filter(|e| on_plane(e.mid) && e.start.is_some_and(on_plane) && e.end.is_some_and(on_plane))
            .filter_map(|e| trace_edge(e, Some(n)))
            .collect();
        let loops = chain_loops(traced);
        // 2D frame: provisional, until the outer loop tells us which way u runs.
        let u0 = X.reject(n).norm().or_else(|| Y.reject(n).norm()).or_else(|| any_perpendicular(n))?;
        let flat = |f: &Frame2, p: V3| {
            let d = p - f.base;
            [d.dot(f.u), d.dot(f.v)]
        };
        let f0 = Frame2 { base: pick, u: u0, v: n.cross(u0) };
        let pick2 = flat(&f0, pick);
        let flat_loops: Vec<Vec<[f64; 2]>> = loops.iter().map(|l| l.points.iter().map(|p| flat(&f0, *p)).collect()).collect();
        // The outer loop is the smallest loop around the pick point.
        let outer_i = flat_loops
            .iter()
            .enumerate()
            .filter(|(_, l)| point_in_polygon(pick2, l))
            .min_by(|a, b| area(a.1).abs().total_cmp(&area(b.1).abs()))
            .map(|(i, _)| i)?;
        // u runs along the outer loop's longest straight edge.
        let u = loops
            .get(outer_i)
            .and_then(|l| l.straight.iter().max_by(|a, b| a.0.dist(a.1).total_cmp(&b.0.dist(b.1))))
            .and_then(|(a, b)| (*b - *a).reject(n).norm())
            .unwrap_or(u0);
        let f = Frame2 { base: pick, u, v: n.cross(u) };
        let flat_loops: Vec<Vec<[f64; 2]>> = loops.iter().map(|l| l.points.iter().map(|p| flat(&f, *p)).collect()).collect();
        let pick2 = flat(&f, pick);
        let outer = flat_loops.get(outer_i)?.clone();
        // Holes: loops inside the outer one, not around the pick point, and not inside another hole.
        let inside_outer: Vec<usize> = (0..flat_loops.len())
            .filter(|&i| i != outer_i)
            .filter(|&i| flat_loops.get(i).and_then(|l| l.first()).is_some_and(|p| point_in_polygon(*p, &outer)))
            .filter(|&i| flat_loops.get(i).is_some_and(|l| !point_in_polygon(pick2, l)))
            .collect();
        let holes: Vec<Vec<[f64; 2]>> = inside_outer
            .iter()
            .filter(|&&i| {
                let first = flat_loops.get(i).and_then(|l| l.first().copied());
                !inside_outer.iter().any(|&j| j != i && first.is_some_and(|p| flat_loops.get(j).is_some_and(|l| point_in_polygon(p, l))))
            })
            .filter_map(|&i| flat_loops.get(i).cloned())
            .collect();
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for p in &outer {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
        let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
        if !(w.is_finite() && h.is_finite()) || w <= TOL || h <= TOL {
            return None;
        }
        Some(PlaneFace { base: f.base, u: f.u, v: f.v, n, outer, holes, lo, hi })
    }

    fn contains(&self, q: [f64; 2]) -> bool {
        point_in_polygon(q, &self.outer) && !self.holes.iter().any(|h| point_in_polygon(q, h))
    }

    fn boundary_distance(&self, q: [f64; 2]) -> f64 {
        std::iter::once(&self.outer).chain(self.holes.iter()).map(|l| ring_distance(l, q)).fold(f64::INFINITY, f64::min)
    }

    fn sample(&self, u: f64, v: f64) -> Option<Sample> {
        let q = [self.lo[0] + u * (self.hi[0] - self.lo[0]), self.lo[1] + v * (self.hi[1] - self.lo[1])];
        if !self.contains(q) {
            return None;
        }
        let origin = self.base + self.u * q[0] + self.v * q[1];
        Some(Sample { frame: Frame { origin, x: self.u, y: self.v, z: self.n }, border: self.boundary_distance(q) })
    }

    fn distance_to(&self, p: V3) -> f64 {
        let d = p - self.base;
        let height = d.dot(self.n);
        let q = [d.dot(self.u), d.dot(self.v)];
        if self.contains(q) { height.abs() } else { height.hypot(self.boundary_distance(q)) }
    }
}

struct Frame2 {
    base: V3,
    u: V3,
    v: V3,
}

/// A closed chain of traced edges.
struct Loop {
    points: Vec<V3>,
    /// The straight edges in it, as (start, end).
    straight: Vec<(V3, V3)>,
}

/// Join traced edges end to end into closed loops. Chains that do not close are dropped.
fn chain_loops(mut edges: Vec<Traced>) -> Vec<Loop> {
    let mut loops = Vec::new();
    while let Some(first) = edges.pop() {
        let straight_of = |t: &Traced| if t.straight { t.start().zip(t.end()) } else { None };
        let mut lp = Loop { straight: straight_of(&first).into_iter().collect(), points: first.points };
        let (Some(start), Some(mut tail)) = (lp.points.first().copied(), lp.points.last().copied()) else { continue };
        let mut closed = start.dist(tail) < TOL;
        while !closed {
            let next = edges.iter().position(|e| e.start().is_some_and(|p| p.dist(tail) < TOL) || e.end().is_some_and(|p| p.dist(tail) < TOL));
            let Some(i) = next else { break };
            let mut e = edges.swap_remove(i);
            if e.start().is_some_and(|p| p.dist(tail) >= TOL) {
                e.points.reverse();
            }
            if let Some(s) = straight_of(&e) {
                lp.straight.push(s);
            }
            lp.points.extend(e.points.into_iter().skip(1));
            match lp.points.last() {
                Some(t) => tail = *t,
                None => break,
            }
            closed = start.dist(tail) < TOL;
        }
        if closed && lp.points.len() >= 3 {
            loops.push(lp);
        }
    }
    loops
}

/// Signed area of a 2D polygon (shoelace).
fn area(poly: &[[f64; 2]]) -> f64 {
    let n = poly.len();
    (0..n).filter_map(|i| Some((poly.get(i)?, poly.get((i + 1) % n)?))).map(|(a, b)| a[0] * b[1] - b[0] * a[1]).sum::<f64>() * 0.5
}

/// Even-odd test; the polygon may repeat its first point at the end.
pub fn point_in_polygon(q: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let n = poly.len();
    let mut inside = false;
    for i in 0..n {
        let (Some(a), Some(b)) = (poly.get(i), poly.get((i + 1) % n)) else { continue };
        if (a[1] > q[1]) != (b[1] > q[1]) {
            let x = a[0] + (q[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if q[0] < x {
                inside = !inside;
            }
        }
    }
    inside
}

fn ring_distance(poly: &[[f64; 2]], q: [f64; 2]) -> f64 {
    let n = poly.len();
    let lift = |p: &[f64; 2]| V3::new(p[0], p[1], 0.0);
    let q3 = lift(&q);
    (0..n)
        .filter_map(|i| Some((poly.get(i)?, poly.get((i + 1) % n)?)))
        .map(|(a, b)| segment_distance(lift(a), lift(b), q3))
        .fold(f64::INFINITY, f64::min)
}

/// A cylindrical face, taken as the patch between its lowest and highest edge points along the
/// axis and, unless it goes all the way round, between the angles its edges reach.
#[derive(Clone, Debug)]
pub struct CylinderFace {
    origin: V3,
    axis: V3,
    e1: V3,
    e2: V3,
    radius: f64,
    /// Start angle and sweep (radians); a full turn when `sweep` is TAU.
    theta0: f64,
    sweep: f64,
    h0: f64,
    h1: f64,
    /// +1 for the normal pointing away from the axis, -1 towards it.
    side: f64,
}

impl CylinderFace {
    fn build(c: &CylinderInfo, edges: &[EdgeInfo], flip: bool) -> Option<CylinderFace> {
        let origin = V3::from_array(c.origin);
        let axis = V3::from_array(c.axis).norm()?;
        let radius = c.radius;
        if !radius.is_finite() || radius <= TOL {
            return None;
        }
        let radial = |p: V3| (p - origin).reject(axis).len();
        let on = |p: [f64; 3]| (radial(V3::from_array(p)) - radius).abs() < TOL * 10.0;
        let mine: Vec<&EdgeInfo> = edges.iter().filter(|e| on(e.mid) && e.start.is_some_and(on) && e.end.is_some_and(on)).collect();
        let traced: Vec<Traced> = mine.iter().filter_map(|e| trace_edge(e, Some(axis))).collect();
        if traced.is_empty() {
            return None;
        }
        // Angle zero at a straight edge (the seam, or a side of a partial cylinder) if there is one.
        let seam = traced.iter().find(|t| t.straight).and_then(Traced::start);
        let e1 = seam.and_then(|p| (p - origin).reject(axis).norm()).or_else(|| X.reject(axis).norm()).or_else(|| any_perpendicular(axis))?;
        let e2 = axis.cross(e1);
        let pts: Vec<V3> = traced.iter().flat_map(|t| t.points.iter().copied()).collect();
        let (mut h0, mut h1) = (f64::INFINITY, f64::NEG_INFINITY);
        let mut angles: Vec<f64> = Vec::with_capacity(pts.len());
        for p in &pts {
            let d = *p - origin;
            let h = d.dot(axis);
            h0 = h0.min(h);
            h1 = h1.max(h);
            angles.push(d.dot(e2).atan2(d.dot(e1)).rem_euclid(TAU));
        }
        if !(h1 - h0).is_finite() || h1 - h0 <= TOL {
            return None;
        }
        angles.sort_by(f64::total_cmp);
        let full_circle = mine.iter().any(|e| match (e.start, e.end) {
            (Some(s), Some(t)) => V3::from_array(s).dist(V3::from_array(t)) < TOL && (e.length - TAU * radius).abs() < 0.01 * radius,
            _ => false,
        });
        // The largest gap between edge angles is where the face is not.
        let (mut gap, mut gap_end) = (0.0_f64, 0.0_f64);
        for (i, a) in angles.iter().enumerate() {
            let next = angles.get(i + 1).copied().unwrap_or_else(|| angles.first().copied().unwrap_or(0.0) + TAU);
            if next - a > gap {
                gap = next - a;
                gap_end = next.rem_euclid(TAU);
            }
        }
        let (theta0, sweep) = if full_circle || gap < 5f64.to_radians() { (0.0, TAU) } else { (gap_end, TAU - gap) };
        Some(CylinderFace { origin, axis, e1, e2, radius, theta0, sweep, h0, h1, side: if flip { -1.0 } else { 1.0 } })
    }

    fn dir(&self, theta: f64) -> V3 {
        self.e1 * theta.cos() + self.e2 * theta.sin()
    }

    fn full(&self) -> bool {
        self.sweep >= TAU - 1e-9
    }

    fn sample(&self, u: f64, v: f64) -> Option<Sample> {
        let theta = self.theta0 + u * self.sweep;
        let h = self.h0 + v * (self.h1 - self.h0);
        let r = self.dir(theta);
        let origin = self.origin + self.axis * h + r * self.radius;
        let tangent = self.axis.cross(r);
        let z = r * self.side;
        let mut border = (h - self.h0).min(self.h1 - h);
        if !self.full() {
            border = border.min(self.radius * (u * self.sweep)).min(self.radius * ((1.0 - u) * self.sweep));
        }
        Some(Sample { frame: Frame { origin, x: tangent, y: z.cross(tangent), z }, border })
    }

    fn distance_to(&self, p: V3) -> f64 {
        let d = p - self.origin;
        let h = d.dot(self.axis).clamp(self.h0, self.h1);
        let a = d.dot(self.e2).atan2(d.dot(self.e1)).rem_euclid(TAU);
        let theta = if self.full() {
            a
        } else {
            let rel = (a - self.theta0).rem_euclid(TAU);
            if rel <= self.sweep {
                a
            } else if rel - self.sweep < TAU - rel {
                self.theta0 + self.sweep
            } else {
                self.theta0
            }
        };
        let on = self.origin + self.axis * h + self.dir(theta) * self.radius;
        on.dist(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(s: [f64; 3], m: [f64; 3], e: [f64; 3]) -> EdgeInfo {
        EdgeInfo { mid: m, length: V3::from_array(s).dist(V3::from_array(e)), start: Some(s), end: Some(e) }
    }

    /// The top face of a 40 x 20 box at z = 10, with a 5 mm hole at (10, 10).
    fn plate() -> (FaceInfo, Vec<EdgeInfo>) {
        let z = 10.0;
        let mut edges = vec![
            edge([0.0, 0.0, z], [20.0, 0.0, z], [40.0, 0.0, z]),
            edge([40.0, 0.0, z], [40.0, 10.0, z], [40.0, 20.0, z]),
            edge([40.0, 20.0, z], [20.0, 20.0, z], [0.0, 20.0, z]),
            edge([0.0, 20.0, z], [0.0, 10.0, z], [0.0, 0.0, z]),
            // Bottom edges, which must be ignored.
            edge([0.0, 0.0, 0.0], [20.0, 0.0, 0.0], [40.0, 0.0, 0.0]),
        ];
        edges.push(EdgeInfo { mid: [5.0, 10.0, z], length: TAU * 5.0, start: Some([15.0, 10.0, z]), end: Some([15.0, 10.0, z]) });
        let face = FaceInfo { index: 0, name: Some("top".into()), centroid: [20.0, 10.0, z], plane_normal: Some([0.0, 0.0, 1.0]), point: Some([30.0, 5.0, z]), cylinder: None };
        (face, edges)
    }

    #[test]
    fn plane_outline_and_hole() {
        let (face, edges) = plate();
        let s = build(&face, &edges, false).unwrap();
        let Surface::Plane(p) = &s else { panic!("plane expected") };
        assert_eq!(p.holes.len(), 1);
        // u runs along a 40 mm edge.
        assert!(p.u.x.abs() > 0.99);
        let mid = s.sample(0.5, 0.5).unwrap();
        assert!((mid.frame.origin.z - 10.0).abs() < 1e-9);
        assert!((mid.frame.z.z - 1.0).abs() < 1e-9);
        // Nearest boundary: the hole, 10 mm away with a 5 mm radius.
        assert!((mid.border - 5.0).abs() < 1e-6, "{}", mid.border);
        // The hole centre is off the face.
        let hole_u = if p.u.x > 0.0 { 10.0 / 40.0 } else { 30.0 / 40.0 };
        assert!(s.sample(hole_u, 0.5).is_none());
        assert!((s.distance_to(V3::new(20.0, 10.0, 15.0)) - 5.0).abs() < 1e-9);
        assert!((s.distance_to(V3::new(43.0, 10.0, 14.0)) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn cylinder_full_turn() {
        let r = 5.0;
        let edges = vec![
            EdgeInfo { mid: [-r, 0.0, 0.0], length: TAU * r, start: Some([r, 0.0, 0.0]), end: Some([r, 0.0, 0.0]) },
            EdgeInfo { mid: [-r, 0.0, 30.0], length: TAU * r, start: Some([r, 0.0, 30.0]), end: Some([r, 0.0, 30.0]) },
            edge([r, 0.0, 0.0], [r, 0.0, 15.0], [r, 0.0, 30.0]),
        ];
        let face = FaceInfo { index: 2, name: None, centroid: [0.0, 0.0, 15.0], plane_normal: None, point: Some([r, 0.0, 15.0]), cylinder: Some(CylinderInfo { origin: [0.0, 0.0, 0.0], axis: [0.0, 0.0, 1.0], radius: r }) };
        let s = build(&face, &edges, false).unwrap();
        let q = s.sample(0.25, 0.5).unwrap();
        assert!((q.frame.origin.dist(V3::new(0.0, r, 15.0))) < 1e-9);
        assert!((q.frame.z.dist(Y)) < 1e-9);
        assert!((q.border - 15.0).abs() < 1e-9);
        assert!((s.distance_to(V3::new(10.0, 0.0, 15.0)) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn arcs_bulge_the_right_way() {
        let t = trace_edge(&edge([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]), None).unwrap();
        assert!(!t.straight);
        assert!(t.points.iter().all(|p| (p.len() - 1.0).abs() < 1e-9 && p.y >= -1e-9));
        let t = trace_edge(&edge([1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [-1.0, 0.0, 0.0]), None).unwrap();
        assert!(t.points.iter().all(|p| p.y <= 1e-9));
    }
}
