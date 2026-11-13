//! Attractors and the bodies they are read from.

use crate::engine::{Engine, run};
use crate::params::{AttractorRef, FaceRef};
use crate::surface::{self, EdgeInfo, FaceInfo, Surface, Traced};
use crate::vec::V3;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

/// Faces and edges of bodies, read once per body.
#[derive(Default)]
pub struct Bodies {
    cache: BTreeMap<String, BodyGeometry>,
}

pub struct BodyGeometry {
    pub faces: Vec<FaceInfo>,
    pub edges: Vec<EdgeInfo>,
}

#[derive(Deserialize)]
struct FacesReply {
    faces: Vec<FaceInfo>,
}

#[derive(Deserialize)]
struct EdgesReply {
    edges: Vec<EdgeInfo>,
}

impl Bodies {
    pub fn get(&mut self, engine: &mut dyn Engine, body: &str) -> Result<&BodyGeometry, String> {
        if !self.cache.contains_key(body) {
            let faces: FacesReply = serde_json::from_value(run(engine, "model.faces", &json!({ "body": body }))?).map_err(|e| format!("model.faces gave an unexpected reply: {e}"))?;
            let edges: EdgesReply = serde_json::from_value(run(engine, "model.edges", &json!({ "body": body }))?).map_err(|e| format!("model.edges gave an unexpected reply: {e}"))?;
            self.cache.insert(body.to_owned(), BodyGeometry { faces: faces.faces, edges: edges.edges });
        }
        self.cache.get(body).ok_or_else(|| format!("no body `{body}`"))
    }
}

/// Find a face of `body` by name or by a point on it, and build its surface.
pub fn find_face(engine: &mut dyn Engine, bodies: &mut Bodies, body: &str, face: &FaceRef, flip: bool) -> Result<Surface, String> {
    let g = bodies.get(engine, body)?;
    match face {
        FaceRef::Name(name) => {
            let f = g.faces.iter().find(|f| f.name.as_deref() == Some(name.as_str())).ok_or_else(|| format!("`{body}` has no face named `{name}`"))?;
            surface::build(f, &g.edges, flip)
        }
        FaceRef::Point(p) => {
            let p = V3::from_array(*p);
            let mut best: Option<(f64, Surface)> = None;
            let mut why = None;
            for f in &g.faces {
                match surface::build(f, &g.edges, flip) {
                    Ok(s) => {
                        let d = s.distance_to(p);
                        if best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                            best = Some((d, s));
                        }
                    }
                    Err(e) => why = Some(e),
                }
            }
            match best {
                Some((d, s)) if d < 0.01 => Ok(s),
                Some((d, _)) => Err(format!("the point is {d:.3} mm from the nearest planar or cylindrical face of `{body}`; pick a point on the face")),
                None => Err(why.unwrap_or_else(|| format!("`{body}` has no planar or cylindrical faces"))),
            }
        }
    }
}

pub enum Attractor {
    Point(V3),
    Path(Vec<V3>),
    Face(Box<Surface>),
}

impl Attractor {
    pub fn distance_to(&self, p: V3) -> f64 {
        match self {
            Attractor::Point(a) => a.dist(p),
            Attractor::Path(pts) => surface::polyline_distance(pts, p),
            Attractor::Face(s) => s.distance_to(p),
        }
    }
}

pub fn distance(attractors: &[Attractor], p: V3) -> f64 {
    attractors.iter().map(|a| a.distance_to(p)).fold(f64::INFINITY, f64::min)
}

/// Turn the parameters' attractors into geometry.
pub fn resolve(engine: &mut dyn Engine, bodies: &mut Bodies, refs: &[AttractorRef]) -> Result<Vec<Attractor>, String> {
    let mut out = Vec::new();
    for r in refs {
        match r {
            AttractorRef::Point(p) => out.push(Attractor::Point(V3::from_array(*p))),
            AttractorRef::Tagged(t) => {
                if let Some(p) = t.point {
                    out.push(Attractor::Point(V3::from_array(p)));
                } else if let Some([a, b]) = t.line {
                    out.push(Attractor::Path(vec![V3::from_array(a), V3::from_array(b)]));
                } else if let (Some(face), Some(body)) = (&t.face, &t.body) {
                    out.push(Attractor::Face(Box::new(find_face(engine, bodies, body, face, false)?)));
                } else if let (Some(p), Some(body)) = (t.edge, &t.body) {
                    let g = bodies.get(engine, body)?;
                    let p = V3::from_array(p);
                    let nearest = traced_edges(g).into_iter().min_by(|a, b| a.distance_to(p).total_cmp(&b.distance_to(p)));
                    match nearest {
                        Some(e) if e.distance_to(p) < 0.01 => out.push(Attractor::Path(e.points)),
                        _ => return Err(format!("no edge of `{body}` passes through the point")),
                    }
                } else if let Some(body) = &t.body {
                    // A whole body: its planar and cylindrical faces, and all of its edges for the rest.
                    let g = bodies.get(engine, body)?;
                    let faces: Vec<Surface> = g.faces.iter().filter_map(|f| surface::build(f, &g.edges, false).ok()).collect();
                    let edges = traced_edges(g);
                    if faces.is_empty() && edges.is_empty() {
                        return Err(format!("`{body}` has no faces or edges to measure to"));
                    }
                    out.extend(faces.into_iter().map(|s| Attractor::Face(Box::new(s))));
                    out.extend(edges.into_iter().map(|e| Attractor::Path(e.points)));
                }
            }
        }
    }
    Ok(out)
}

/// Every edge of a body traced. A closed edge takes its plane from a planar face it lies in.
fn traced_edges(g: &BodyGeometry) -> Vec<Traced> {
    g.edges
        .iter()
        .filter_map(|e| {
            let normal = g.faces.iter().filter_map(|f| f.plane_normal.map(|n| (V3::from_array(n), V3::from_array(f.point.unwrap_or(f.centroid))))).find_map(|(n, on)| {
                let n = n.norm()?;
                let flat = |p: [f64; 3]| (V3::from_array(p) - on).dot(n).abs() < 1e-3;
                (flat(e.mid) && e.start.is_some_and(flat)).then_some(n)
            });
            surface::trace_edge(e, normal).or_else(|| Some(Traced { points: [e.start, Some(e.mid)].into_iter().flatten().map(V3::from_array).collect(), straight: false }))
        })
        .collect()
}
