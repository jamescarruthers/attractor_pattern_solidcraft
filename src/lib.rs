//! Attractor pattern: a SolidCraft plugin.
//!
//! Copies a body onto points of a face and changes each copy by its distance to one or more
//! attractors: scale, depth along the face normal, a turn, and colour. The copies can stay
//! separate or be joined to, cut from or intersected with other bodies.
//!
//! It is written against the draft plugin interface `solidcraft:plugin@0.1.0`
//! (`wit/solidcraft-plugin.wit`, world `command-plugin`). All of the work happens through
//! engine commands, so one Undo removes a whole pattern.

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

    use exports::solidcraft::plugin::command::Guest;
    use solidcraft::plugin::types::PluginError;
    use solidcraft::plugin::{engine, log};

    struct Host;

    impl super::Engine for Host {
        fn run(&mut self, command: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
            let text = engine::run(command, &args.to_string()).map_err(|e| match e {
                PluginError::Failed(m) => m,
                PluginError::Denied(m) => format!("denied: {m}"),
                PluginError::Limit(m) => format!("limit: {m}"),
                PluginError::Invalid(m) => format!("invalid: {m}"),
            })?;
            serde_json::from_str(&text).map_err(|e| format!("unreadable reply: {e}"))
        }

        fn log(&mut self, message: &str) {
            log::log(log::Level::Info, message);
        }
    }

    struct Plugin;

    impl Guest for Plugin {
        fn execute(entry: String, params: String) -> Result<String, PluginError> {
            super::execute(&mut Host, &entry, &params).map_err(|e| {
                if e.starts_with("bad parameters") || e.starts_with('`') || e.starts_with("no entry") {
                    PluginError::Invalid(e)
                } else {
                    PluginError::Failed(e)
                }
            })
        }
    }

    export!(Plugin);
}
