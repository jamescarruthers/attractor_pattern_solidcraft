//! The one way the plugin reaches the model: SolidCraft's engine commands, JSON in and out.

use serde_json::Value;

/// Runs engine commands. The WebAssembly build wraps the host's `engine.run` import; tests and
/// harnesses supply their own.
pub trait Engine {
    fn run(&mut self, command: &str, args: &Value) -> Result<Value, String>;

    /// A message for the user's log. Ignored unless the host shows it.
    fn log(&mut self, _message: &str) {}
}

/// Run a command and add its name to any error.
pub fn run(engine: &mut dyn Engine, command: &str, args: &Value) -> Result<Value, String> {
    engine.run(command, args).map_err(|e| format!("{command}: {e}"))
}

/// The names of the bodies a feature command made (`created_bodies` in its reply).
pub fn created_bodies(reply: &Value) -> Vec<String> {
    reply
        .get("created_bodies")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}
