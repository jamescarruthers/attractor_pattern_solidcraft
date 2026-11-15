//! Inputs from the design's selection, for whatever the form left empty.
//!
//! Select the face to pattern on first, then the body to copy (the body, or any face or edge
//! of it), then any attractors, and run the entry with those fields empty.

use crate::engine::{Engine, run};
use crate::params::Flat;
use serde_json::{Value, json};

/// One selected item, as `document.inspect` lists it.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    Face { body: String, point: [f64; 3] },
    Edge { body: String, point: [f64; 3] },
    Body(String),
    /// Anything else that has a point in space.
    Point([f64; 3]),
    Other(String),
}

impl Pick {
    fn body(&self) -> Option<&str> {
        match self {
            Pick::Face { body, .. } | Pick::Edge { body, .. } => Some(body),
            Pick::Body(b) => Some(b),
            _ => None,
        }
    }
}

fn point_of(v: &Value) -> Option<[f64; 3]> {
    let a = v.get("point")?.as_array()?;
    match a.as_slice() {
        [x, y, z] => Some([x.as_f64()?, y.as_f64()?, z.as_f64()?]),
        _ => None,
    }
}

pub fn picks(selection: &Value) -> Vec<Pick> {
    selection
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            let kind = item.get("type").and_then(Value::as_str).unwrap_or("");
            let body = item.get("body").and_then(Value::as_str).map(str::to_owned);
            match (kind, body, point_of(item)) {
                ("face", Some(body), Some(point)) => Pick::Face { body, point },
                ("edge", Some(body), Some(point)) => Pick::Edge { body, point },
                ("body", _, _) => match item.get("name").and_then(Value::as_str) {
                    Some(name) => Pick::Body(name.to_owned()),
                    None => Pick::Other(kind.to_owned()),
                },
                (_, _, Some(point)) => Pick::Point(point),
                _ => Pick::Other(kind.to_owned()),
            }
        })
        .collect()
}

fn xyz(p: [f64; 3]) -> String {
    format!("{}, {}, {}", p[0], p[1], p[2])
}

fn empty(field: &Option<String>) -> bool {
    field.as_deref().is_none_or(|t| t.trim().is_empty())
}

/// Whether the form left anything for the selection to fill.
pub fn wanted(flat: &Flat) -> bool {
    empty(&flat.body) || empty(&flat.face) || empty(&flat.face_body) || no_attractors(flat)
}

fn no_attractors(flat: &Flat) -> bool {
    [&flat.attractor_points, &flat.attractor_lines, &flat.attractor_edges, &flat.attractor_faces, &flat.attractor_bodies].iter().all(|f| empty(f))
}

/// Fill the empty inputs from the picks. Returns what each pick was used for.
pub fn apply(flat: &mut Flat, picks: Vec<Pick>) -> Value {
    let mut left = picks;
    let mut used = serde_json::Map::new();
    let take = |left: &mut Vec<Pick>, test: &dyn Fn(&Pick) -> bool| left.iter().position(test).map(|i| left.remove(i));

    if empty(&flat.face)
        && let Some(Pick::Face { body, point }) = take(&mut left, &|p| matches!(p, Pick::Face { .. }))
    {
        used.insert("face".into(), json!(format!("{body} @ {}", xyz(point))));
        flat.face = Some(xyz(point));
        if empty(&flat.face_body) {
            flat.face_body = Some(body);
        }
    }
    if empty(&flat.body) {
        let base = flat.face_body.clone().unwrap_or_default();
        // The next pick on another body, whether the body itself or a face or edge of it.
        let seed = take(&mut left, &|p| p.body().is_some_and(|b| b != base));
        if let Some(b) = seed.as_ref().and_then(Pick::body) {
            used.insert("body".into(), json!(b));
            flat.body = Some(b.to_owned());
        }
    }
    if no_attractors(flat) {
        let mut lists: [(&str, Vec<String>); 4] = [("points", Vec::new()), ("edges", Vec::new()), ("faces", Vec::new()), ("bodies", Vec::new())];
        let mut ignored = Vec::new();
        for p in left {
            match p {
                Pick::Point(pt) => lists[0].1.push(xyz(pt)),
                Pick::Edge { body, point } => lists[1].1.push(format!("{body} @ {}", xyz(point))),
                Pick::Face { body, point } => lists[2].1.push(format!("{body} @ {}", xyz(point))),
                Pick::Body(b) => lists[3].1.push(b),
                Pick::Other(kind) => ignored.push(kind),
            }
        }
        let join = |v: &Vec<String>| (!v.is_empty()).then(|| v.join("; "));
        let [(_, pts), (_, edges), (_, faces), (_, bodies)] = &lists;
        flat.attractor_points = join(pts);
        flat.attractor_edges = join(edges);
        flat.attractor_faces = join(faces);
        flat.attractor_bodies = join(bodies);
        let mut a = serde_json::Map::new();
        for (name, list) in &lists {
            if !list.is_empty() {
                a.insert((*name).into(), json!(list));
            }
        }
        if !a.is_empty() {
            used.insert("attractors".into(), Value::Object(a));
        }
        if !ignored.is_empty() {
            used.insert("ignored".into(), json!(ignored));
        }
    }
    Value::Object(used)
}

/// Read the selection when the form left inputs empty, and fill them. Returns how many engine
/// calls that took and what the selection was used for.
pub fn fill(engine: &mut dyn Engine, flat: &mut Flat) -> Result<(usize, Option<Value>), String> {
    if !wanted(flat) {
        return Ok((0, None));
    }
    let reply = run(engine, "document.inspect", &json!({}))?;
    let picks = picks(reply.get("selection").unwrap_or(&Value::Null));
    if picks.is_empty() {
        return Ok((1, None));
    }
    Ok((1, Some(apply(flat, picks))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(json: &str) -> Flat {
        crate::params::parse_flat(json).unwrap()
    }

    fn sel() -> Value {
        json!([
            {"type": "face", "body": "Plate", "index": 0, "point": [50.0, 30.0, 10.0]},
            {"type": "face", "body": "Pin", "index": 1, "point": [0.0, 0.0, 6.0]},
            {"type": "edge", "body": "Plate", "index": 1, "point": [50.0, 0.0, 10.0]},
            {"type": "body", "name": "Orb"},
            {"type": "feature", "id": 3}
        ])
    }

    #[test]
    fn everything_from_the_selection() {
        let mut f = flat("{}");
        let used = apply(&mut f, picks(&sel()));
        assert_eq!(f.face.as_deref(), Some("50, 30, 10"));
        assert_eq!(f.face_body.as_deref(), Some("Plate"));
        assert_eq!(f.body.as_deref(), Some("Pin"));
        assert_eq!(f.attractor_edges.as_deref(), Some("Plate @ 50, 0, 10"));
        assert_eq!(f.attractor_bodies.as_deref(), Some("Orb"));
        assert_eq!(used["ignored"], json!(["feature"]));
        let p = crate::params::finish(f).unwrap();
        assert_eq!(p.attractors.len(), 2);
    }

    #[test]
    fn the_next_pick_on_another_body_is_the_one_to_copy() {
        let mut f = flat("{}");
        apply(&mut f, picks(&json!([{"type": "face", "body": "Plate", "point": [1.0, 2.0, 10.0]}, {"type": "body", "name": "Pin"}, {"type": "face", "body": "Orb", "point": [0.0, 0.0, 1.0]}])));
        assert_eq!(f.body.as_deref(), Some("Pin"));
        assert_eq!(f.attractor_faces.as_deref(), Some("Orb @ 0, 0, 1"));
    }

    #[test]
    fn typed_fields_win() {
        let mut f = flat(r#"{"body": "Seed", "attractor_points": "0, 0, 0"}"#);
        apply(&mut f, picks(&sel()));
        assert_eq!(f.body.as_deref(), Some("Seed"));
        assert_eq!(f.face_body.as_deref(), Some("Plate"));
        assert_eq!(f.attractor_points.as_deref(), Some("0, 0, 0"));
        assert!(f.attractor_bodies.is_none());
    }

    #[test]
    fn nothing_selected_and_nothing_typed_says_what_is_missing() {
        let mut f = flat("{}");
        apply(&mut f, Vec::new());
        let e = crate::params::finish(f).unwrap_err();
        assert!(e.contains("select"), "{e}");
    }
}
