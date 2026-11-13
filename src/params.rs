//! The entry parameters, as JSON, and their checks.

use serde::Deserialize;

/// The most copies one run makes. Each copy costs one to three timeline features, and the host
/// allows 10 000 engine commands per transaction.
pub const MAX_INSTANCES: usize = 1500;
/// The most grid points looked at before culling.
pub const MAX_CANDIDATES: u64 = 4_000_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// The body to copy over the face.
    pub body: String,
    /// The face to pattern on: a point on it, or its name from `model.faces`.
    pub face: FaceRef,
    /// The body that owns `face`.
    pub face_body: String,
    /// Turn the face's normal round (copies stand on the other side).
    #[serde(default)]
    pub flip_normal: bool,
    /// The body's own frame, which is laid onto each point of the face. Default: the world.
    #[serde(default)]
    pub body_frame: Option<FrameParam>,
    /// Turn the body over before placing it.
    #[serde(default)]
    pub flip_body: bool,
    /// What the copies react to.
    pub attractors: Vec<AttractorRef>,
    #[serde(default)]
    pub grid: Grid,
    #[serde(default)]
    pub scale: Option<ScaleParam>,
    #[serde(default)]
    pub depth: Option<NearFar>,
    #[serde(default)]
    pub rotation: Option<RotationParam>,
    #[serde(default)]
    pub color: Option<ColorParam>,
    #[serde(default)]
    pub range: Option<NearFar>,
    #[serde(default)]
    pub cutoff: Option<f64>,
    #[serde(default)]
    pub falloff: Option<Falloff>,
    #[serde(default)]
    pub operation: Operation,
    #[serde(default)]
    pub merge_scope: Option<Vec<String>>,
    #[serde(default)]
    pub name_prefix: Option<String>,
    #[serde(default)]
    pub keep_input: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum FaceRef {
    Point([f64; 3]),
    Name(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameParam {
    #[serde(default)]
    pub origin: [f64; 3],
    #[serde(default)]
    pub z_dir: Option<[f64; 3]>,
    #[serde(default)]
    pub x_dir: Option<[f64; 3]>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum AttractorRef {
    Point([f64; 3]),
    Tagged(AttractorTagged),
}

/// One of `point`, `line`, `edge` (with `body`), `face` (with `body`), or `body` alone.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttractorTagged {
    #[serde(default)]
    pub point: Option<[f64; 3]>,
    #[serde(default)]
    pub line: Option<[[f64; 3]; 2]>,
    #[serde(default)]
    pub edge: Option<[f64; 3]>,
    #[serde(default)]
    pub face: Option<FaceRef>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Grid {
    #[serde(rename = "type")]
    pub kind: GridKind,
    pub u_count: u32,
    pub v_count: u32,
    /// Random grids: how many points.
    pub count: u32,
    /// Random grids: the seed.
    pub seed: u32,
    /// Degrees, counter-clockwise seen from the side the normal points to.
    pub rotation: f64,
    /// Points closer than this to the face's edges are dropped (mm).
    pub border: f64,
}

impl Default for Grid {
    fn default() -> Self {
        Grid { kind: GridKind::Rectangular, u_count: 5, v_count: 5, count: 50, seed: 1, rotation: 0.0, border: 0.0 }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GridKind {
    #[default]
    Rectangular,
    Triangular,
    Random,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScaleParam {
    #[serde(default = "one")]
    pub near: Scale3,
    #[serde(default = "one")]
    pub far: Scale3,
}

fn one() -> Scale3 {
    Scale3::Uniform(1.0)
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(untagged)]
pub enum Scale3 {
    Uniform(f64),
    PerAxis([f64; 3]),
}

impl Scale3 {
    pub fn xyz(self) -> [f64; 3] {
        match self {
            Scale3::Uniform(k) => [k; 3],
            Scale3::PerAxis(a) => a,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NearFar {
    pub near: f64,
    pub far: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RotationParam {
    #[serde(default)]
    pub axis: Axis,
    /// Degrees.
    #[serde(default = "thirty")]
    pub near: f64,
    #[serde(default)]
    pub far: f64,
}

fn thirty() -> f64 {
    30.0
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    X,
    Y,
    #[default]
    Z,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorParam {
    #[serde(default = "white")]
    pub near: Colour,
    #[serde(default = "teal")]
    pub far: Colour,
    #[serde(default = "opaque")]
    pub near_alpha: f64,
    #[serde(default = "opaque")]
    pub far_alpha: f64,
}

fn white() -> Colour {
    Colour::Hex("#FFFFFF".into())
}
fn teal() -> Colour {
    Colour::Hex("#00F0F0".into())
}
fn opaque() -> f64 {
    255.0
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Colour {
    /// "#rrggbb" or "rrggbb".
    Hex(String),
    /// 0-255 each.
    Rgb([f64; 3]),
    /// Hue in degrees, saturation and lightness 0-1.
    Hsl { h: f64, s: f64, l: f64 },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Falloff {
    #[serde(rename = "type", default)]
    pub kind: FalloffKind,
    #[serde(default = "half")]
    pub waves: f64,
    /// mm; without it the waves keep their height.
    #[serde(default)]
    pub decay: Option<f64>,
}

fn half() -> f64 {
    0.5
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FalloffKind {
    #[default]
    Wave,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    /// The copies stay separate bodies.
    #[default]
    New,
    /// Joined to the merge scope.
    Join,
    /// Cut from the merge scope.
    Cut,
    /// The merge scope keeps only what the copies overlap.
    Intersect,
}

impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Operation::New => "new",
            Operation::Join => "join",
            Operation::Cut => "cut",
            Operation::Intersect => "intersect",
        }
    }
}

pub fn parse(json: &str) -> Result<Params, String> {
    if json.len() > 1 << 20 {
        return Err("the parameters are larger than 1 MiB".into());
    }
    let p: Params = serde_json::from_str(if json.trim().is_empty() { "{}" } else { json }).map_err(|e| format!("bad parameters: {e}"))?;
    check(&p)?;
    Ok(p)
}

fn finite(name: &str, v: f64) -> Result<f64, String> {
    if v.is_finite() { Ok(v) } else { Err(format!("`{name}` must be a finite number")) }
}

fn within(name: &str, v: f64, lo: f64, hi: f64) -> Result<(), String> {
    finite(name, v)?;
    if v < lo || v > hi { Err(format!("`{name}` must be between {lo} and {hi}, not {v}")) } else { Ok(()) }
}

fn finite3(name: &str, v: [f64; 3]) -> Result<(), String> {
    v.iter().try_for_each(|x| finite(name, *x).map(|_| ()))
}

fn check(p: &Params) -> Result<(), String> {
    if p.body.is_empty() || p.face_body.is_empty() {
        return Err("`body` and `face_body` must name bodies".into());
    }
    if let FaceRef::Point(pt) = p.face {
        finite3("face", pt)?;
    }
    if p.attractors.is_empty() || p.attractors.len() > 256 {
        return Err("`attractors` must list 1 to 256 attractors".into());
    }
    for a in &p.attractors {
        match a {
            AttractorRef::Point(pt) => finite3("attractors", *pt)?,
            AttractorRef::Tagged(t) => {
                let given = [t.point.is_some(), t.line.is_some(), t.edge.is_some(), t.face.is_some()].iter().filter(|b| **b).count();
                if given > 1 || (given == 0 && t.body.is_none()) {
                    return Err("each attractor is a point [x,y,z], or one of {point}, {line}, {edge, body}, {face, body} or {body}".into());
                }
                if (t.edge.is_some() || t.face.is_some()) && t.body.is_none() {
                    return Err("an `edge` or `face` attractor needs the `body` it is on".into());
                }
                if (t.point.is_some() || t.line.is_some()) && t.body.is_some() {
                    return Err("a `point` or `line` attractor takes no `body`".into());
                }
                for pt in t.point.iter().chain(t.edge.iter()).chain(t.line.iter().flatten()) {
                    finite3("attractors", *pt)?;
                }
            }
        }
    }
    let g = &p.grid;
    within("grid.u_count", g.u_count.into(), 1.0, 100_000.0)?;
    within("grid.v_count", g.v_count.into(), 1.0, 100_000.0)?;
    within("grid.count", g.count.into(), 0.0, 10_000.0)?;
    within("grid.rotation", g.rotation, -360.0, 360.0)?;
    within("grid.border", g.border, 0.0, 100_000.0)?;
    if g.kind != GridKind::Random && (2 * u64::from(g.u_count) - 1) * (2 * u64::from(g.v_count) - 1) > MAX_CANDIDATES {
        return Err(format!("`grid.u_count` x `grid.v_count` is too large; keep it under about {}", MAX_CANDIDATES / 4));
    }
    if let Some(f) = &p.body_frame {
        finite3("body_frame.origin", f.origin)?;
        for d in f.z_dir.iter().chain(f.x_dir.iter()) {
            finite3("body_frame", *d)?;
        }
    }
    if let Some(s) = &p.scale {
        for k in s.near.xyz().into_iter().chain(s.far.xyz()) {
            within("scale", k, 1e-8, 10_000.0)?;
        }
    }
    if let Some(d) = &p.depth {
        within("depth.near", d.near, -1e6, 1e6)?;
        within("depth.far", d.far, -1e6, 1e6)?;
    }
    if let Some(r) = &p.rotation {
        within("rotation.near", r.near, -100_000.0, 100_000.0)?;
        within("rotation.far", r.far, -100_000.0, 100_000.0)?;
    }
    if let Some(c) = &p.color {
        within("color.near_alpha", c.near_alpha, 0.0, 255.0)?;
        within("color.far_alpha", c.far_alpha, 0.0, 255.0)?;
        crate::colour::rgb(&c.near)?;
        crate::colour::rgb(&c.far)?;
    }
    if let Some(r) = &p.range {
        within("range.near", r.near, 0.0, 1e6)?;
        within("range.far", r.far, 0.0, 1e6)?;
        if r.far <= r.near {
            return Err("`range.far` must be greater than `range.near`".into());
        }
    }
    if let Some(c) = p.cutoff {
        within("cutoff", c, 0.0, 1e6)?;
    }
    if let Some(f) = &p.falloff {
        within("falloff.waves", f.waves, 0.0, 1e6)?;
        if let Some(d) = f.decay {
            within("falloff.decay", d, -1e6, 1e6)?;
        }
    }
    match (&p.merge_scope, p.operation) {
        (Some(_), Operation::New) => return Err("`merge_scope` needs `operation` join, cut or intersect".into()),
        (Some(list), _) if list.is_empty() || list.len() > 1000 => return Err("`merge_scope` must list 1 to 1000 bodies".into()),
        _ => {}
    }
    if let Some(n) = &p.name_prefix {
        if p.operation != Operation::New {
            return Err("`name_prefix` names separate copies, so it needs `operation` new".into());
        }
        if n.trim().is_empty() || n.chars().count() > 64 || n.chars().any(char::is_control) {
            return Err("`name_prefix` must be 1 to 64 printable characters".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: &str = r#"{"body":"Seed","face":[1,2,3],"face_body":"Plate","attractors":[[0,0,0]]}"#;

    #[test]
    fn minimal_parses_with_defaults() {
        let p = parse(MIN).unwrap();
        assert_eq!(p.grid.u_count, 5);
        assert_eq!(p.operation, Operation::New);
        assert!(!p.keep_input);
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            r#"{"body":"Seed","face":[1,2,3],"face_body":"Plate","attractors":[]}"#,
            r#"{"body":"Seed","face":[1,2,3],"face_body":"Plate","attractors":[[0,0,0]],"grid":{"u_count":0}}"#,
            r#"{"body":"Seed","face":[1,2,3],"face_body":"Plate","attractors":[[0,0,0]],"scale":{"near":0}}"#,
            r#"{"body":"Seed","face":[1,2,3],"face_body":"Plate","attractors":[[0,0,0]],"merge_scope":["A"]}"#,
            r#"{"body":"Seed","face":[1,2,3],"face_body":"Plate","attractors":[{"edge":[0,0,0]}]}"#,
            r##"{"body":"Seed","face":[1,2,3],"face_body":"Plate","attractors":[[0,0,0]],"colour":"#fff"}"##,
            r##"{"body":"Seed","face":[1,2,3],"face_body":"Plate","attractors":[[0,0,0]],"color":{"near":"#ggg"}}"##,
            r#"{"body":"Seed","face":[1,2,3],"face_body":"Plate","attractors":[[0,0,0]],"grid":{"u_count":100000,"v_count":100000}}"#,
            "[]",
            "",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn accepts_every_attractor_form() {
        let p = parse(r#"{"body":"Seed","face":"F1:top","face_body":"Plate","attractors":[[0,0,0],{"point":[1,1,1]},{"line":[[0,0,0],[1,0,0]]},{"edge":[0,0,0],"body":"B"},{"face":[0,0,0],"body":"B"},{"body":"B"}]}"#).unwrap();
        assert_eq!(p.attractors.len(), 6);
    }
}
