//! Attractor pattern: a SolidCraft plugin.
//!
//! Copies a body onto points of a face and changes each copy by its distance to one or more
//! attractors: scale, depth along the face normal, a turn, and colour. The copies can stay
//! separate or be joined to, cut from or intersected with other bodies.
//!
//! It is built for SolidCraft 0.41's plugin host: WIT package `craft:plugin@0.1.0`
//! (`wit/plugin.wit`), world `command-plugin`, capability `commands@0.1`. All of the work
//! happens through engine commands, so one Undo removes a whole pattern.

pub mod attractor;
pub mod colour;
pub mod engine;
pub mod grid;
pub mod params;
pub mod pattern;
pub mod surface;
pub mod vec;

use engine::Engine;

/// Run one entry with JSON parameters; returns the JSON result.
pub fn execute(engine: &mut dyn Engine, entry: &str, params: &str) -> Result<String, String> {
    let p = params::parse(params)?;
    let out = match entry {
        "pattern" => pattern::pattern(engine, &p)?,
        "preview" => pattern::preview(engine, &p)?,
        other => return Err(format!("no entry `{other}`; this plugin has `pattern` and `preview`")),
    };
    serde_json::to_string(&out).map_err(|e| e.to_string())
}

/// The WebAssembly glue. The code wit-bindgen generates uses `unsafe`; nothing written here does.
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
mod component {
    wit_bindgen::generate!({ world: "command-plugin", path: "wit" });

    use craft::plugin::commands;
    use craft::plugin::types::{CommandFailure, FieldFailure, PluginError};
    use exports::craft::plugin::entry::Guest;

    /// The host's commands. A refused call poisons the whole run, so the first host error is
    /// kept and returned as it was, rather than as text.
    #[derive(Default)]
    struct Host {
        refused: Option<PluginError>,
    }

    fn describe(e: &PluginError) -> String {
        match e {
            PluginError::PermissionDenied(m) => format!("permission denied: {m}"),
            PluginError::CommandFailed(f) => format!("{}: {}", f.command, f.reason),
            PluginError::LimitExceeded(m) => format!("limit exceeded: {m}"),
            PluginError::InvalidManifest(f) => format!("invalid manifest {}: {}", f.field, f.reason),
            PluginError::InvalidInput(f) => format!("invalid input {}: {}", f.field, f.reason),
            PluginError::Trap(m) => format!("trap: {m}"),
            PluginError::Cancelled => "cancelled".into(),
        }
    }

    impl super::Engine for Host {
        fn run(&mut self, command: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
            match commands::run(command, &args.to_string()) {
                Ok(text) => serde_json::from_str(&text).map_err(|e| format!("unreadable reply: {e}")),
                Err(e) => {
                    let text = describe(&e);
                    self.refused.get_or_insert(e);
                    Err(text)
                }
            }
        }
    }

    struct Plugin;

    impl Guest for Plugin {
        fn execute(entry: String, params: String) -> Result<String, PluginError> {
            let mut host = Host::default();
            super::execute(&mut host, &entry, &params).map_err(|reason| match host.refused.take() {
                Some(e) => e,
                None if reason.starts_with("bad parameters") || reason.starts_with('`') => {
                    PluginError::InvalidInput(FieldFailure { field: "params".into(), reason })
                }
                None => PluginError::CommandFailed(CommandFailure { command: format!("attractor-pattern.{entry}"), reason }),
            })
        }
    }

    export!(Plugin);
}
