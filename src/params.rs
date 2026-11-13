//! The entry parameters, as JSON, and their checks.

use serde::Deserialize;

/// The most engine commands SolidCraft 0.41 lets one plugin run make.
pub const MAX_CALLS: usize = 256;
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
    pub keep_input: bool,
    /// How many steps scale and colour take between near and far. Each step costs a prepared
    /// copy of the body, and a run has 256 engine calls in all.
    #[serde(default = "default_levels")]
    pub levels: u32,
}

fn default_levels() -> u32 {
    12
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
#[derive(Debug, Default, Deserialize)]
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
    let flat: Flat = serde_json::from_str(if json.trim().is_empty() { "{}" } else { json }).map_err(|e| format!("bad parameters: {e}"))?;
    let p = flat.into_params()?;
    check(&p)?;
    Ok(p)
}

/// The entry inputs as SolidCraft's plugin dialog draws them: numbers, whole numbers, check
/// boxes, choices and text. Points are text "x, y, z" and lists are text separated by ";".
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flat {
    pub body: String,
    pub face_body: String,
    pub face: String,
    #[serde(default)]
    pub attractor_points: Option<String>,
    #[serde(default)]
    pub attractor_lines: Option<String>,
    #[serde(default)]
    pub attractor_edges: Option<String>,
    #[serde(default)]
    pub attractor_faces: Option<String>,
    #[serde(default)]
    pub attractor_bodies: Option<String>,
    #[serde(default)]
    pub grid: GridKind,
    #[serde(default)]
    pub u_count: Option<u32>,
    #[serde(default)]
    pub v_count: Option<u32>,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub seed: Option<u32>,
    #[serde(default)]
    pub grid_rotation: Option<f64>,
    #[serde(default)]
    pub border: Option<f64>,
    #[serde(default)]
    pub scale_near: Option<f64>,
    #[serde(default)]
    pub scale_far: Option<f64>,
    #[serde(default)]
    pub scale_near_xyz: Option<String>,
    #[serde(default)]
    pub scale_far_xyz: Option<String>,
    #[serde(default)]
    pub depth_near: Option<f64>,
    #[serde(default)]
    pub depth_far: Option<f64>,
    #[serde(default)]
    pub rotation_axis: Axis,
    #[serde(default)]
    pub rotation_near: Option<f64>,
    #[serde(default)]
    pub rotation_far: Option<f64>,
    #[serde(default)]
    pub color_near: Option<String>,
    #[serde(default)]
    pub color_far: Option<String>,
    #[serde(default)]
    pub alpha_near: Option<f64>,
    #[serde(default)]
    pub alpha_far: Option<f64>,
    #[serde(default)]
    pub range_near: Option<f64>,
    #[serde(default)]
    pub range_far: Option<f64>,
    #[serde(default)]
    pub cutoff: Option<f64>,
    #[serde(default)]
    pub waves: Option<f64>,
    #[serde(default)]
    pub decay: Option<f64>,
    #[serde(default)]
    pub operation: Operation,
    #[serde(default)]
    pub merge_scope: Option<String>,
    #[serde(default)]
    pub keep_input: bool,
    #[serde(default)]
    pub levels: Option<u32>,
    #[serde(default)]
    pub flip_body: bool,
    #[serde(default)]
    pub flip_normal: bool,
    #[serde(default)]
    pub body_origin: Option<String>,
    #[serde(default)]
    pub body_z: Option<String>,
    #[serde(default)]
    pub body_x: Option<String>,
}

/// "x, y, z" (commas or spaces between the numbers).
pub fn point(field: &str, text: &str) -> Result<[f64; 3], String> {
    let nums: Vec<f64> = text.split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()).map(str::parse).collect::<Result<_, _>>().map_err(|_| format!("`{field}`: `{}` is not a point like 10, 20, 0", text.trim()))?;
    match nums.as_slice() {
        [x, y, z] if nums.iter().all(|n| n.is_finite()) => Ok([*x, *y, *z]),
        _ => Err(format!("`{field}`: `{}` is not a point like 10, 20, 0", text.trim())),
    }
}

/// The non-empty items of a ";"-separated list.
fn items(text: &Option<String>) -> Vec<&str> {
    text.as_deref().map(|t| t.split(';').map(str::trim).filter(|s| !s.is_empty()).collect()).unwrap_or_default()
}

/// "Body @ rest", split at the last "@".
fn on_body<'a>(field: &str, item: &'a str) -> Result<(&'a str, &'a str), String> {
    item.rsplit_once('@').map(|(b, r)| (b.trim(), r.trim())).filter(|(b, r)| !b.is_empty() && !r.is_empty()).ok_or_else(|| format!("`{field}`: `{item}` should be Body @ x, y, z"))
}

fn face_ref(field: &str, text: &str) -> Result<FaceRef, String> {
    let looks_numeric = text.trim().starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+' || c == '.');
    if looks_numeric { point(field, text).map(FaceRef::Point) } else { Ok(FaceRef::Name(text.trim().to_owned())) }
}

/// "#rrggbb", "r, g, b" (0-255) or "hsl(h, s, l)" (degrees, 0-1, 0-1).
pub fn colour(field: &str, text: &str) -> Result<Colour, String> {
    let t = text.trim();
    let bad = || format!("`{field}`: `{t}` is not a colour like #ff8000, 255, 128, 0 or hsl(30, 1, 0.5)");
    if let Some(inner) = t.strip_prefix("hsl(").and_then(|r| r.strip_suffix(')')) {
        let [h, s, l] = point(field, inner).map_err(|_| bad())?;
        return Ok(Colour::Hsl { h, s, l });
    }
    if t.contains(',') {
        return point(field, t).map(Colour::Rgb).map_err(|_| bad());
    }
    Ok(Colour::Hex(t.to_owned()))
}

impl Flat {
    pub fn into_params(self) -> Result<Params, String> {
        let mut attractors = Vec::new();
        for it in items(&self.attractor_points) {
            attractors.push(AttractorRef::Point(point("attractor_points", it)?));
        }
        for it in items(&self.attractor_lines) {
            let (a, b) = it.split_once(" to ").ok_or_else(|| format!("`attractor_lines`: `{it}` should be x, y, z to x, y, z"))?;
            attractors.push(AttractorRef::Tagged(AttractorTagged { line: Some([point("attractor_lines", a)?, point("attractor_lines", b)?]), ..AttractorTagged::default() }));
        }
        for it in items(&self.attractor_edges) {
            let (b, at) = on_body("attractor_edges", it)?;
            attractors.push(AttractorRef::Tagged(AttractorTagged { edge: Some(point("attractor_edges", at)?), body: Some(b.to_owned()), ..AttractorTagged::default() }));
        }
        for it in items(&self.attractor_faces) {
            let (b, at) = on_body("attractor_faces", it)?;
            attractors.push(AttractorRef::Tagged(AttractorTagged { face: Some(face_ref("attractor_faces", at)?), body: Some(b.to_owned()), ..AttractorTagged::default() }));
        }
        for it in items(&self.attractor_bodies) {
            attractors.push(AttractorRef::Tagged(AttractorTagged { body: Some(it.to_owned()), ..AttractorTagged::default() }));
        }
        if attractors.is_empty() {
            return Err("`attractor_points`: give at least one attractor (points, lines, edges, faces or bodies)".into());
        }

        let d = Grid::default();
        let grid = Grid {
            kind: self.grid,
            u_count: self.u_count.unwrap_or(d.u_count),
            v_count: self.v_count.unwrap_or(d.v_count),
            count: self.count.unwrap_or(d.count),
            seed: self.seed.unwrap_or(d.seed),
            rotation: self.grid_rotation.unwrap_or(d.rotation),
            border: self.border.unwrap_or(d.border),
        };

        let three = |field: &str, t: &Option<String>| t.as_deref().map(|t| point(field, t)).transpose();
        let near_xyz = three("scale_near_xyz", &self.scale_near_xyz)?;
        let far_xyz = three("scale_far_xyz", &self.scale_far_xyz)?;
        let any_scale = self.scale_near.is_some() || self.scale_far.is_some() || near_xyz.is_some() || far_xyz.is_some();
        let scale = any_scale.then(|| ScaleParam {
            near: near_xyz.map(Scale3::PerAxis).unwrap_or(Scale3::Uniform(self.scale_near.unwrap_or(1.0))),
            far: far_xyz.map(Scale3::PerAxis).unwrap_or(Scale3::Uniform(self.scale_far.unwrap_or(1.0))),
        });

        let pair = |name: &str, near: Option<f64>, far: Option<f64>, dn: f64, df: f64| match (near, far) {
            (None, None) => Ok(None),
            (n, f) if name == "range" && (n.is_none() || f.is_none()) => Err("`range_near` and `range_far` go together".to_owned()),
            (n, f) => Ok(Some(NearFar { near: n.unwrap_or(dn), far: f.unwrap_or(df) })),
        };
        let depth = pair("depth", self.depth_near, self.depth_far, 0.0, 0.0)?;
        let range = pair("range", self.range_near, self.range_far, 0.0, 0.0)?;
        let rotation = (self.rotation_near.is_some() || self.rotation_far.is_some())
            .then(|| RotationParam { axis: self.rotation_axis, near: self.rotation_near.unwrap_or(0.0), far: self.rotation_far.unwrap_or(0.0) });
        let color = match (&self.color_near, &self.color_far) {
            (None, None) => None,
            (n, f) => Some(ColorParam {
                near: n.as_deref().map(|t| colour("color_near", t)).transpose()?.unwrap_or_else(white),
                far: f.as_deref().map(|t| colour("color_far", t)).transpose()?.unwrap_or_else(teal),
                near_alpha: self.alpha_near.unwrap_or(255.0),
                far_alpha: self.alpha_far.unwrap_or(255.0),
            }),
        };
        let falloff = (self.waves.is_some() || self.decay.is_some()).then(|| Falloff { kind: FalloffKind::Wave, waves: self.waves.unwrap_or(0.5), decay: self.decay });
        let body_frame = match (&self.body_origin, &self.body_z, &self.body_x) {
            (None, None, None) => None,
            (o, z, x) => Some(FrameParam {
                origin: three("body_origin", o)?.unwrap_or_default(),
                z_dir: three("body_z", z)?,
                x_dir: three("body_x", x)?,
            }),
        };
        let merge: Vec<String> = items(&self.merge_scope).into_iter().map(str::to_owned).collect();
        Ok(Params {
            face: face_ref("face", &self.face)?,
            body: self.body.trim().to_owned(),
            face_body: self.face_body.trim().to_owned(),
            flip_normal: self.flip_normal,
            body_frame,
            flip_body: self.flip_body,
            attractors,
            grid,
            scale,
            depth,
            rotation,
            color,
            range,
            cutoff: self.cutoff,
            falloff,
            operation: self.operation,
            merge_scope: (!merge.is_empty()).then_some(merge),
            keep_input: self.keep_input,
            levels: self.levels.unwrap_or_else(default_levels),
        })
    }
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
    within("levels", p.levels.into(), 2.0, 64.0)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: &str = r#"{"body":"Seed","face":"1, 2, 3","face_body":"Plate","attractor_points":"0,0,0"}"#;

    #[test]
    fn minimal_parses_with_defaults() {
        let p = parse(MIN).unwrap();
        assert_eq!(p.grid.u_count, 5);
        assert_eq!(p.operation, Operation::New);
        assert!(!p.keep_input && p.scale.is_none() && p.color.is_none());
        assert!(matches!(p.face, FaceRef::Point([1.0, 2.0, 3.0])));
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            r#"{"body":"Seed","face":"1,2,3","face_body":"Plate"}"#,
            r#"{"body":"Seed","face":"1,2,3","face_body":"Plate","attractor_points":"0,0"}"#,
            r#"{"body":"Seed","face":"1,2,3","face_body":"Plate","attractor_points":"0,0,0","u_count":0}"#,
            r#"{"body":"Seed","face":"1,2,3","face_body":"Plate","attractor_points":"0,0,0","scale_near":0}"#,
            r#"{"body":"Seed","face":"1,2,3","face_body":"Plate","attractor_points":"0,0,0","merge_scope":"A"}"#,
            r#"{"body":"Seed","face":"1,2,3","face_body":"Plate","attractor_edges":"0,0,0"}"#,
            r#"{"body":"Seed","face":"1,2,3","face_body":"Plate","attractor_points":"0,0,0","range_near":3}"#,
            r##"{"body":"Seed","face":"1,2,3","face_body":"Plate","attractor_points":"0,0,0","color_near":"#ggg"}"##,
            r#"{"body":"Seed","face":"1,2,3","face_body":"Plate","attractor_points":"0,0,0","u_count":100000,"v_count":100000}"#,
            r#"{"body":"Seed","face":"1,2,3","face_body":"Plate","attractor_points":"0,0,0","colour":"red"}"#,
            "[]",
            "",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn reads_every_text_form() {
        let p = parse(r##"{"body":"Seed","face":"F1:top","face_body":"Plate",
            "attractor_points":"0,0,0; 1 1 1","attractor_lines":"0,0,0 to 1,0,0","attractor_edges":"B @ 0,0,0",
            "attractor_faces":"B @ F2:side; B @ 1,2,3","attractor_bodies":"Ball; Rod 2",
            "scale_near_xyz":"1, 1, 2","scale_far":0.5,"color_near":"hsl(10, 1, 0.5)","color_far":"0, 128, 255",
            "merge_scope":"Plate; Base","operation":"join","body_z":"0,0,-1"}"##).unwrap();
        assert_eq!(p.attractors.len(), 8);
        assert!(matches!(p.face, FaceRef::Name(ref n) if n == "F1:top"));
        assert_eq!(p.merge_scope.as_deref(), Some(&["Plate".to_owned(), "Base".to_owned()][..]));
        let s = p.scale.unwrap();
        assert_eq!(s.near.xyz(), [1.0, 1.0, 2.0]);
        assert_eq!(s.far.xyz(), [0.5; 3]);
        assert!(matches!(p.color.unwrap().far, Colour::Rgb(_)));
    }
}
