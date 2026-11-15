# Attractor pattern for SolidCraft

A SolidCraft plugin that copies a body over a face. Each copy changes with its distance to one
or more attractors: it can grow or shrink, sink into the face, turn, and change colour. The
copies can stay separate bodies, or be joined to, cut from or intersected with other bodies.

It is a port of the Onshape FeatureScript feature in `attractor-pattern-onshape.code`.

It needs SolidCraft 0.41 or later: plugin capability `commands@0.1`, WIT package
`craft:plugin@0.1.0` (a copy is in `wit/plugin.wit`).

## Install

Build the zip (needs Rust and `cargo install wasm-tools`):

```bash
./build.sh
```

Then install `dist/attractor-pattern-<version>.zip` from **Preferences > Plugins > Install
plugin…**. The entries appear in the **Plugins** menu as *Attractor pattern* and *Preview
attractor pattern*.

## Use

The plugin has two entries.

- `pattern` makes the copies in one transaction, so one Undo removes them all.
- `preview` changes nothing. It returns where each copy would go, its distance, turn, scale and
  colour, how many grid points were dropped, and how many engine calls `pattern` would need.

The Plugins menu draws a form for each entry. Scripts, MCP and the CLI call them through
`plugin.run`:

```json
{"command": "plugin.run", "params": {"path": "<plugin folder>", "approval": "<hash from plugin.inspect>", "entry": "pattern", "params": { ... }}}
```

`examples/pins.json` holds parameters for pins on a plate.

`examples/attractor-curved-demo.sc` shows curved faces: beads on a torus, studs on a ball and
spikes on a cone.

`examples/attractor-demo.sc` is a design made with the plugin in SolidCraft 0.41's command-line
tool: tiles on a plate, studs round a post, and dimples cut into a panel. It opens without the
plugin, because the copies are ordinary features. `examples/attractor-demo-script.json` is the
script that made it; fill in the plugin folder and the approval from `plugin.inspect` to run it.

### Picking with the mouse

You can select things instead of typing their names. Select, in this order:

1. the face to pattern on;
2. the body to copy: the body itself, or any face or edge of it;
3. any attractors: faces, edges and bodies (Shift-click to add to the selection).

Then open the entry from the Plugins menu and leave **Face**, **Body with the face**, **Body to
copy** and the attractor fields empty. Anything typed in a field is used instead of the
selection. Attractors come from the selection only when every attractor field is empty, so you
can type points and still pick the face and the body. The result's `from_selection` says what
each pick was used for; run *Preview attractor pattern* first to check it. Keep the selection
until the run finishes: the app drops a result made from a selection that has since changed.

### Parameters

Lengths are in mm, angles in degrees. Points are text, `x, y, z`. Lists are text, items separated
by `;`. Empty fields take their defaults.

| Parameter | Default | Meaning |
|---|---|---|
| `body` | the selection | The body to copy. Model it at the origin, standing on XY and pointing up Z, or give its frame with `body_origin`, `body_z` and `body_x`. |
| `face_body` | the selection | The body that owns the face. |
| `face` | the selection | A point on the face, `x, y, z`, or the face's name from `model.faces`. Any face: see "Curved faces" below. |
| `attractor_points` | | `x, y, z; x, y, z; …` |
| `attractor_lines` | | `x, y, z to x, y, z; …` |
| `attractor_edges` | | `Body @ x, y, z; …`: the edge of that body through the point. |
| `attractor_faces` | | `Body @ x, y, z; …` or `Body @ face name; …`. |
| `attractor_bodies` | | `Body; Body; …`: the distance to the body's surface, read from its mesh. |
| `grid` | `rectangular` | `rectangular`, `triangular` (every other column shifted half a step) or `random`. |
| `u_count`, `v_count` | 5, 5 | About how many copies across and along the face. |
| `count`, `seed` | 50, 1 | Random grids: how many points, and the seed. |
| `grid_rotation` | 0 | Turns the grid on the face, counter-clockwise seen from the side the normal points to. |
| `border` | 0 | Drops points closer than this to the face's edges. |
| `scale_near`, `scale_far` | off | Scale of the nearest and farthest copies. |
| `scale_near_xyz`, `scale_far_xyz` | off | Per-axis scale, `x, y, z` along the body's axes. Overrides the plain scale. |
| `depth_near`, `depth_far` | off | How far copies sink along the face normal. Negative lifts them. |
| `rotation_axis`, `rotation_near`, `rotation_far` | `z`, off | A turn about the body's axis. |
| `color_near`, `color_far` | off | `#rrggbb`, `r, g, b` (0–255) or `hsl(h, s, l)` (degrees, 0–1, 0–1). |
| `alpha_near`, `alpha_far` | 255 | Opacity, 0–255. |
| `range_near`, `range_far` | measured | The distances that count as near and far; give both or neither. Distances outside are held at the ends. Without them, the nearest and farthest copies set the ends. |
| `cutoff` | off | Drops copies farther than this from every attractor. |
| `waves`, `decay` | off | Wave falloff: a wave over distance instead of a straight ramp. Its height shrinks 100-fold every `decay` mm. |
| `operation` | `new` | `new` (separate bodies), `join`, `cut` or `intersect`. |
| `merge_scope` | every other visible body | `Body; Body; …` to join, cut or intersect with. |
| `keep_input` | false | Keep the body that was copied. |
| `levels` | 12 | How many steps scale and colour take from near to far (2–64). |
| `flip_body` | false | Turns the body over before placing it. |
| `flip_normal` | false | Flips the face normal, so copies stand on the other side. Use it for holes. |
| `body_origin`, `body_z`, `body_x` | the world | The body's own frame. Its origin goes onto the face, `z` along the normal, `x` along the grid. |
| `mesh_tolerance` | from the body's size | How closely the mesh of a curved face or body attractor follows the surface (mm). |

## Curved faces

Flat and cylindrical faces are read exactly, and the grid lies on them in rows, as in Onshape.

Any other face (a torus, a sphere, a cone, a fillet round a corner, a loft) is read from the
body's mesh (`model.mesh`, read-only). The face is the part of the mesh under the given point.
The copies are spread evenly over it rather than in rows: about as many as the grid would put on
a flat face (`u_count` × `v_count`), or `count` random ones for a random grid. Each point is lifted
from the mesh's flat triangles towards the curved surface. On a 20 × 5 mm torus in testing, every
copy sat within 0.04 mm of the true surface. Each copy's `x` axis runs round the face's main axis,
as on a cylinder, and `grid_rotation` turns it.

The mesh tolerance is chosen from the body's area so that the mesh stays within what the engine
will hand to a plugin (8192 points); set `mesh_tolerance` to override it. A finer mesh that
breaks that limit stops the run with "mesh exceeds the plugin transfer budget", and nothing
changes. Curved faces of bodies inside a component are not supported yet, because the mesh is
in the component's own coordinates.

## The 256-call limit

SolidCraft lets one plugin run make at most 256 engine calls. The plugin spends them like this:

- a few to read the design: 3, plus 2 for each other body named as an attractor;
- one per scale and colour step that is used: up to 3 calls each (a copy, a scale, a colour);
- one per copy on the face;
- one to join, or one per target body to cut or intersect;
- one to remove the seeds and the original body.

So a run makes about 200 copies at most. Turns and depths are exact for every copy. Scale and
colour come in `levels` steps, because each step costs a prepared copy of the body. `preview`
reports the count (`budget.engine_calls`) before anything is made. `pattern` checks it first and
refuses, changing nothing, when it would not fit.

Each run must also finish within 30 seconds, and that includes the time SolidCraft spends on
the join, cut or intersect. Booleans with many copies are slow: in a test build, cutting 114
spheres from a plate took 67 seconds by itself, while 33 spheres took under 3 seconds. `preview`
adds a `note` when a boolean has more than 30 copies. If a run stops with "plugin limit exceeded:
deadline", nothing has changed: run it again with **Result** `new`, then join or cut the copies
with SolidCraft's own Combine, which has no such limit.

## How it maps from the Onshape feature

| Onshape | Here |
|---|---|
| Base surface: any face | Any face. SolidCraft has no command that evaluates a face at a (u, v) parameter, so flat and cylindrical faces are rebuilt from `model.faces` and `model.edges`, and other faces from the body's mesh (see "Curved faces"). Curved edges of flat faces are traced as circular arcs through their start, middle and end points, so spline edges are approximated. Cylindrical faces are treated as untrimmed between their edges. |
| Grid rows on curved faces | Rows on flat and cylindrical faces; evenly spread points on other curved faces. |
| Input body from another Part Studio | Not available: the body must be in the design. |
| Mate connector | `body_origin`, `body_z`, `body_x`. |
| Attractors: vertex, edge, face, body | Points, lines, edges, faces and bodies. |
| Scale and colour for every copy | In `levels` steps (see above). |
| Grid U/V on the face's parameter space | The same lattice, on the plugin's parameterisation: for a plane, the box around its outline, aligned with its longest straight edge; for a cylinder, angle and height. |
| Random grid | Points spread over the whole face. The original drew from twice the face's range, so most points fell off; asking for 50 now gives about 50 before culling. Seeds do not give the same layout as Onshape. |
| Flip toggles beside each value | Use negative values. |
| Composite part, naming the copies | Not available. Renaming costs one call per copy. |
| Pause regeneration and preview points | The `preview` entry. |
| A parametric feature that rebuilds | Not available in SolidCraft 0.41. Each run adds ordinary Move, Scale, Combine and Appearance features to the timeline. |

## Testing

```bash
cargo test
```

The unit tests cover the maths, the face outlines, the grid and the parameter checks.

Version 0.2.0 was also installed from its zip into SolidCraft 0.41's command-line tool
(`plugin.install`) and run there (`plugin.inspect`, `plugin.run`): a preview; pins on a plate
with scale, colour and turns; per-axis scale, turns and depth joined into the plate; spheres cut
from a plate with a hole; studs around a cylinder; and the refusals for too many copies and for
a point off the face. Each pattern was one undo step. The Plugins menu's form was not tried.

Version 0.3.0 added curved faces and was run the same way: a torus (grid, random grid and
preview), a sphere, the side of a cone, a body as attractor, and the flat plate again.

## Credit and licence

The original feature is by Evan Reese and distributed by The Onsherpa. Its header says it may
not be redistributed without the owner's permission. This port is new code, but it follows that
feature's design closely; get permission before publishing it.

The WIT file in `wit/` is from SolidCraft and is under its MIT or Apache-2.0 licence.
