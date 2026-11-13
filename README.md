# Attractor pattern for SolidCraft

A SolidCraft plugin that copies a body over a face. Each copy changes with its distance to one
or more attractors: it can grow or shrink, sink into the face, turn, and change colour. The
copies can stay separate bodies, or be joined to, cut from or intersected with other bodies.

It is a port of the Onshape FeatureScript feature in `attractor-pattern-onshape.code`.

## Status

SolidCraft cannot run this plugin yet. Its plugin host does not exist: today the app lists
plugin folders but loads nothing, and its listing reader refuses the fields this `plugin.json`
uses, so the Plugins page will show the plugin as skipped.

The plugin is written against the draft interface in SolidCraft's plugin proposal
([tangency-labs/solidcraft#4980](https://github.com/tangency-labs/solidcraft/pull/4980)):
WIT package `solidcraft:plugin@0.1.0`, world `command-plugin`, `plugin.json` version 1. A copy
of that WIT file is in `wit/`. If the interface changes before it ships, `src/lib.rs` (the
WebAssembly glue) and `plugin.json` will need to follow; the rest talks to SolidCraft only
through its JSON engine commands, which MCP and scripts already use.

The geometry has been checked against SolidCraft's real engine by running the plugin's code
natively against `solidcraft_engine::Session` (see "Testing" below).

## Build and install

```bash
./build.sh
```

This builds `attractor_pattern.wasm` for `wasm32-wasip2` and puts it with `plugin.json` in
`dist/attractor-pattern/`. Copy that folder into SolidCraft's `{data}/plugins/` folder.

## Use

The plugin has two entries.

- `pattern` makes the copies in one transaction, so one Undo removes them all.
- `preview` makes nothing. It returns where each copy would go, its distance, turn, scale and
  colour, and how many grid points were dropped. It takes the same parameters.

From a script, MCP or the control channel:

```json
{"command": "plugin.run", "params": {"plugin": "attractor-pattern", "entry": "pattern", "params": { ... }}}
```

`examples/pins.json` is a complete script: a plate, a pin, and pins joined over the plate.

### Parameters

Lengths are in mm, angles in degrees.

| Parameter | Default | Meaning |
|---|---|---|
| `body` | required | The body to copy. |
| `face` | required | The face to pattern on: a point on it `[x, y, z]`, or its name from `model.faces`. |
| `face_body` | required | The body that owns the face. |
| `attractors` | required | 1 to 256 of: a point `[x, y, z]`; `{"point": [x, y, z]}`; `{"line": [[x, y, z], [x, y, z]]}`; `{"edge": [x, y, z], "body": B}` (the edge of B through that point); `{"face": point or name, "body": B}`; `{"body": B}` (all of B's planar and cylindrical faces and all its edges). Distance is to the nearest one. |
| `grid.type` | `rectangular` | `rectangular`, `triangular` (every other column shifted half a step) or `random`. |
| `grid.u_count`, `grid.v_count` | 5, 5 | Roughly how many points across and along the face. |
| `grid.count`, `grid.seed` | 50, 1 | Random grids: how many points, and the seed. |
| `grid.rotation` | 0 | Turns the grid on the face, counter-clockwise seen from the side the normal points to. |
| `grid.border` | 0 | Drops points closer than this to the face's edges. |
| `scale` | off | `{"near": k, "far": k}`; each `k` is a number or `[x, y, z]` along the body frame's axes. |
| `depth` | off | `{"near": d, "far": d}`: how far each copy sinks along the face normal. Negative lifts it. |
| `rotation` | off | `{"axis": "x" \| "y" \| "z", "near": a, "far": a}`: a turn about the body frame's axis. |
| `color` | off | `{"near": c, "far": c, "near_alpha": 0-255, "far_alpha": 0-255}`; `c` is `"#rrggbb"`, `[r, g, b]` (0-255) or `{"h": degrees, "s": 0-1, "l": 0-1}`. |
| `range` | measured | `{"near": d, "far": d}`: the distances that count as near and far. Distances outside are held at the ends. Without it, the nearest and farthest copies set the ends. |
| `cutoff` | off | Drops copies farther than this from every attractor. |
| `falloff` | off | `{"waves": n, "decay": d}`: a wave over distance instead of a straight ramp. |
| `operation` | `new` | `new` (separate bodies), `join`, `cut` or `intersect`. |
| `merge_scope` | every other visible body | The bodies to join, cut or intersect with. |
| `name_prefix` | none | With `new`: names the copies `"<prefix> 1"`, `"<prefix> 2"`, ... |
| `keep_input` | false | Keep the body that was copied. |
| `body_frame` | the world | `{"origin", "z_dir", "x_dir"}`: the body's own frame. It is laid onto each point: its origin on the face, `z` along the normal, `x` along the grid's first direction. |
| `flip_body` | false | Turns the body over before placing it. |
| `flip_normal` | false | Flips the face normal, so copies stand on the other side. For holes, flip it. |

Model the body to copy at the origin, standing on the XY plane and pointing up Z, or give its
frame with `body_frame`.

## How it maps from the Onshape feature

| Onshape | Here |
|---|---|
| Base surface: any face | Planar and cylindrical faces only. SolidCraft has no command that evaluates a face at a (u, v) parameter, so the plugin builds its own from `model.faces` and `model.edges`. Curved edges are traced as circular arcs through their start, middle and end points, so spline edges are approximated. Cylindrical faces are treated as untrimmed between their edges. |
| Input body from another Part Studio | Not available: the body must be in the design. |
| Mate connector | `body_frame`. |
| Attractors: vertex, edge, face, body | Points, lines, edges, planar and cylindrical faces, and bodies (their planar and cylindrical faces and edges). |
| Grid U/V on the face's parameter space | The same lattice, on the plugin's parameterisation: for a plane, the box around its outline, aligned with its longest straight edge; for a cylinder, angle and height. |
| Random grid | Points spread over the whole face. The original drew from twice the face's range, so most points fell off; asking for 50 now gives about 50 before culling. Seeds do not give the same layout as Onshape. |
| Flip toggles beside each value | Use negative values. |
| Wave falloff, range, cutoff, colour (hex, RGB, HSL), alpha | As the original, including the wave's decay (height shrinks 100-fold every `decay` mm). |
| Composite part | No equivalent; `new` leaves separate bodies, which `name_prefix` can name. |
| Pause regeneration and preview points | The `preview` entry. |
| A parametric feature that rebuilds | Not yet: plugin features that rebuild are later work in SolidCraft's plan. Each run adds ordinary Move, Scale, Combine and Appearance features to the timeline. |

## Limits

- At most 1500 copies per run. Each copy costs one to three timeline features, and the host
  allows 10 000 engine commands and, as asked in `plugin.json`, 120 seconds per run.
- A copy that is not scaled costs one feature; a uniform scale two; a scale that differs per
  axis three.

## Testing

```bash
cargo test
```

The unit tests cover the maths, the face outlines, the grid and the parameter checks. The
geometry was also checked by running the plugin's code natively against SolidCraft's engine:
pins on a plate, a join with per-axis scale and turns, random spheres cut from a plate with a
hole, studs around a cylinder, and error cases.

## Credit and licence

The original feature is by Evan Reese and distributed by The Onsherpa. Its header says it may
not be redistributed without the owner's permission. This port is new code, but it follows that
feature's design closely; get permission before publishing it.

The WIT file in `wit/` is from SolidCraft and is under its MIT or Apache-2.0 licence.
