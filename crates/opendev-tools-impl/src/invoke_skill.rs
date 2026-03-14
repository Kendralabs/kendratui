//! invoke_skill tool — loads skill content into conversation context on demand.
//!
//! Mirrors the Python `_handle_invoke_skill` in `registry.py`.
//! Supports listing available skills, loading by name (with namespace),
//! and session-scoped deduplication to avoid re-loading the same skill.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use opendev_tools_core::{BaseTool, ToolContext, ToolResult};

use opendev_agents::skills::SkillLoader;

/// Tool that loads skill content into the conversation context.
///
/// Skills are markdown files with YAML frontmatter discovered from:
/// - `<project>/.opendev/skills/` (highest priority)
/// - `~/.opendev/skills/`
/// - Built-in skills embedded in the binary
#[derive(Debug)]
pub struct InvokeSkillTool {
    skill_loader: Arc<Mutex<SkillLoader>>,
    /// Tracks which skills have been invoked this session to avoid re-loading.
    invoked_skills: Mutex<HashSet<String>>,
}

impl InvokeSkillTool {
    /// Create a new invoke_skill tool with a shared skill loader.
    pub fn new(skill_loader: Arc<Mutex<SkillLoader>>) -> Self {
        Self {
            skill_loader,
            invoked_skills: Mutex::new(HashSet::new()),
        }
    }
}

#[async_trait::async_trait]
impl BaseTool for InvokeSkillTool {
    fn name(&self) -> &str {
        "invoke_skill"
    }

    fn description(&self) -> &str {
        "Load a skill's knowledge and instructions into the current conversation context. \
         Call without skill_name to list available skills."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "skill_name": {
                    "type": "string",
                    "description": "Name of the skill to load (e.g. 'commit', 'git:rebase'). Omit to list all available skills."
                }
            },
            "required": []
        })
    }

    async fn execute(
        &self,
        args: HashMap<String, serde_json::Value>,
        _ctx: &ToolContext,
    ) -> ToolResult {
        let skill_name = args
            .get("skill_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        let mut loader = match self.skill_loader.lock() {
            Ok(l) => l,
            Err(_) => return ToolResult::fail("Failed to acquire skill loader lock"),
        };

        // No skill_name provided — list available skills.
        if skill_name.is_empty() {
            let names = loader.get_skill_names();
            if names.is_empty() {
                return ToolResult::ok("No skills available.");
            }
            let mut sorted = names;
            sorted.sort();
            return ToolResult::ok(format!("Available skills: {}", sorted.join(", ")));
        }

        // Try to load the skill.
        let skill = match loader.load_skill(skill_name) {
            Some(s) => s,
            None => {
                let names = loader.get_skill_names();
                let available = if names.is_empty() {
                    "None".to_string()
                } else {
                    let mut sorted = names;
                    sorted.sort();
                    sorted.join(", ")
                };
                return ToolResult::fail(format!(
                    "Skill not found: '{skill_name}'. Available: {available}"
                ));
            }
        };

        // Dedup: if already invoked this session, return a short reminder.
        if let Ok(mut invoked) = self.invoked_skills.lock() {
            if invoked.contains(skill_name) {
                let mut meta = HashMap::new();
                meta.insert(
                    "skill_name".to_string(),
                    serde_json::json!(skill.metadata.name),
                );
                meta.insert(
                    "skill_namespace".to_string(),
                    serde_json::json!(skill.metadata.namespace),
                );
                return ToolResult::ok_with_metadata(
                    format!(
                        "Skill '{}' is already loaded in this conversation. \
                         Refer to the skill content above and proceed with the next action step — \
                         do not invoke this skill again.",
                        skill.metadata.name
                    ),
                    meta,
                );
            }
            invoked.insert(skill_name.to_string());
        }

        // Return the full skill content with metadata.
        let mut meta = HashMap::new();
        meta.insert(
            "skill_name".to_string(),
            serde_json::json!(skill.metadata.name),
        );
        meta.insert(
            "skill_namespace".to_string(),
            serde_json::json!(skill.metadata.namespace),
        );

        ToolResult::ok_with_metadata(
            format!("Loaded skill: {}\n\n{}", skill.metadata.name, skill.content),
            meta,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_loader(skill_dir: Option<&std::path::Path>) -> Arc<Mutex<SkillLoader>> {
        let dirs = match skill_dir {
            Some(d) => vec![d.to_path_buf()],
            None => vec![],
        };
        let mut loader = SkillLoader::new(dirs);
        loader.discover_skills();
        Arc::new(Mutex::new(loader))
    }

    #[tokio::test]
    async fn test_list_skills_no_arg() {
        let loader = create_test_loader(None);
        let tool = InvokeSkillTool::new(loader);
        let ctx = ToolContext::new("/tmp/test");

        let result = tool.execute(HashMap::new(), &ctx).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("Available skills:"));
        assert!(output.contains("commit"));
    }

    #[tokio::test]
    async fn test_list_skills_empty_string() {
        let loader = create_test_loader(None);
        let tool = InvokeSkillTool::new(loader);
        let ctx = ToolContext::new("/tmp/test");

        let mut args = HashMap::new();
        args.insert("skill_name".to_string(), serde_json::json!(""));

        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("Available skills:"));
    }

    #[tokio::test]
    async fn test_load_builtin_skill() {
        let loader = create_test_loader(None);
        let tool = InvokeSkillTool::new(loader);
        let ctx = ToolContext::new("/tmp/test");

        let mut args = HashMap::new();
        args.insert("skill_name".to_string(), serde_json::json!("commit"));

        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("Loaded skill: commit"));
        assert!(output.contains("Git Commit"));
        assert_eq!(result.metadata.get("skill_name").unwrap(), "commit");
        assert_eq!(result.metadata.get("skill_namespace").unwrap(), "default");
    }

    #[tokio::test]
    async fn test_skill_not_found() {
        let loader = create_test_loader(None);
        let tool = InvokeSkillTool::new(loader);
        let ctx = ToolContext::new("/tmp/test");

        let mut args = HashMap::new();
        args.insert(
            "skill_name".to_string(),
            serde_json::json!("nonexistent-skill-xyz"),
        );

        let result = tool.execute(args, &ctx).await;
        assert!(!result.success);
        let error = result.error.unwrap();
        assert!(error.contains("Skill not found: 'nonexistent-skill-xyz'"));
        assert!(error.contains("Available:"));
    }

    #[tokio::test]
    async fn test_dedup_second_invoke_returns_reminder() {
        let loader = create_test_loader(None);
        let tool = InvokeSkillTool::new(loader);
        let ctx = ToolContext::new("/tmp/test");

        let mut args = HashMap::new();
        args.insert("skill_name".to_string(), serde_json::json!("commit"));

        // First invoke: full content.
        let result1 = tool.execute(args.clone(), &ctx).await;
        assert!(result1.success);
        assert!(result1.output.unwrap().contains("Loaded skill: commit"));

        // Second invoke: dedup reminder.
        let result2 = tool.execute(args, &ctx).await;
        assert!(result2.success);
        let output2 = result2.output.unwrap();
        assert!(output2.contains("already loaded"));
        assert!(output2.contains("do not invoke this skill again"));
    }

    #[tokio::test]
    async fn test_load_filesystem_skill() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("deploy.md"),
            "---\nname: deploy\ndescription: Deploy instructions\nnamespace: ops\n---\n\n# Deploy\nStep 1: push.\n",
        ).unwrap();

        let loader = create_test_loader(Some(&skill_dir));
        let tool = InvokeSkillTool::new(loader);
        let ctx = ToolContext::new("/tmp/test");

        let mut args = HashMap::new();
        args.insert("skill_name".to_string(), serde_json::json!("deploy"));

        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("Loaded skill: deploy"));
        assert!(output.contains("Step 1: push."));
        assert_eq!(result.metadata.get("skill_namespace").unwrap(), "ops");
    }

    #[tokio::test]
    async fn test_load_namespaced_skill() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("rebase.md"),
            "---\nname: rebase\ndescription: Git rebase\nnamespace: git\n---\n\n# Rebase\n",
        )
        .unwrap();

        let loader = create_test_loader(Some(&skill_dir));
        let tool = InvokeSkillTool::new(loader);
        let ctx = ToolContext::new("/tmp/test");

        let mut args = HashMap::new();
        args.insert("skill_name".to_string(), serde_json::json!("git:rebase"));

        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("Loaded skill: rebase"));
        assert_eq!(result.metadata.get("skill_namespace").unwrap(), "git");
    }
}
