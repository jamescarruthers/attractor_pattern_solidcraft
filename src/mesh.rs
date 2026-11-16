//! Curved faces of any kind, from SolidCraft's read-only body mesh (`model.mesh`).
//!
//! The mesh lists each face of the body as its own connected set of triangles, so the face
//! under a point is the set that holds the nearest triangle. Points on it are placed by area
//! and spread evenly (farthest-point sampling), and lifted from the flat triangles towards the
//! curved surface with Phong tessellation, which uses the normals at the triangle's corners.

use crate::params::{Grid, GridKind};
use crate::surface::Sample;
use crate::vec::{Frame, M3, V3, X, Y, Z, any_perpendicular};
use serde::Deserialize;
use std::collections::BTreeMap;

/// What `model.mesh` returns.
#[derive(Debug, Deserialize)]
pub struct MeshReply {
    pub positions: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
    #[serde(default)]
    pub component: Option<u64>,
}

/// The chord tolerance to ask for: about 2500 vertices for a body of this surface area (the
/// engine refuses meshes over 8192 vertices, and one refused call ends the run), never finer
/// than 0.02 mm.
pub fn tolerance_for_area(area: f64) -> f64 {
    if !area.is_finite() || area <= 0.0 { 0.05 } else { (2.0 * area.sqrt() / 2500.0).clamp(0.02, 50.0) }
}

/// Phong tessellation's shape factor: 0 keeps the flat triangle, 1 follows the normals fully.
const PHONG: f64 = 0.75;

#[derive(Clone, Debug)]
pub struct MeshFace {
    pts: Vec<V3>,
    nrm: Vec<V3>,
    tris: Vec<[usize; 3]>,
    /// Running total of triangle areas, for picking triangles by area.
    cumulative: Vec<f64>,
    boundary: Vec<(V3, V3)>,
    /// The direction copies' x axes run around (the face's axis of least spread).
    axis: V3,
    centroid: V3,
    side: f64,
    pub tolerance: f64,
}

fn tri_normal(a: V3, b: V3, c: V3) -> V3 {
    (b - a).cross(c - a)
}

/// The closest point to `p` on triangle `abc` (Ericson, Real-Time Collision Detection 5.1.5).
fn closest_on_triangle(p: V3, a: V3, b: V3, c: V3) -> V3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = va + vb + vc;
    if denom.abs() < 1e-300 {
        return a;
    }
    let v = vb / denom;
    let w = vc / denom;
    a + ab * v + ac * w
}

/// Distance from `p` to the nearest of the triangles.
pub fn triangles_distance(pts: &[V3], tris: &[[usize; 3]], p: V3) -> f64 {
    tris.iter()
        .filter_map(|t| Some((pts.get(t[0])?, pts.get(t[1])?, pts.get(t[2])?)))
        .map(|(a, b, c)| closest_on_triangle(p, *a, *b, *c).dist(p))
        .fold(f64::INFINITY, f64::min)
}

/// A mesh as points and triangles, its indices checked.
pub struct Soup {
    pub pts: Vec<V3>,
    pub tris: Vec<[usize; 3]>,
}

pub fn soup(m: &MeshReply) -> Result<Soup, String> {
    let pts: Vec<V3> = m.positions.iter().map(|p| V3::from_array(*p)).collect();
    if pts.iter().any(|p| !p.is_finite()) {
        return Err("model.mesh gave a point that is not a number".into());
    }
    let n = pts.len();
    let tris: Vec<[usize; 3]> = m.triangles.iter().map(|t| t.map(|i| i as usize)).collect();
    if tris.iter().flatten().any(|i| *i >= n) {
        return Err("model.mesh gave a triangle with a corner out of range".into());
    }
    if tris.is_empty() {
        return Err("model.mesh gave no triangles".into());
    }
    Ok(Soup { pts, tris })
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while let Some(&p) = parent.get(x) {
        if p == x {
            break;
        }
        let gp = parent.get(p).copied().unwrap_or(p);
        if let Some(slot) = parent.get_mut(x) {
            *slot = gp;
        }
        x = gp;
    }
    x
}

impl MeshFace {
    /// The face of the mesh nearest `pick`, and how far `pick` is from it.
    pub fn build(m: &MeshReply, pick: V3, flip: bool, tolerance: f64) -> Result<(MeshFace, f64), String> {
        let s = soup(m)?;
        // The faces: sets of triangles joined by shared corner indices.
        let mut parent: Vec<usize> = (0..s.pts.len()).collect();
        for t in &s.tris {
            let (a, b, c) = (find(&mut parent, t[0]), find(&mut parent, t[1]), find(&mut parent, t[2]));
            for (x, y) in [(a, b), (b, c)] {
                let (rx, ry) = (find(&mut parent, x), find(&mut parent, y));
                if let Some(slot) = parent.get_mut(rx) {
                    *slot = ry;
                }
            }
        }
        let (nearest, dist) = s
            .tris
            .iter()
            .enumerate()
            .filter_map(|(i, t)| Some((i, closest_on_triangle(pick, *s.pts.get(t[0])?, *s.pts.get(t[1])?, *s.pts.get(t[2])?).dist(pick))))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .ok_or("the mesh has no triangles")?;
        let root = s.tris.get(nearest).map(|t| find(&mut parent, t[0])).ok_or("the mesh has no triangles")?;
        let mine: Vec<[usize; 3]> = s.tris.iter().copied().filter(|t| find(&mut parent, t[0]) == root).collect();

        // Weld points that sit on the same spot (the seam of a closed face), so the seam is
        // neither an edge of the face nor a crease in its normals.
        let mut index: BTreeMap<[i64; 3], usize> = BTreeMap::new();
        let mut remap: BTreeMap<usize, usize> = BTreeMap::new();
        let mut pts: Vec<V3> = Vec::new();
        let key = |p: V3| [(p.x * 1e5).round() as i64, (p.y * 1e5).round() as i64, (p.z * 1e5).round() as i64];
        for i in mine.iter().flatten() {
            if remap.contains_key(i) {
                continue;
            }
            let Some(p) = s.pts.get(*i) else { continue };
            let k = key(*p);
            let w = *index.entry(k).or_insert_with(|| {
                pts.push(*p);
                pts.len() - 1
            });
            remap.insert(*i, w);
        }
        let tris: Vec<[usize; 3]> = mine
            .iter()
            .filter_map(|t| Some([*remap.get(&t[0])?, *remap.get(&t[1])?, *remap.get(&t[2])?]))
            .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
            .collect();
        if tris.is_empty() {
            return Err("the face's mesh has no triangles with area".into());
        }

        // Corner normals: area-weighted means of the triangles round each point.
        let mut nrm = vec![crate::vec::ZERO; pts.len()];
        let mut cumulative = Vec::with_capacity(tris.len());
        let mut total = 0.0;
        let mut weighted = crate::vec::ZERO;
        for t in &tris {
            let (Some(a), Some(b), Some(c)) = (pts.get(t[0]), pts.get(t[1]), pts.get(t[2])) else { continue };
            let n = tri_normal(*a, *b, *c);
            let area = n.len() / 2.0;
            for i in t {
                if let Some(slot) = nrm.get_mut(*i) {
                    *slot = *slot + n;
                }
            }
            total += area;
            weighted = weighted + (*a + *b + *c) * (area / 3.0);
            cumulative.push(total);
        }
        if total <= 0.0 {
            return Err("the face's mesh has no area".into());
        }
        let nrm: Vec<V3> = nrm.into_iter().map(|n| n.norm().unwrap_or(Z)).collect();
        let centroid = weighted * (1.0 / total);

        // Edges used by one triangle are the face's boundary.
        let mut uses: BTreeMap<(usize, usize), u32> = BTreeMap::new();
        for t in &tris {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *uses.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        let boundary = uses.iter().filter(|(_, n)| **n == 1).filter_map(|((a, b), _)| Some((*pts.get(*a)?, *pts.get(*b)?))).collect();

        Ok((MeshFace { axis: least_spread(&pts, centroid), pts, nrm, tris, cumulative, boundary, centroid, side: if flip { -1.0 } else { 1.0 }, tolerance }, dist))
    }

    fn corners(&self, t: usize) -> Option<([V3; 3], [V3; 3])> {
        let tri = self.tris.get(t)?;
        let p = [*self.pts.get(tri[0])?, *self.pts.get(tri[1])?, *self.pts.get(tri[2])?];
        let n = [*self.nrm.get(tri[0])?, *self.nrm.get(tri[1])?, *self.nrm.get(tri[2])?];
        Some((p, n))
    }

    /// The point at barycentric `b` on triangle `t`, lifted towards the curved surface, and
    /// the normal there.
    fn point_at(&self, t: usize, b: [f64; 3]) -> Option<(V3, V3)> {
        let (p, n) = self.corners(t)?;
        let flat = p[0] * b[0] + p[1] * b[1] + p[2] * b[2];
        let onto = |q: V3, m: V3| flat - m * (flat - q).dot(m);
        let curved = onto(p[0], n[0]) * b[0] + onto(p[1], n[1]) * b[1] + onto(p[2], n[2]) * b[2];
        let at = flat * (1.0 - PHONG) + curved * PHONG;
        let normal = (n[0] * b[0] + n[1] * b[1] + n[2] * b[2]).norm()?;
        Some((at, normal))
    }

    pub fn boundary_distance(&self, p: V3) -> f64 {
        self.boundary.iter().map(|(a, b)| crate::surface::segment_distance(*a, *b, p)).fold(f64::INFINITY, f64::min)
    }

    pub fn distance_to(&self, p: V3) -> f64 {
        triangles_distance(&self.pts, &self.tris, p)
    }

    /// A copy's frame at a point: `z` along the normal, `x` round the face's axis, turned by
    /// the grid rotation.
    fn frame(&self, at: V3, normal: V3, rotation_deg: f64) -> Frame {
        let z = normal * self.side;
        let x = self.axis.cross(normal).norm().or_else(|| X.reject(z).norm()).or_else(|| Y.reject(z).norm()).or_else(|| any_perpendicular(z)).unwrap_or(X);
        let x = M3::rotation(z, rotation_deg.to_radians()).apply(x);
        Frame { origin: at, x, y: z.cross(x), z }
    }

    /// Points spread over the face: `count` random ones for a random grid, otherwise as many
    /// as the grid would put on a flat face, spread evenly. Points nearer the face's edges than
    /// `grid.border` are not used. Returns the points and how many candidates the border took.
    pub fn samples(&self, grid: &Grid) -> (Vec<Sample>, usize) {
        let mut state = if grid.kind == GridKind::Random { grid.seed } else { 1 };
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            f64::from(state) / (f64::from(u32::MAX) + 1.0)
        };
        let total = self.cumulative.last().copied().unwrap_or(0.0);
        let draw = |next: &mut dyn FnMut() -> f64| -> Option<(V3, V3)> {
            let target = next() * total;
            let t = self.cumulative.partition_point(|c| *c < target).min(self.tris.len().saturating_sub(1));
            let (r1, r2) = (next().sqrt(), next());
            self.point_at(t, [1.0 - r1, r1 * (1.0 - r2), r1 * r2])
        };
        let wanted = match grid.kind {
            GridKind::Random => grid.count as usize,
            _ => crate::grid::points(grid).len(),
        }
        .min(3000);
        let pool = match grid.kind {
            GridKind::Random => wanted,
            _ => (wanted * 15).clamp(500, 20_000),
        };
        let mut near_border = 0;
        let mut candidates: Vec<(V3, V3, f64)> = Vec::with_capacity(pool);
        for _ in 0..pool {
            let Some((at, n)) = draw(&mut next) else { continue };
            let border = self.boundary_distance(at);
            if border < grid.border {
                near_border += 1;
                continue;
            }
            candidates.push((at, n, border));
        }
        let picked: Vec<(V3, V3, f64)> = if grid.kind == GridKind::Random { candidates } else { spread(candidates, wanted, self.centroid) };
        let samples = picked.into_iter().map(|(at, n, border)| Sample { frame: self.frame(at, n, grid.rotation), border }).collect();
        (samples, if grid.kind == GridKind::Random { near_border } else { 0 })
    }
}

/// Farthest-point sampling: `n` of the candidates, each as far as it can be from those already
/// picked, starting from the one nearest `start`.
fn spread(candidates: Vec<(V3, V3, f64)>, n: usize, start: V3) -> Vec<(V3, V3, f64)> {
    if candidates.len() <= n {
        return candidates;
    }
    let mut gap = vec![f64::INFINITY; candidates.len()];
    let mut out = Vec::with_capacity(n);
    let mut pick = candidates.iter().enumerate().min_by(|a, b| a.1.0.dist(start).total_cmp(&b.1.0.dist(start))).map(|(i, _)| i).unwrap_or(0);
    for _ in 0..n {
        let Some(chosen) = candidates.get(pick).copied() else { break };
        out.push(chosen);
        let mut best = (0, -1.0);
        for (i, c) in candidates.iter().enumerate() {
            if let Some(g) = gap.get_mut(i) {
                *g = g.min(c.0.dist(chosen.0));
                if *g > best.1 {
                    best = (i, *g);
                }
            }
        }
        pick = best.0;
    }
    out
}

/// The direction in which the points spread least (the axis of a ring or a disc), by power
/// iteration on the inverted covariance's largest eigenvector.
fn least_spread(pts: &[V3], centroid: V3) -> V3 {
    let mut c = [[0.0f64; 3]; 3];
    for p in pts {
        let d = (*p - centroid).to_array();
        for (i, row) in c.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v += d.get(i).copied().unwrap_or(0.0) * d.get(j).copied().unwrap_or(0.0);
            }
        }
    }
    let m = M3 { c0: V3::from_array(c[0]), c1: V3::from_array(c[1]), c2: V3::from_array(c[2]) };
    // Largest eigenvector of (trace * I - C) is the smallest of C.
    let tr = c[0][0] + c[1][1] + c[2][2];
    let shifted = M3 { c0: X * tr - m.c0, c1: Y * tr - m.c1, c2: Z * tr - m.c2 };
    let mut v = V3::new(0.3, 0.5, 0.8);
    for _ in 0..64 {
        match shifted.apply(v).norm() {
            Some(n) => v = n,
            None => return Z,
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit sphere of radius `r` as a UV mesh, with a duplicated seam like the engine's.
    fn sphere(r: f64) -> MeshReply {
        let (nu, nv) = (32, 16);
        let mut positions = Vec::new();
        for j in 0..=nv {
            let phi = std::f64::consts::PI * j as f64 / nv as f64;
            for i in 0..=nu {
                let th = std::f64::consts::TAU * i as f64 / nu as f64;
                positions.push([r * phi.sin() * th.cos(), r * phi.sin() * th.sin(), r * phi.cos()]);
            }
        }
        let mut triangles = Vec::new();
        let id = |i: usize, j: usize| (j * (nu + 1) + i) as u32;
        for j in 0..nv {
            for i in 0..nu {
                triangles.push([id(i, j), id(i, j + 1), id(i + 1, j + 1)]);
                triangles.push([id(i, j), id(i + 1, j + 1), id(i + 1, j)]);
            }
        }
        MeshReply { positions, triangles, component: Some(0) }
    }

    #[test]
    fn sphere_points_sit_on_the_sphere_facing_out() {
        let (f, d) = MeshFace::build(&sphere(10.0), V3::new(0.0, 0.0, 10.0), false, 0.1).unwrap();
        assert!(d < 0.5);
        // A closed sphere has no boundary once the seam and poles are welded.
        assert!(f.boundary.is_empty(), "{}", f.boundary.len());
        let grid = Grid { u_count: 6, v_count: 6, ..Grid::default() };
        let (s, _) = f.samples(&grid);
        assert!(s.len() >= 20, "{}", s.len());
        for p in &s {
            let r = p.frame.origin.len();
            assert!((r - 10.0).abs() < 0.1, "radius {r}");
            assert!(p.frame.z.dot(p.frame.origin * 0.1) > 0.95);
            assert!((p.frame.x.cross(p.frame.y).dot(p.frame.z) - 1.0).abs() < 1e-9);
        }
        // Evenly spread: no two points much closer than the typical spacing.
        let min_gap = s.iter().enumerate().flat_map(|(i, a)| s.iter().skip(i + 1).map(move |b| a.frame.origin.dist(b.frame.origin))).fold(f64::INFINITY, f64::min);
        assert!(min_gap > 3.0, "{min_gap}");
        assert!((f.distance_to(V3::new(0.0, 0.0, 15.0)) - 5.0).abs() < 0.1);
    }

    #[test]
    fn random_grids_are_seeded() {
        let (f, _) = MeshFace::build(&sphere(10.0), V3::new(10.0, 0.0, 0.0), false, 0.1).unwrap();
        let g = Grid { kind: GridKind::Random, count: 40, seed: 7, ..Grid::default() };
        let a: Vec<V3> = f.samples(&g).0.iter().map(|s| s.frame.origin).collect();
        let b: Vec<V3> = f.samples(&g).0.iter().map(|s| s.frame.origin).collect();
        assert_eq!(a.len(), 40);
        assert_eq!(a, b);
    }

    #[test]
    fn tolerance_keeps_meshes_small() {
        assert!((tolerance_for_area(3927.0) - 0.05).abs() < 0.01);
        assert!(tolerance_for_area(1.13e6) > 0.8);
        assert_eq!(tolerance_for_area(0.0), 0.05);
    }
}
