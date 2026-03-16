//! Slash command execution: /mode, /thinking, /models, /help, etc.

use crate::event::AppEvent;

use super::{App, AutonomyLevel, DisplayMessage, DisplayRole, OperationMode, ThinkingLevel};

impl App {
    pub(super) fn push_system_message(&mut self, content: String) {
        self.state.messages.push(DisplayMessage {
            role: DisplayRole::System,
            content,
            tool_call: None,
            collapsed: false,
        });
        self.state.message_generation += 1;
    }

    /// Execute a slash command locally.
    pub(super) fn execute_slash_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd[1..].splitn(2, ' ').collect();
        let name = parts[0];
        let args = parts.get(1).map(|s| s.trim());

        match name {
            "exit" | "quit" | "q" => {
                self.state.running = false;
            }
            "clear" => {
                self.state.messages.clear();
                self.state.scroll_offset = 0;
                self.state.user_scrolled = false;
                self.state.message_generation += 1;
            }
            "mode" => {
                match args {
                    Some(arg) => {
                        if let Some(mode) = OperationMode::from_str_loose(arg) {
                            self.state.mode = mode;
                        } else {
                            self.push_system_message(format!(
                                "Unknown mode: '{arg}'. Use: normal, plan"
                            ));
                            return;
                        }
                    }
                    None => {
                        self.state.mode = match self.state.mode {
                            OperationMode::Normal => OperationMode::Plan,
                            OperationMode::Plan => OperationMode::Normal,
                        };
                    }
                }
                self.push_system_message(format!("Mode: {}", self.state.mode));
            }
            "thinking" => {
                match args {
                    Some(arg) => {
                        if let Some(level) = ThinkingLevel::from_str_loose(arg) {
                            self.state.thinking_level = level;
                        } else {
                            self.push_system_message(format!(
                                "Unknown thinking level: '{arg}'. Use: off, low, medium, high"
                            ));
                            return;
                        }
                    }
                    None => {
                        self.state.thinking_level = match self.state.thinking_level {
                            ThinkingLevel::Off => ThinkingLevel::Low,
                            ThinkingLevel::Low => ThinkingLevel::Medium,
                            ThinkingLevel::Medium => ThinkingLevel::High,
                            ThinkingLevel::High => ThinkingLevel::Off,
                        };
                    }
                }
                self.push_system_message(format!("Thinking: {}", self.state.thinking_level));
            }
            "autonomy" => {
                match args {
                    Some(arg) => {
                        if let Some(level) = AutonomyLevel::from_str_loose(arg) {
                            self.state.autonomy = level;
                        } else {
                            self.push_system_message(format!(
                                "Unknown autonomy level: '{arg}'. Use: manual, semi-auto, auto"
                            ));
                            return;
                        }
                    }
                    None => {
                        self.state.autonomy = match self.state.autonomy {
                            AutonomyLevel::Manual => AutonomyLevel::SemiAuto,
                            AutonomyLevel::SemiAuto => AutonomyLevel::Auto,
                            AutonomyLevel::Auto => AutonomyLevel::Manual,
                        };
                    }
                }
                self.push_system_message(format!("Autonomy: {}", self.state.autonomy));
            }
            "models" | "session-models" => {
                let scope = if name == "session-models" {
                    "session"
                } else {
                    "global"
                };
                match args {
                    Some("clear") if name == "session-models" => {
                        self.push_system_message(
                            "Session model override cleared. Using global model.".to_string(),
                        );
                    }
                    Some(model_name) => {
                        self.state.model = model_name.to_string();
                        self.push_system_message(format!(
                            "Model set to: {} ({scope})",
                            self.state.model
                        ));
                    }
                    None => {
                        self.push_system_message(format!(
                            "Current model: {}\nUsage: /{name} <model-name>",
                            self.state.model
                        ));
                    }
                }
            }
            "mcp" => {
                let result = self.mcp_controller.handle_command(args.unwrap_or(""));
                self.push_system_message(result);
            }
            "tasks" => {
                let msg = if let Ok(mgr) = self.task_manager.try_lock() {
                    let tasks = mgr.all_tasks();
                    if tasks.is_empty() {
                        "No background tasks.".to_string()
                    } else {
                        let mut lines = vec![format!(
                            "Background tasks ({} total, {} running):",
                            tasks.len(),
                            mgr.running_count()
                        )];
                        for task in &tasks {
                            lines.push(format!(
                                "  {} [{}] {} ({:.1}s)",
                                task.task_id,
                                task.state,
                                task.description,
                                task.runtime_seconds()
                            ));
                        }
                        lines.join("\n")
                    }
                } else {
                    "Task manager busy. Try again.".to_string()
                };
                self.push_system_message(msg);
            }
            "task" => match args {
                Some(id) => {
                    let msg = if let Ok(mgr) = self.task_manager.try_lock() {
                        let output = mgr.read_output(id, 50);
                        if output.is_empty() {
                            format!("No output for task '{id}'.")
                        } else {
                            format!("Output for task {id}:\n{output}")
                        }
                    } else {
                        "Task manager busy. Try again.".to_string()
                    };
                    self.push_system_message(msg);
                }
                None => {
                    self.push_system_message("Usage: /task <id>".to_string());
                }
            },
            "kill" => match args {
                Some(id) => {
                    let id = id.to_string();
                    let _ = self.event_tx.send(AppEvent::KillTask(id));
                }
                None => {
                    self.push_system_message("Usage: /kill <id>".to_string());
                }
            },
            "init" => {
                let path = args.unwrap_or(".");
                self.push_system_message(format!(
                    "Analyzing codebase at '{path}' and generating OPENDEV.md...\n\
                     Send a message to the agent to perform initialization."
                ));
            }
            "agents" => match args {
                Some("create") => {
                    self.push_system_message("Agent creation coming soon.".to_string());
                }
                _ => {
                    self.push_system_message("No custom agents configured.".to_string());
                }
            },
            "skills" => match args {
                Some("create") => {
                    self.push_system_message("Skill creation coming soon.".to_string());
                }
                _ => {
                    self.push_system_message("No custom skills configured.".to_string());
                }
            },
            "plugins" => match args {
                Some("install") => {
                    self.push_system_message("Plugin installation coming soon.".to_string());
                }
                Some("remove") => {
                    self.push_system_message("Plugin removal coming soon.".to_string());
                }
                _ => {
                    self.push_system_message("No plugins installed.".to_string());
                }
            },
            "sound" => {
                opendev_runtime::play_finish_sound();
                self.push_system_message("Playing test sound...".to_string());
            }
            "compact" => {
                if self.state.messages.len() < 5 {
                    self.push_system_message(
                        "Not enough messages to compact (need at least 5).".to_string(),
                    );
                } else if self.state.compaction_active {
                    self.push_system_message("Compaction already in progress.".to_string());
                } else if self.state.agent_active {
                    self.push_system_message("Cannot compact while agent is running.".to_string());
                } else {
                    // Send special sentinel to trigger compaction in the backend
                    if let Some(ref tx) = self.user_message_tx {
                        let _ = tx.send("\x00__COMPACT__".to_string());
                    }
                }
            }
            "help" => {
                self.push_system_message(
                    [
                        "Available commands:",
                        "  /help              — Show this help",
                        "  /clear             — Clear conversation",
                        "  /mode [plan|normal]      — Toggle or set mode",
                        "  /thinking [off|low|medium|high] — Cycle or set thinking level",
                        "  /autonomy [manual|semi-auto|auto] — Cycle or set autonomy",
                        "  /models [name]     — Show or set model (global)",
                        "  /session-models [name|clear] — Set model for session",
                        "  /mcp [list|add|remove|enable|disable] — Manage MCP servers",
                        "  /tasks             — List background tasks",
                        "  /task <id>         — Show task output",
                        "  /kill <id>         — Kill a background task",
                        "  /init [path]       — Generate OPENDEV.md",
                        "  /agents [list|create] — Manage custom agents",
                        "  /skills [list|create] — Manage custom skills",
                        "  /plugins [list|install|remove] — Manage plugins",
                        "  /sound             — Play test notification sound",
                        "  /compact           — Compact conversation context",
                        "  /exit              — Quit OpenDev",
                        "",
                        "Keyboard shortcuts:",
                        "  Ctrl+C      — Clear input / interrupt / quit",
                        "  Escape      — Interrupt agent",
                        "  Shift+Tab   — Toggle mode",
                        "  PageUp/Down — Scroll conversation",
                    ]
                    .join("\n"),
                );
            }
            _ => {
                self.push_system_message(format!(
                    "Unknown command: /{name}. Type /help for available commands."
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn test_slash_mode_with_arg() {
        let mut app = App::new();
        assert_eq!(app.state.mode, OperationMode::Normal);
        app.execute_slash_command("/mode plan");
        assert_eq!(app.state.mode, OperationMode::Plan);
        app.execute_slash_command("/mode normal");
        assert_eq!(app.state.mode, OperationMode::Normal);
    }

    #[test]
    fn test_slash_mode_bad_arg() {
        let mut app = App::new();
        app.execute_slash_command("/mode bogus");
        // Mode should not change
        assert_eq!(app.state.mode, OperationMode::Normal);
        // Should have an error message
        assert!(
            app.state
                .messages
                .last()
                .unwrap()
                .content
                .contains("Unknown mode")
        );
    }

    #[test]
    fn test_slash_mode_no_arg_toggles() {
        let mut app = App::new();
        app.execute_slash_command("/mode");
        assert_eq!(app.state.mode, OperationMode::Plan);
        app.execute_slash_command("/mode");
        assert_eq!(app.state.mode, OperationMode::Normal);
    }

    #[test]
    fn test_slash_thinking_with_arg() {
        let mut app = App::new();
        app.execute_slash_command("/thinking high");
        assert_eq!(app.state.thinking_level, ThinkingLevel::High);
        app.execute_slash_command("/thinking off");
        assert_eq!(app.state.thinking_level, ThinkingLevel::Off);
    }

    #[test]
    fn test_slash_thinking_bad_arg() {
        let mut app = App::new();
        let original = app.state.thinking_level;
        app.execute_slash_command("/thinking bogus");
        assert_eq!(app.state.thinking_level, original);
        assert!(
            app.state
                .messages
                .last()
                .unwrap()
                .content
                .contains("Unknown thinking")
        );
    }

    #[test]
    fn test_slash_thinking_no_arg_cycles() {
        let mut app = App::new();
        // Default is Medium
        assert_eq!(app.state.thinking_level, ThinkingLevel::Medium);
        app.execute_slash_command("/thinking");
        assert_eq!(app.state.thinking_level, ThinkingLevel::High);
        app.execute_slash_command("/thinking");
        assert_eq!(app.state.thinking_level, ThinkingLevel::Off);
    }

    #[test]
    fn test_slash_autonomy_with_arg() {
        let mut app = App::new();
        app.execute_slash_command("/autonomy auto");
        assert_eq!(app.state.autonomy, AutonomyLevel::Auto);
        app.execute_slash_command("/autonomy manual");
        assert_eq!(app.state.autonomy, AutonomyLevel::Manual);
        app.execute_slash_command("/autonomy semi-auto");
        assert_eq!(app.state.autonomy, AutonomyLevel::SemiAuto);
    }

    #[test]
    fn test_slash_autonomy_bad_arg() {
        let mut app = App::new();
        app.execute_slash_command("/autonomy bogus");
        assert_eq!(app.state.autonomy, AutonomyLevel::Manual);
        assert!(
            app.state
                .messages
                .last()
                .unwrap()
                .content
                .contains("Unknown autonomy")
        );
    }

    #[test]
    fn test_slash_models_show_current() {
        let mut app = App::new();
        app.execute_slash_command("/models");
        assert!(
            app.state
                .messages
                .last()
                .unwrap()
                .content
                .contains("claude-sonnet-4")
        );
    }

    #[test]
    fn test_slash_models_set() {
        let mut app = App::new();
        app.execute_slash_command("/models gpt-4o");
        assert_eq!(app.state.model, "gpt-4o");
        assert!(
            app.state
                .messages
                .last()
                .unwrap()
                .content
                .contains("gpt-4o")
        );
    }

    #[test]
    fn test_slash_tasks_empty() {
        let mut app = App::new();
        app.execute_slash_command("/tasks");
        assert!(
            app.state
                .messages
                .last()
                .unwrap()
                .content
                .contains("No background tasks")
        );
    }

    #[test]
    fn test_slash_task_no_arg() {
        let mut app = App::new();
        app.execute_slash_command("/task");
        assert!(app.state.messages.last().unwrap().content.contains("Usage"));
    }

    #[test]
    fn test_slash_kill_no_arg() {
        let mut app = App::new();
        app.execute_slash_command("/kill");
        assert!(app.state.messages.last().unwrap().content.contains("Usage"));
    }

    #[test]
    fn test_slash_mcp_list_empty() {
        let mut app = App::new();
        app.execute_slash_command("/mcp list");
        assert!(
            app.state
                .messages
                .last()
                .unwrap()
                .content
                .contains("No MCP servers")
        );
    }

    #[test]
    fn test_slash_init() {
        let mut app = App::new();
        app.execute_slash_command("/init");
        assert!(
            app.state
                .messages
                .last()
                .unwrap()
                .content
                .contains("OPENDEV.md")
        );
    }

    #[test]
    fn test_slash_agents() {
        let mut app = App::new();
        app.execute_slash_command("/agents");
        assert!(
            app.state
                .messages
                .last()
                .unwrap()
                .content
                .contains("No custom agents")
        );
    }

    #[test]
    fn test_slash_skills() {
        let mut app = App::new();
        app.execute_slash_command("/skills");
        assert!(
            app.state
                .messages
                .last()
                .unwrap()
                .content
                .contains("No custom skills")
        );
    }

    #[test]
    fn test_slash_plugins() {
        let mut app = App::new();
        app.execute_slash_command("/plugins");
        assert!(
            app.state
                .messages
                .last()
                .unwrap()
                .content
                .contains("No plugins")
        );
    }

    #[test]
    fn test_slash_help_lists_all_commands() {
        let mut app = App::new();
        app.execute_slash_command("/help");
        let help = &app.state.messages.last().unwrap().content;
        // Check that all major commands appear
        for cmd in &[
            "mode", "thinking", "autonomy", "models", "mcp", "tasks", "task", "kill", "agents",
            "skills", "plugins",
        ] {
            assert!(help.contains(cmd), "Help text missing /{cmd}");
        }
    }
}
