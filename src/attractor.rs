//! Attractors and the bodies they are read from.

use crate::engine::{Engine, run};
use crate::mesh::{self, MeshFace, MeshReply};
use crate::params::{AttractorRef, FaceRef};
use crate::surface::{self, EdgeInfo, FaceInfo, Surface, Traced};
use crate::vec::V3;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

/// Faces, edges and meshes of bodies, each read at most once per body.
#[derive(Default)]
pub struct Bodies {
    cache: BTreeMap<String, BodyGeometry>,
    meshes: BTreeMap<String, (MeshReply, f64)>,
    /// The chord tolerance for meshes, when the parameters give one.
    pub tolerance: Option<f64>,
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

    /// The body's mesh and the tolerance it was made with. The tolerance follows the body's
    /// area so the mesh stays inside the engine's transfer limit, since a refused call would end
    /// the run.
    pub fn mesh(&mut self, engine: &mut dyn Engine, body: &str) -> Result<&(MeshReply, f64), String> {
        if !self.meshes.contains_key(body) {
            let area: f64 = self.get(engine, body)?.faces.iter().map(|f| f.area).sum();
            let tolerance = self.tolerance.unwrap_or_else(|| mesh::tolerance_for_area(area));
            let reply: MeshReply = serde_json::from_value(run(engine, "model.mesh", &json!({ "body": body, "tolerance": tolerance }))?)
                .map_err(|e| format!("model.mesh gave an unexpected reply: {e}"))?;
            if reply.component.is_some_and(|c| c != 0) {
                return Err(format!("`{body}` is inside a component; curved faces and body attractors work on bodies at the top level only"));
            }
            self.meshes.insert(body.to_owned(), (reply, tolerance));
        }
        self.meshes.get(body).ok_or_else(|| format!("no mesh for `{body}`"))
    }
}

fn exact(f: &FaceInfo) -> bool {
    f.plane_normal.is_some() || f.cylinder.is_some()
}

/// The face of `body` nearest `pick`, from the body's mesh, if `pick` is close enough to it.
fn mesh_face(engine: &mut dyn Engine, bodies: &mut Bodies, body: &str, pick: V3, flip: bool) -> Result<Surface, String> {
    let (reply, tolerance) = bodies.mesh(engine, body)?;
    let (face, d) = MeshFace::build(reply, pick, flip, *tolerance)?;
    let reach = 2.0 * tolerance + 0.01;
    if d > reach {
        return Err(format!("the point is {d:.3} mm from `{body}`; pick a point on the face"));
    }
    Ok(Surface::Mesh(Box::new(face)))
}

/// Find a face of `body` by name or by a point on it, and build its surface: exactly for planar
/// and cylindrical faces, from the body's mesh for any other.
pub fn find_face(engine: &mut dyn Engine, bodies: &mut Bodies, body: &str, face: &FaceRef, flip: bool) -> Result<Surface, String> {
    let g = bodies.get(engine, body)?;
    match face {
        FaceRef::Name(name) => {
            let f = g.faces.iter().find(|f| f.name.as_deref() == Some(name.as_str())).ok_or_else(|| format!("`{body}` has no face named `{name}`"))?;
            if exact(f)
                && let Ok(s) = surface::build(f, &g.edges, flip)
            {
                return Ok(s);
            }
            let pick = V3::from_array(f.point.unwrap_or(f.centroid));
            mesh_face(engine, bodies, body, pick, flip)
        }
        FaceRef::Point(p) => {
            let p = V3::from_array(*p);
            let best = g
                .faces
                .iter()
                .filter(|f| exact(f))
                .filter_map(|f| surface::build(f, &g.edges, flip).ok())
                .map(|s| (s.distance_to(p), s))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            match best {
                Some((d, s)) if d < 0.01 => Ok(s),
                _ => mesh_face(engine, bodies, body, p, flip),
            }
        }
    }
}

pub enum Attractor {
    Point(V3),
    Path(Vec<V3>),
    Face(Box<Surface>),
    /// A whole body, by its mesh.
    Body(mesh::Soup),
}

impl Attractor {
    pub fn distance_to(&self, p: V3) -> f64 {
        match self {
            Attractor::Point(a) => a.dist(p),
            Attractor::Path(pts) => surface::polyline_distance(pts, p),
            Attractor::Face(s) => s.distance_to(p),
            Attractor::Body(m) => mesh::triangles_distance(&m.pts, &m.tris, p),
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
                    // A whole body: the distance to its surface, by its mesh.
                    let (reply, _) = bodies.mesh(engine, body)?;
                    out.push(Attractor::Body(mesh::soup(reply)?));
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
