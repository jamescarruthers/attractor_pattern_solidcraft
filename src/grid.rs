//! Where the copies go, as (u, v) parameters on the face, both in 0..1.

use crate::params::{Grid, GridKind};

/// The grid's points inside the unit square.
///
/// Rectangular and triangular grids follow the original feature: a lattice of `2u` by `2v`
/// cells spaced `2 / (2n - 1)`, so it reaches past the square on every side, turned about the
/// square's centre, and clipped to it. That keeps the square filled at any rotation, with
/// about `u` by `v` points inside. Triangular grids shift every other column by half a step.
pub fn points(g: &Grid) -> Vec<[f64; 2]> {
    match g.kind {
        GridKind::Rectangular | GridKind::Triangular => lattice(g),
        GridKind::Random => random(g.count, g.seed),
    }
}

fn lattice(g: &Grid) -> Vec<[f64; 2]> {
    let nu = 2 * g.u_count as usize;
    let nv = 2 * g.v_count as usize;
    let du = 2.0 / (nu as f64 - 1.0);
    let dv = 2.0 / (nv as f64 - 1.0);
    let (s, c) = g.rotation.to_radians().sin_cos();
    let mut out = Vec::new();
    let mut shift = 0.0;
    for i in 1..nu {
        for j in 1..nv {
            let a = du * i as f64 - du / 2.0 - 0.5 - 0.5;
            let b = dv * j as f64 - dv / 2.0 - 0.5 + shift * dv / 2.0 - 0.5;
            let p = [c * a - s * b + 0.5, s * a + c * b + 0.5];
            if p[0] > 0.0 && p[0] < 1.0 && p[1] > 0.0 && p[1] < 1.0 {
                out.push(p);
            }
        }
        if g.kind == GridKind::Triangular {
            shift = 1.0 - shift;
        }
    }
    out
}

/// `count` points from a linear congruential generator (Numerical Recipes constants), so the
/// same seed gives the same layout.
fn random(count: u32, seed: u32) -> Vec<[f64; 2]> {
    let mut state = seed;
    let mut next = || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(state) / (f64::from(u32::MAX) + 1.0)
    };
    (0..count).map(|_| [next(), next()]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(kind: GridKind, u: u32, v: u32, rotation: f64) -> Grid {
        Grid { kind, u_count: u, v_count: v, rotation, ..Grid::default() }
    }

    #[test]
    fn rectangular_counts() {
        // 5 x 5 asks for about 25 points in the square.
        let n = points(&grid(GridKind::Rectangular, 5, 5, 0.0)).len();
        assert!((16..=36).contains(&n), "{n}");
        let one = points(&grid(GridKind::Rectangular, 1, 1, 0.0));
        assert_eq!(one.len(), 1);
    }

    #[test]
    fn rotation_keeps_the_square_filled() {
        let flat = points(&grid(GridKind::Rectangular, 8, 8, 0.0)).len() as f64;
        let turned = points(&grid(GridKind::Rectangular, 8, 8, 30.0)).len() as f64;
        assert!((turned / flat - 1.0).abs() < 0.35, "{flat} {turned}");
    }

    #[test]
    fn random_is_seeded_and_in_range() {
        let a = random(100, 7);
        assert_eq!(a, random(100, 7));
        assert_ne!(a, random(100, 8));
        assert!(a.iter().all(|p| (0.0..1.0).contains(&p[0]) && (0.0..1.0).contains(&p[1])));
    }
}
