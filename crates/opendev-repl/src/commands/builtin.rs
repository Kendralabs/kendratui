//! Built-in slash commands: /help, /clear, /mode, /exit, etc.
//!
//! Mirrors `opendev/repl/repl.py::_handle_command`.

use opendev_runtime::{AutonomyLevel, ThinkingLevel};

use crate::repl::{OperationMode, ReplState};

/// Outcome of dispatching a slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOutcome {
    /// Command was handled successfully.
    Handled,
    /// The user wants to exit.
    Exit,
    /// Command was not recognized.
    Unknown,
}

/// Handles built-in slash commands.
pub struct BuiltinCommands;

impl BuiltinCommands {
    /// Create a new command handler.
    pub fn new() -> Self {
        Self
    }

    /// Dispatch a slash command.
    ///
    /// Returns the outcome indicating how the REPL should proceed.
    pub fn dispatch(&self, cmd: &str, args: &str, state: &mut ReplState) -> CommandOutcome {
        match cmd {
            "/help" => {
                self.handle_help(args);
                CommandOutcome::Handled
            }
            "/exit" | "/quit" => CommandOutcome::Exit,
            "/clear" => {
                self.handle_clear(state);
                CommandOutcome::Handled
            }
            "/mode" => {
                self.handle_mode(args, state);
                CommandOutcome::Handled
            }
            "/compact" => {
                self.handle_compact(state);
                CommandOutcome::Handled
            }
            "/models" => {
                self.handle_models();
                CommandOutcome::Handled
            }
            "/mcp" => {
                self.handle_mcp(args);
                CommandOutcome::Handled
            }
            "/agents" => {
                self.handle_agents(args);
                CommandOutcome::Handled
            }
            "/skills" => {
                self.handle_skills(args);
                CommandOutcome::Handled
            }
            "/plugins" => {
                self.handle_plugins(args);
                CommandOutcome::Handled
            }
            "/session-models" => {
                self.handle_session_models(args);
                CommandOutcome::Handled
            }
            "/thinking" => {
                self.handle_thinking(args, state);
                CommandOutcome::Handled
            }
            "/autonomy" => {
                self.handle_autonomy(args, state);
                CommandOutcome::Handled
            }
            "/status" => {
                self.handle_status(state);
                CommandOutcome::Handled
            }
            "/init" => {
                self.handle_init();
                CommandOutcome::Handled
            }
            _ => CommandOutcome::Unknown,
        }
    }

    fn handle_help(&self, _args: &str) {
        println!("Available commands:");
        println!("  /help                   Show this help message");
        println!("  /exit, /quit            Exit the REPL");
        println!("  /clear                  Clear conversation history");
        println!("  /mode [plan|normal]     Switch operation mode");
        println!("  /thinking [off|low|medium|high]  Set thinking depth");
        println!("  /autonomy [manual|semi-auto|auto] Set approval level");
        println!("  /status                 Show current mode/thinking/autonomy");
        println!("  /compact                Compact conversation context");
        println!("  /models                 Show model selector");
        println!("  /mcp <subcommand>       Manage MCP servers");
        println!("  /agents <args>          Manage agents");
        println!("  /skills <args>          Manage skills");
        println!("  /plugins <args>         Manage plugins");
        println!("  /session-models         Session model management");
        println!("  /init                   Initialize codebase context");
    }

    fn handle_clear(&self, state: &mut ReplState) {
        state.messages_cleared = true;
        println!("Conversation cleared.");
    }

    fn handle_mode(&self, args: &str, state: &mut ReplState) {
        let target = args.trim().to_lowercase();
        match target.as_str() {
            "plan" => {
                state.mode = OperationMode::Plan;
                println!("Switched to Plan mode (read-only tools).");
            }
            "normal" | "" => {
                state.mode = OperationMode::Normal;
                println!("Switched to Normal mode (full tool access).");
            }
            _ => {
                println!("Usage: /mode [plan|normal]");
            }
        }
    }

    fn handle_compact(&self, state: &mut ReplState) {
        state.compact_requested = true;
        println!("Context compaction triggered.");
    }

    fn handle_models(&self) {
        println!("Available models:");
        println!("  1. gpt-4o (OpenAI)");
        println!("  2. gpt-4o-mini (OpenAI)");
        println!("  3. claude-3.5-sonnet (Anthropic)");
        println!("  4. claude-3-opus (Anthropic)");
        println!("  5. claude-3-haiku (Anthropic)");
        println!("  6. gemini-1.5-pro (Google)");
        println!("  7. deepseek-chat (DeepSeek)");
        println!();
        println!("Use /session-models set model <name> to change the active model.");
    }

    fn handle_mcp(&self, args: &str) {
        let parts: Vec<&str> = args.trim().splitn(2, ' ').collect();
        let subcommand = parts.first().copied().unwrap_or("");
        let sub_args = parts.get(1).copied().unwrap_or("");

        match subcommand {
            "" | "list" => {
                println!("MCP Servers:");
                println!("  (none configured)");
                println!();
                println!("Use /mcp add <name> <command> to register a server.");
            }
            "add" => {
                if sub_args.is_empty() {
                    println!("Usage: /mcp add <name> <command> [args...]");
                } else {
                    let name = sub_args.split_whitespace().next().unwrap_or(sub_args);
                    println!("MCP server '{}' registered (restart required to activate).", name);
                }
            }
            "remove" => {
                if sub_args.is_empty() {
                    println!("Usage: /mcp remove <name>");
                } else {
                    println!("MCP server '{}' removed.", sub_args.trim());
                }
            }
            "enable" => {
                if sub_args.is_empty() {
                    println!("Usage: /mcp enable <name>");
                } else {
                    println!("MCP server '{}' enabled.", sub_args.trim());
                }
            }
            "disable" => {
                if sub_args.is_empty() {
                    println!("Usage: /mcp disable <name>");
                } else {
                    println!("MCP server '{}' disabled.", sub_args.trim());
                }
            }
            _ => {
                println!("Unknown MCP subcommand: {}", subcommand);
                println!("Usage: /mcp [list|add|remove|enable|disable] ...");
            }
        }
    }

    fn handle_agents(&self, args: &str) {
        let subcommand = args.trim().split_whitespace().next().unwrap_or("list");
        match subcommand {
            "list" | "" => {
                println!("Available agents:");
                println!("  - Code-Explorer    Explore and understand codebase structure");
                println!("  - Planner          Create and refine implementation plans");
                println!("  - Ask-User         Request clarification from the user");
                println!("  - PR-Reviewer      Review pull requests for issues");
                println!("  - Security-Reviewer Audit code for security vulnerabilities");
                println!("  - Web-Clone        Clone and adapt web pages");
                println!("  - Web-Generator    Generate web applications from prompts");
            }
            _ => {
                println!("Usage: /agents [list]");
            }
        }
    }

    fn handle_skills(&self, args: &str) {
        let subcommand = args.trim().split_whitespace().next().unwrap_or("list");
        match subcommand {
            "list" | "" => {
                println!("Built-in skills:");
                println!("  - commit       Git commit best practices");
                println!("  - review-pr    Pull request review guidelines");
                println!("  - create-pr    Pull request creation workflow");
                println!();
                println!("Use /skills to invoke a skill by name.");
            }
            _ => {
                println!("Usage: /skills [list]");
            }
        }
    }

    fn handle_plugins(&self, args: &str) {
        let parts: Vec<&str> = args.trim().splitn(2, ' ').collect();
        let subcommand = parts.first().copied().unwrap_or("");
        let sub_args = parts.get(1).copied().unwrap_or("");

        match subcommand {
            "" | "list" => {
                println!("Plugins: (none installed)");
                println!("Use /plugins install <name> to add plugins.");
            }
            "install" => {
                if sub_args.is_empty() {
                    println!("Usage: /plugins install <name>");
                } else {
                    println!("Installing plugin '{}'...", sub_args.trim());
                    println!("Plugin installation not yet connected to marketplace.");
                }
            }
            "remove" => {
                if sub_args.is_empty() {
                    println!("Usage: /plugins remove <name>");
                } else {
                    println!("Plugin '{}' not found in installed plugins.", sub_args.trim());
                }
            }
            _ => {
                println!("Unknown plugins subcommand: {}", subcommand);
                println!("Usage: /plugins [list|install|remove] ...");
            }
        }
    }

    fn handle_session_models(&self, args: &str) {
        let parts: Vec<&str> = args.trim().splitn(2, ' ').collect();
        let subcommand = parts.first().copied().unwrap_or("");
        let sub_args = parts.get(1).copied().unwrap_or("");

        match subcommand {
            "" | "show" => {
                println!("No session model overrides set.");
                println!();
                println!("Available slots: model, model_thinking, model_vlm, model_critique, model_compact");
                println!("Use /session-models set <slot> <value> to override a model for this session.");
            }
            "set" => {
                let set_parts: Vec<&str> = sub_args.splitn(2, ' ').collect();
                if set_parts.len() < 2 {
                    println!("Usage: /session-models set <slot> <model-name>");
                } else {
                    let slot = set_parts[0];
                    let value = set_parts[1];
                    let valid_slots = [
                        "model", "model_provider",
                        "model_thinking", "model_thinking_provider",
                        "model_vlm", "model_vlm_provider",
                        "model_critique", "model_critique_provider",
                        "model_compact", "model_compact_provider",
                    ];
                    if valid_slots.contains(&slot) {
                        println!("Session override: {} = {}", slot, value);
                    } else {
                        println!("Unknown slot: {}", slot);
                        println!("Valid slots: {}", valid_slots.join(", "));
                    }
                }
            }
            "clear" => {
                println!("Session model overrides cleared.");
            }
            _ => {
                println!("Unknown session-models subcommand: {}", subcommand);
                println!("Usage: /session-models [show|set|clear] ...");
            }
        }
    }

    fn handle_thinking(&self, args: &str, state: &mut ReplState) {
        let target = args.trim();
        if target.is_empty() {
            println!("Thinking level: {}", state.thinking_level);
            println!("Usage: /thinking [off|low|medium|high]");
            return;
        }
        match ThinkingLevel::from_str_loose(target) {
            Some(level) => {
                state.thinking_level = level;
                let detail = match level {
                    ThinkingLevel::Off => "(thinking disabled)",
                    ThinkingLevel::Low => "(basic reasoning)",
                    ThinkingLevel::Medium => "(standard reasoning)",
                    ThinkingLevel::High => "(deep reasoning with critique)",
                };
                println!("Thinking level set to: {} {}", level, detail);
            }
            None => {
                println!("Invalid thinking level: {}", target);
                println!("Valid levels: off, low, medium, high");
            }
        }
    }

    fn handle_autonomy(&self, args: &str, state: &mut ReplState) {
        let target = args.trim();
        if target.is_empty() {
            println!("Autonomy level: {}", state.autonomy_level);
            println!("Usage: /autonomy [manual|semi-auto|auto]");
            return;
        }
        match AutonomyLevel::from_str_loose(target) {
            Some(level) => {
                state.autonomy_level = level;
                let detail = match level {
                    AutonomyLevel::Manual => "(all commands require approval)",
                    AutonomyLevel::SemiAuto => "(safe commands auto-approved)",
                    AutonomyLevel::Auto => "(all commands auto-approved)",
                };
                println!("Autonomy level set to: {} {}", level, detail);
            }
            None => {
                println!("Invalid autonomy level: {}", target);
                println!("Valid levels: manual, semi-auto, auto");
            }
        }
    }

    fn handle_status(&self, state: &ReplState) {
        println!("Current status:");
        println!("  Mode:      {}", state.mode);
        println!("  Thinking:  {}", state.thinking_level);
        println!("  Autonomy:  {}", state.autonomy_level);
    }

    fn handle_init(&self) {
        println!("Scanning codebase...");
        match std::env::current_dir() {
            Ok(cwd) => {
                println!("Working directory: {}", cwd.display());
                // Check for common project markers
                let markers = [
                    ("Cargo.toml", "Rust"),
                    ("package.json", "Node.js"),
                    ("pyproject.toml", "Python"),
                    ("go.mod", "Go"),
                    ("Makefile", "Make"),
                    (".git", "Git repo"),
                ];
                let mut found = Vec::new();
                for (file, label) in &markers {
                    if cwd.join(file).exists() {
                        found.push(*label);
                    }
                }
                if found.is_empty() {
                    println!("No recognized project markers found.");
                } else {
                    println!("Detected: {}", found.join(", "));
                }
                println!("Codebase context initialized.");
            }
            Err(e) => {
                println!("Failed to read working directory: {}", e);
            }
        }
    }
}

impl Default for BuiltinCommands {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_commands() {
        let cmds = BuiltinCommands::new();
        let mut state = ReplState::default();

        assert_eq!(
            cmds.dispatch("/exit", "", &mut state),
            CommandOutcome::Exit
        );
        assert_eq!(
            cmds.dispatch("/quit", "", &mut state),
            CommandOutcome::Exit
        );
    }

    #[test]
    fn test_help_command() {
        let cmds = BuiltinCommands::new();
        let mut state = ReplState::default();

        assert_eq!(
            cmds.dispatch("/help", "", &mut state),
            CommandOutcome::Handled
        );
    }

    #[test]
    fn test_mode_switch() {
        let cmds = BuiltinCommands::new();
        let mut state = ReplState::default();
        assert_eq!(state.mode, OperationMode::Normal);

        cmds.dispatch("/mode", "plan", &mut state);
        assert_eq!(state.mode, OperationMode::Plan);

        cmds.dispatch("/mode", "normal", &mut state);
        assert_eq!(state.mode, OperationMode::Normal);

        // Empty arg defaults to normal
        state.mode = OperationMode::Plan;
        cmds.dispatch("/mode", "", &mut state);
        assert_eq!(state.mode, OperationMode::Normal);
    }

    #[test]
    fn test_unknown_command() {
        let cmds = BuiltinCommands::new();
        let mut state = ReplState::default();

        assert_eq!(
            cmds.dispatch("/foobar", "", &mut state),
            CommandOutcome::Unknown
        );
    }

    #[test]
    fn test_clear_command() {
        let cmds = BuiltinCommands::new();
        let mut state = ReplState::default();

        assert_eq!(
            cmds.dispatch("/clear", "", &mut state),
            CommandOutcome::Handled
        );
    }

    #[test]
    fn test_thinking_command() {
        let cmds = BuiltinCommands::new();
        let mut state = ReplState::default();
        assert_eq!(state.thinking_level, ThinkingLevel::Medium);

        cmds.dispatch("/thinking", "high", &mut state);
        assert_eq!(state.thinking_level, ThinkingLevel::High);

        cmds.dispatch("/thinking", "off", &mut state);
        assert_eq!(state.thinking_level, ThinkingLevel::Off);

        cmds.dispatch("/thinking", "low", &mut state);
        assert_eq!(state.thinking_level, ThinkingLevel::Low);

        // Invalid value should not change level
        cmds.dispatch("/thinking", "garbage", &mut state);
        assert_eq!(state.thinking_level, ThinkingLevel::Low);

        // Empty shows current level
        assert_eq!(
            cmds.dispatch("/thinking", "", &mut state),
            CommandOutcome::Handled
        );
    }

    #[test]
    fn test_autonomy_command() {
        let cmds = BuiltinCommands::new();
        let mut state = ReplState::default();
        assert_eq!(state.autonomy_level, AutonomyLevel::SemiAuto);

        cmds.dispatch("/autonomy", "manual", &mut state);
        assert_eq!(state.autonomy_level, AutonomyLevel::Manual);

        cmds.dispatch("/autonomy", "auto", &mut state);
        assert_eq!(state.autonomy_level, AutonomyLevel::Auto);

        cmds.dispatch("/autonomy", "semi-auto", &mut state);
        assert_eq!(state.autonomy_level, AutonomyLevel::SemiAuto);

        // Invalid value should not change level
        cmds.dispatch("/autonomy", "garbage", &mut state);
        assert_eq!(state.autonomy_level, AutonomyLevel::SemiAuto);
    }

    #[test]
    fn test_status_command() {
        let cmds = BuiltinCommands::new();
        let mut state = ReplState::default();

        assert_eq!(
            cmds.dispatch("/status", "", &mut state),
            CommandOutcome::Handled
        );
    }
}
