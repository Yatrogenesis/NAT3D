// SOTA 9: Agentic AI Scene Orchestrator
use crate::ScriptingHost;
use std::sync::Arc;

/// A local deterministic AI agent that parses natural language intent into scene actions
pub struct SceneAgent {
    host: Arc<dyn ScriptingHost>,
}

impl SceneAgent {
    pub fn new(host: Arc<dyn ScriptingHost>) -> Self {
        Self { host }
    }

    /// Process a natural language command deterministically
    pub fn process_intent(&self, command: &str) -> String {
        let cmd = command.to_lowercase();
        if cmd.contains("create") && cmd.contains("cube") {
            self.host.create_object("Cube", "AI_Cube");
            "Action: Created Cube".to_string()
        } else if cmd.contains("delete") {
            // Very naive extraction for demonstration
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if let Some(target) = parts.last() {
                self.host.delete_object(target);
                return format!("Action: Deleted {}", target);
            }
            "Failed to parse target".to_string()
        } else {
            "Intent not recognized".to_string()
        }
    }
}
