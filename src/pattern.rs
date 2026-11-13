//! The pattern itself: where each copy goes and how it is changed, then the engine commands
//! that make it.

use crate::attractor::{self, Bodies};
use crate::colour;
use crate::engine::{Engine, created_bodies, run};
use crate::grid;
use crate::params::{Axis, FalloffKind, MAX_INSTANCES, Operation, Params};
use crate::vec::{Frame, M3, Rigid, V3, X, Y, Z};
use serde_json::{Value, json};

/// One copy, worked out but not yet made.
pub struct Instance {
    /// The face's tangent frame at the copy, moved along the normal by `depth`.
    pub frame: Frame,
    /// The point on the face it stands on, before `depth`.
    pub on_face: V3,
    /// Distance to the nearest attractor, after falloff and range.
    pub distance: f64,
    /// Turn about the body frame's chosen axis (degrees).
    pub angle: f64,
    /// Scale along the body frame's axes.
    pub scale: [f64; 3],
    /// "#rrggbb" and opacity 0..1.
    pub colour: Option<(String, f64)>,
}

pub struct Plan {
    pub face_kind: &'static str,
    pub candidates: usize,
    pub off_face: usize,
    pub near_border: usize,
    pub beyond_cutoff: usize,
    /// The distances that map to "near" and "far".
    pub near_far: [f64; 2],
    pub instances: Vec<Instance>,
    pub body_frame: Frame,
}

/// `value` mapped from `[in0, in1]` to `[out0, out1]`, not clamped; `out0` when the input
/// range is empty.
fn remap(value: f64, in0: f64, in1: f64, out0: f64, out1: f64) -> f64 {
    let span = in1 - in0;
    if span.abs() < 1e-12 { out0 } else { out0 + (value - in0) * (out1 - out0) / span }
}

pub fn plan(engine: &mut dyn Engine, p: &Params) -> Result<Plan, String> {
    if !other_bodies(engine, &[])?.contains(&p.body) {
        return Err(format!("no visible body `{}` to pattern", p.body));
    }
    let mut bodies = Bodies::default();
    let face = attractor::find_face(engine, &mut bodies, &p.face_body, &p.face, p.flip_normal)?;
    let attractors = attractor::resolve(engine, &mut bodies, &p.attractors)?;

    let uv = grid::points(&p.grid);
    let candidates = uv.len();
    let (mut off_face, mut near_border) = (0, 0);
    let mut samples = Vec::new();
    for [u, v] in uv {
        let Some(s) = face.sample(u, v) else {
            off_face += 1;
            continue;
        };
        if s.border < p.grid.border {
            near_border += 1;
            continue;
        }
        let d = attractor::distance(&attractors, s.frame.origin);
        if d.is_finite() {
            samples.push((s.frame, d));
        }
    }
    if samples.is_empty() {
        return Err(format!("none of the {candidates} grid points landed on the face ({off_face} off it, {near_border} within the border)"));
    }

    let measured_min = samples.iter().map(|s| s.1).fold(f64::INFINITY, f64::min);
    let measured_max = samples.iter().map(|s| s.1).fold(f64::NEG_INFINITY, f64::max);

    // Wave falloff, as the original: a cosine over the measured distances, its height shrinking
    // by a factor of 100 every `decay` mm when `decay` is given.
    if let Some(f) = &p.falloff {
        match f.kind {
            FalloffKind::Wave => {
                let amplitude = measured_max / 2.0;
                let period = if f.waves > 0.0 { measured_max / f.waves } else { f64::INFINITY };
                for s in &mut samples {
                    let d = s.1;
                    let decay = match f.decay {
                        Some(k) if k != 0.0 => 100f64.powf(-d / k),
                        _ => 1.0,
                    };
                    let phase = if period.is_finite() && period > 0.0 { d * std::f64::consts::TAU / period } else { 0.0 };
                    s.1 = -(amplitude * decay * phase.cos()) + amplitude.abs();
                }
            }
        }
    }

    let [near_d, far_d] = match &p.range {
        Some(r) => [r.near, r.far],
        None => [measured_min, measured_max],
    };

    let base = p.body_frame.as_ref().map_or(Ok(Frame::WORLD), |f| {
        let z = f.z_dir.map(V3::from_array).unwrap_or(Z);
        let x = f.x_dir.map(V3::from_array).unwrap_or(X);
        Frame::from_z_x(V3::from_array(f.origin), z, x).ok_or_else(|| "`body_frame.z_dir` must not be zero".to_owned())
    })?;
    let body_frame = if p.flip_body { Frame { z: -base.z, y: -base.y, ..base } } else { base };

    let colours = match &p.color {
        Some(c) => Some((colour::rgb(&c.near)?, colour::rgb(&c.far)?, c.near_alpha / 255.0, c.far_alpha / 255.0)),
        None => None,
    };

    let mut beyond_cutoff = 0;
    let mut instances = Vec::new();
    for (frame, mut d) in samples {
        if p.cutoff.is_some_and(|c| d > c) {
            beyond_cutoff += 1;
            continue;
        }
        if let Some(r) = &p.range {
            d = d.clamp(r.near, r.far);
        }
        let map = |near: f64, far: f64| remap(d, near_d, far_d, near, far);
        let angle = p.rotation.as_ref().map_or(0.0, |r| map(r.near, r.far));
        let scale = match &p.scale {
            Some(s) => {
                let (n, f) = (s.near.xyz(), s.far.xyz());
                [map(n[0], f[0]).max(1e-6), map(n[1], f[1]).max(1e-6), map(n[2], f[2]).max(1e-6)]
            }
            None => [1.0; 3],
        };
        let depth = p.depth.as_ref().map_or(0.0, |dp| map(dp.near, dp.far));
        let colour = colours.map(|(cn, cf, an, af)| {
            let ch = |i: usize| map(*cn.get(i).unwrap_or(&0.0), *cf.get(i).unwrap_or(&0.0)).clamp(0.0, 1.0);
            (colour::to_hex([ch(0), ch(1), ch(2)]), map(an, af).clamp(0.0, 1.0))
        });
        let on_face = frame.origin;
        let frame = Frame { origin: frame.origin - frame.z * depth, ..frame };
        instances.push(Instance { frame, on_face, distance: d, angle, scale, colour });
    }
    Ok(Plan { face_kind: face.kind(), candidates, off_face, near_border, beyond_cutoff, near_far: [near_d, far_d], instances, body_frame })
}

fn rotation_axis(p: &Params) -> V3 {
    match p.rotation.as_ref().map(|r| r.axis) {
        Some(Axis::X) => X,
        Some(Axis::Y) => Y,
        _ => Z,
    }
}

/// `solid.move` parameters for a rigid motion (rotate about the world origin, then translate).
fn move_params(body: &str, m: &Rigid, copy: bool) -> Value {
    let mut v = json!({ "bodies": [body], "translate": m.t.to_array() });
    if copy {
        set(&mut v, "copy", json!(true));
    }
    if let Some((axis, angle)) = m.r.axis_angle() {
        set(&mut v, "axis", json!(axis.to_array()));
        set(&mut v, "angle", json!(angle.to_degrees()));
    }
    v
}

/// Add a field to a JSON object (a no-op on anything else).
fn set(v: &mut Value, key: &str, value: Value) {
    if let Some(o) = v.as_object_mut() {
        o.insert(key.to_owned(), value);
    }
}

fn is_one(k: f64) -> bool {
    (k - 1.0).abs() < 1e-9
}

/// Make one copy; returns its body name.
fn place(engine: &mut dyn Engine, p: &Params, plan: &Plan, inst: &Instance) -> Result<String, String> {
    let to_body = plan.body_frame.from_world();
    let turn = Rigid { r: M3::rotation(rotation_axis(p), inst.angle.to_radians()), t: crate::vec::ZERO };
    let onto_face = inst.frame.to_world().then_after(&turn);
    let [sx, sy, sz] = inst.scale;
    let copy_of = |reply: &Value| created_bodies(reply).into_iter().next().ok_or_else(|| "solid.move made no copy".to_owned());
    if is_one(sx) && is_one(sy) && is_one(sz) {
        let reply = run(engine, "solid.move", &move_params(&p.body, &onto_face.then_after(&to_body), true))?;
        return copy_of(&reply);
    }
    if (sx - sy).abs() < 1e-12 && (sy - sz).abs() < 1e-12 {
        // A uniform scale commutes with turns, so place first and scale about the point.
        let name = copy_of(&run(engine, "solid.move", &move_params(&p.body, &onto_face.then_after(&to_body), true))?)?;
        run(engine, "solid.scale", &json!({ "bodies": [name], "factor": sx, "origin": inst.frame.origin.to_array() }))?;
        return Ok(name);
    }
    // Scale along the body frame's axes: bring the copy to the world frame, scale there, then place it.
    let name = copy_of(&run(engine, "solid.move", &move_params(&p.body, &to_body, true))?)?;
    run(engine, "solid.scale", &json!({ "bodies": [name], "factors": [sx, sy, sz], "origin": [0.0, 0.0, 0.0] }))?;
    if !onto_face.is_identity() {
        run(engine, "solid.move", &move_params(&name, &onto_face, false))?;
    }
    Ok(name)
}

/// Visible bodies other than the ones named.
fn other_bodies(engine: &mut dyn Engine, except: &[String]) -> Result<Vec<String>, String> {
    let reply = run(engine, "document.inspect", &json!({}))?;
    Ok(reply
        .get("bodies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|b| b.get("visible").and_then(Value::as_bool).unwrap_or(true) && b.get("removed_later").and_then(Value::as_bool) != Some(true))
        .filter_map(|b| b.get("name").and_then(Value::as_str))
        .filter(|n| !except.iter().any(|e| e == n))
        .map(str::to_owned)
        .collect())
}

fn summary(plan: &Plan) -> Value {
    json!({
        "face": plan.face_kind,
        "grid_points": plan.candidates,
        "culled": { "off_face": plan.off_face, "near_border": plan.near_border, "beyond_cutoff": plan.beyond_cutoff },
        "near_far_mm": plan.near_far,
    })
}

fn warnings(n: usize) -> Vec<String> {
    match n {
        0 => vec!["no copies: every point was culled".into()],
        501..=1000 => vec![format!("{n} copies: expect a slow rebuild")],
        1001.. => vec![format!("{n} copies, each a timeline feature: rebuilds will be very slow")],
        _ => Vec::new(),
    }
}

/// The `preview` entry: where the copies would go, without changing the design.
pub fn preview(engine: &mut dyn Engine, p: &Params) -> Result<Value, String> {
    let plan = plan(engine, p)?;
    let points: Vec<Value> = plan
        .instances
        .iter()
        .take(5000)
        .map(|i| json!({ "at": i.on_face.to_array(), "normal": i.frame.z.to_array(), "distance": i.distance, "angle": i.angle, "scale": i.scale, "color": i.colour.as_ref().map(|c| &c.0) }))
        .collect();
    let mut out = summary(&plan);
    set(&mut out, "count", json!(plan.instances.len()));
    set(&mut out, "points", json!(points));
    let mut w = warnings(plan.instances.len());
    if plan.instances.len() > MAX_INSTANCES {
        w.push(format!("`pattern` makes at most {MAX_INSTANCES} copies; reduce the grid or use a cutoff"));
    }
    set(&mut out, "warnings", json!(w));
    Ok(out)
}

/// The `pattern` entry.
pub fn pattern(engine: &mut dyn Engine, p: &Params) -> Result<Value, String> {
    let plan = plan(engine, p)?;
    let n = plan.instances.len();
    if n > MAX_INSTANCES {
        return Err(format!("{n} copies is more than the {MAX_INSTANCES} one run makes; reduce the grid, raise the border or use a cutoff"));
    }
    engine.log(&format!("placing {n} copies on the {}", plan.face_kind));
    let mut made = Vec::with_capacity(n);
    for inst in &plan.instances {
        let name = place(engine, p, &plan, inst)?;
        if let Some((hex, opacity)) = &inst.colour {
            run(engine, "appearance.assign", &json!({ "bodies": [name], "color": hex, "opacity": opacity }))?;
        }
        made.push(name);
    }

    let mut bodies = made.clone();
    if let Some(prefix) = &p.name_prefix {
        bodies.clear();
        for (i, name) in made.iter().enumerate() {
            let new = format!("{prefix} {}", i + 1);
            run(engine, "body.rename", &json!({ "body": name, "name": new }))?;
            bodies.push(new);
        }
    }

    let mut targets = Vec::new();
    if p.operation != Operation::New && !made.is_empty() {
        let mut except = made.clone();
        except.push(p.body.clone());
        targets = match &p.merge_scope {
            Some(list) => list.clone(),
            None => other_bodies(engine, &except)?,
        };
        let Some((first, rest)) = targets.split_first() else {
            return Err(format!("nothing to {} with: no other visible bodies", p.operation.as_str()));
        };
        match p.operation {
            Operation::Join => {
                let tools: Vec<&String> = rest.iter().chain(made.iter()).collect();
                run(engine, "solid.combine", &json!({ "target": first, "tools": tools, "operation": "join" }))?;
            }
            Operation::Cut | Operation::Intersect => {
                for (i, t) in targets.iter().enumerate() {
                    let last = i + 1 == targets.len();
                    run(engine, "solid.combine", &json!({ "target": t, "tools": made, "operation": p.operation.as_str(), "keep_tools": !last }))?;
                }
            }
            Operation::New => {}
        }
        bodies.clear();
    }

    if !p.keep_input {
        run(engine, "solid.remove", &json!({ "bodies": [p.body] }))?;
    }

    let mut out = summary(&plan);
    set(&mut out, "instances", json!(n));
    set(&mut out, "bodies", json!(bodies));
    set(&mut out, "merged_into", json!(targets));
    set(&mut out, "warnings", json!(warnings(n)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remap_handles_an_empty_range() {
        assert_eq!(remap(5.0, 2.0, 2.0, 1.0, 9.0), 1.0);
        assert_eq!(remap(3.0, 2.0, 4.0, 10.0, 20.0), 15.0);
    }

    #[test]
    fn move_params_round_trip() {
        let m = Rigid { r: M3::rotation(Y, 1.0), t: V3::new(1.0, 2.0, 3.0) };
        let v = move_params("B", &m, true);
        assert_eq!(v["copy"], json!(true));
        let axis = V3::from_array([v["axis"][0].as_f64().unwrap(), v["axis"][1].as_f64().unwrap(), v["axis"][2].as_f64().unwrap()]);
        let back = M3::rotation(axis, v["angle"].as_f64().unwrap().to_radians());
        assert!(back.apply(X).dist(m.r.apply(X)) < 1e-9);
    }
}
