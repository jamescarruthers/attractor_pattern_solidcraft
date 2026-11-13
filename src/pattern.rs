//! The pattern itself: where each copy goes and how it is changed, then the engine commands
//! that make it.
//!
//! SolidCraft 0.41 allows 256 engine calls per plugin run, so copies are made from a few
//! prepared seeds. The distance is cut into `levels` steps; each step that is used gets one
//! copy of the body, scaled and coloured for that step and moved into its own frame. Every
//! copy on the face is then one `solid.move` from its step's seed, with its exact turn and
//! depth. The seeds are removed at the end.

use crate::attractor::{self, Bodies};
use crate::colour;
use crate::engine::{Engine, created_bodies, run};
use crate::grid;
use crate::params::{Axis, FalloffKind, MAX_CALLS, Operation, Params};
use crate::vec::{Frame, M3, Rigid, V3, X, Y, Z, ZERO};
use serde_json::{Value, json};
use std::collections::BTreeMap;

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
    /// Which scale and colour step it takes, when scale or colour is on.
    pub level: Option<i64>,
}

/// What one step of scale and colour does.
#[derive(Clone, Debug)]
pub struct Level {
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
    /// The steps the copies use, by index.
    pub levels: BTreeMap<i64, Level>,
    pub body_frame: Frame,
    /// Visible bodies before the run, apart from the one being copied.
    pub others: Vec<String>,
    /// Engine calls made while planning.
    pub calls_so_far: usize,
}

/// `value` mapped from `[in0, in1]` to `[out0, out1]`, not clamped; `out0` when the input
/// range is empty.
fn remap(value: f64, in0: f64, in1: f64, out0: f64, out1: f64) -> f64 {
    let span = in1 - in0;
    if span.abs() < 1e-12 { out0 } else { out0 + (value - in0) * (out1 - out0) / span }
}

/// Counts the engine calls a plan makes.
struct Counting<'a> {
    inner: &'a mut dyn Engine,
    calls: usize,
}

impl Engine for Counting<'_> {
    fn run(&mut self, command: &str, args: &Value) -> Result<Value, String> {
        self.calls += 1;
        self.inner.run(command, args)
    }
}

fn visible_bodies(engine: &mut dyn Engine) -> Result<Vec<String>, String> {
    let reply = run(engine, "document.inspect", &json!({}))?;
    Ok(reply
        .get("bodies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|b| b.get("visible").and_then(Value::as_bool).unwrap_or(true) && b.get("removed_later").and_then(Value::as_bool) != Some(true))
        .filter_map(|b| b.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect())
}

pub fn plan(engine: &mut dyn Engine, p: &Params) -> Result<Plan, String> {
    let mut engine = Counting { inner: engine, calls: 0 };
    let engine = &mut engine;
    let mut others = visible_bodies(engine)?;
    let Some(at) = others.iter().position(|n| *n == p.body) else {
        return Err(format!("no visible body `{}` to pattern", p.body));
    };
    others.remove(at);

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
    let stepped = p.scale.is_some() || colours.is_some();
    let steps = f64::from(p.levels.max(2) - 1);

    let mut beyond_cutoff = 0;
    let mut instances = Vec::new();
    let mut levels = BTreeMap::new();
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
        let depth = p.depth.as_ref().map_or(0.0, |dp| map(dp.near, dp.far));
        let level = stepped.then(|| (remap(d, near_d, far_d, 0.0, 1.0) * steps).round() as i64);
        if let Some(i) = level {
            levels.entry(i).or_insert_with(|| {
                // The step's own distance, so every copy in it looks the same.
                let t = i as f64 / steps;
                let at = |near: f64, far: f64| near + t * (far - near);
                let scale = match &p.scale {
                    Some(s) => {
                        let (n, f) = (s.near.xyz(), s.far.xyz());
                        [at(n[0], f[0]).max(1e-6), at(n[1], f[1]).max(1e-6), at(n[2], f[2]).max(1e-6)]
                    }
                    None => [1.0; 3],
                };
                let colour = colours.map(|(cn, cf, an, af)| {
                    let ch = |k: usize| at(*cn.get(k).unwrap_or(&0.0), *cf.get(k).unwrap_or(&0.0)).clamp(0.0, 1.0);
                    (colour::to_hex([ch(0), ch(1), ch(2)]), at(an, af).clamp(0.0, 1.0))
                });
                Level { scale, colour }
            });
        }
        let on_face = frame.origin;
        let frame = Frame { origin: frame.origin - frame.z * depth, ..frame };
        instances.push(Instance { frame, on_face, distance: d, angle, level });
    }
    let calls_so_far = engine.calls;
    Ok(Plan {
        face_kind: face.kind(),
        candidates,
        off_face,
        near_border,
        beyond_cutoff,
        near_far: [near_d, far_d],
        instances,
        levels,
        body_frame,
        others,
        calls_so_far,
    })
}

fn rotation_axis(p: &Params) -> V3 {
    match p.rotation.as_ref().map(|r| r.axis) {
        Some(Axis::X) => X,
        Some(Axis::Y) => Y,
        _ => Z,
    }
}

/// Add a field to a JSON object (a no-op on anything else).
fn set(v: &mut Value, key: &str, value: Value) {
    if let Some(o) = v.as_object_mut() {
        o.insert(key.to_owned(), value);
    }
}

/// `solid.move` parameters for a rigid motion (rotate about the world origin, then translate).
fn move_params(bodies: &[&str], m: &Rigid, copy: bool) -> Value {
    let mut v = json!({ "bodies": bodies, "translate": m.t.to_array() });
    if copy {
        set(&mut v, "copy", json!(true));
    }
    if let Some((axis, angle)) = m.r.axis_angle() {
        set(&mut v, "axis", json!(axis.to_array()));
        set(&mut v, "angle", json!(angle.to_degrees()));
    }
    v
}

fn is_one(k: f64) -> bool {
    (k - 1.0).abs() < 1e-9
}

/// The cost of a step's seed: the copy, a scale unless it is 1, and a colour if any.
fn level_calls(l: &Level) -> usize {
    1 + usize::from(!l.scale.iter().all(|k| is_one(*k))) + usize::from(l.colour.is_some())
}

/// The merge targets: the given scope, or every other visible body.
fn targets(p: &Params, plan: &Plan) -> Vec<String> {
    match (&p.merge_scope, p.operation) {
        (_, Operation::New) => Vec::new(),
        (Some(list), _) => list.clone(),
        (None, _) => plan.others.clone(),
    }
}

/// The engine calls `pattern` makes after planning.
fn calls_to_make(p: &Params, plan: &Plan) -> usize {
    let seeds: usize = plan.levels.values().map(level_calls).sum();
    let combine = match p.operation {
        Operation::New => 0,
        Operation::Join => 1,
        Operation::Cut | Operation::Intersect => targets(p, plan).len(),
    };
    let remove = usize::from(!p.keep_input || !plan.levels.is_empty());
    seeds + plan.instances.len() + combine + remove
}

fn budget(p: &Params, plan: &Plan) -> Value {
    let total = plan.calls_so_far + calls_to_make(p, plan);
    json!({ "engine_calls": total, "limit": MAX_CALLS, "fits": total <= MAX_CALLS, "levels_used": plan.levels.len() })
}

fn summary(p: &Params, plan: &Plan) -> Value {
    json!({
        "face": plan.face_kind,
        "grid_points": plan.candidates,
        "culled": { "off_face": plan.off_face, "near_border": plan.near_border, "beyond_cutoff": plan.beyond_cutoff },
        "near_far_mm": plan.near_far,
        "budget": budget(p, plan),
    })
}

/// The largest number of copies that would fit, for the error message.
fn copies_that_fit(p: &Params, plan: &Plan) -> usize {
    let fixed = plan.calls_so_far + calls_to_make(p, plan) - plan.instances.len();
    MAX_CALLS.saturating_sub(fixed)
}

/// The `preview` entry: where the copies would go, without changing the design.
pub fn preview(engine: &mut dyn Engine, p: &Params) -> Result<Value, String> {
    let plan = plan(engine, p)?;
    let points: Vec<Value> = plan
        .instances
        .iter()
        .take(2000)
        .map(|i| {
            let level = i.level.and_then(|l| plan.levels.get(&l));
            json!({
                "at": i.on_face.to_array(),
                "normal": i.frame.z.to_array(),
                "distance": i.distance,
                "angle": i.angle,
                "scale": level.map(|l| l.scale),
                "color": level.and_then(|l| l.colour.as_ref().map(|c| &c.0)),
            })
        })
        .collect();
    let mut out = summary(p, &plan);
    set(&mut out, "count", json!(plan.instances.len()));
    set(&mut out, "points", json!(points));
    Ok(out)
}

/// The `pattern` entry.
pub fn pattern(engine: &mut dyn Engine, p: &Params) -> Result<Value, String> {
    let plan = plan(engine, p)?;
    let n = plan.instances.len();
    if n == 0 {
        return Err(format!("no copies to make: all {} points were culled", plan.candidates));
    }
    let total = plan.calls_so_far + calls_to_make(p, &plan);
    if total > MAX_CALLS {
        return Err(format!(
            "{n} copies need {total} engine calls, and SolidCraft allows {MAX_CALLS} per run; about {} copies fit. Use smaller u_count and v_count, a cutoff, a larger border or fewer levels",
            copies_that_fit(p, &plan)
        ));
    }
    let targets = targets(p, &plan);
    if p.operation != Operation::New && targets.is_empty() {
        return Err(format!("nothing to {} with: no other visible bodies", p.operation.as_str()));
    }

    let to_body = plan.body_frame.from_world();
    let first_copy = |reply: &Value| created_bodies(reply).into_iter().next().ok_or_else(|| "solid.move made no copy".to_owned());

    // One seed per step, in the body's own frame (now the world's), scaled and coloured.
    let mut seeds: BTreeMap<i64, String> = BTreeMap::new();
    for (i, level) in &plan.levels {
        let name = first_copy(&run(engine, "solid.move", &move_params(&[&p.body], &to_body, true))?)?;
        let [sx, sy, sz] = level.scale;
        if !(is_one(sx) && is_one(sy) && is_one(sz)) {
            let scale = if (sx - sy).abs() < 1e-12 && (sy - sz).abs() < 1e-12 {
                json!({ "bodies": [name], "factor": sx, "origin": [0.0, 0.0, 0.0] })
            } else {
                json!({ "bodies": [name], "factors": [sx, sy, sz], "origin": [0.0, 0.0, 0.0] })
            };
            run(engine, "solid.scale", &scale)?;
        }
        if let Some((hex, opacity)) = &level.colour {
            run(engine, "appearance.assign", &json!({ "bodies": [name], "color": hex, "opacity": opacity }))?;
        }
        seeds.insert(*i, name);
    }

    let mut made = Vec::with_capacity(n);
    for inst in &plan.instances {
        let turn = Rigid { r: M3::rotation(rotation_axis(p), inst.angle.to_radians()), t: ZERO };
        let onto_face = inst.frame.to_world().then_after(&turn);
        let (source, motion) = match inst.level.and_then(|l| seeds.get(&l)) {
            Some(seed) => (seed.as_str(), onto_face),
            None => (p.body.as_str(), onto_face.then_after(&to_body)),
        };
        made.push(first_copy(&run(engine, "solid.move", &move_params(&[source], &motion, true))?)?);
    }

    match p.operation {
        Operation::New => {}
        Operation::Join => {
            if let Some((first, rest)) = targets.split_first() {
                let tools: Vec<&String> = rest.iter().chain(made.iter()).collect();
                run(engine, "solid.combine", &json!({ "target": first, "tools": tools, "operation": "join" }))?;
            }
        }
        Operation::Cut | Operation::Intersect => {
            for (i, t) in targets.iter().enumerate() {
                let last = i + 1 == targets.len();
                run(engine, "solid.combine", &json!({ "target": t, "tools": made, "operation": p.operation.as_str(), "keep_tools": !last }))?;
            }
        }
    }

    let mut remove: Vec<&str> = seeds.values().map(String::as_str).collect();
    if !p.keep_input {
        remove.push(&p.body);
    }
    if !remove.is_empty() {
        run(engine, "solid.remove", &json!({ "bodies": remove }))?;
    }

    let mut out = summary(p, &plan);
    set(&mut out, "instances", json!(n));
    set(&mut out, "bodies", json!(if p.operation == Operation::New { made } else { Vec::new() }));
    set(&mut out, "merged_into", json!(targets));
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
        let v = move_params(&["B"], &m, true);
        assert_eq!(v["copy"], json!(true));
        let axis = V3::from_array([v["axis"][0].as_f64().unwrap(), v["axis"][1].as_f64().unwrap(), v["axis"][2].as_f64().unwrap()]);
        let back = M3::rotation(axis, v["angle"].as_f64().unwrap().to_radians());
        assert!(back.apply(X).dist(m.r.apply(X)) < 1e-9);
    }
}
