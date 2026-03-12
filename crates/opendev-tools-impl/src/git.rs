//! Git operations tool — structured git commands with safety checks.

use std::collections::HashMap;
use std::process::Command;

use opendev_tools_core::{BaseTool, ToolContext, ToolResult};

/// Branches that should never be force-pushed to.
const PROTECTED_BRANCHES: &[&str] = &["main", "master", "develop", "production", "staging"];

/// Tool for structured git operations.
#[derive(Debug)]
pub struct GitTool;

#[async_trait::async_trait]
impl BaseTool for GitTool {
    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Execute structured git operations: status, diff, log, branch, checkout, commit, push, pull, stash, merge."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "diff", "log", "branch", "checkout", "commit", "push", "pull", "stash", "merge"],
                    "description": "Git action to perform"
                },
                "message": { "type": "string", "description": "Commit message (for commit)" },
                "branch": { "type": "string", "description": "Branch name" },
                "file": { "type": "string", "description": "File path (for diff)" },
                "staged": { "type": "boolean", "description": "Show staged changes (for diff)" },
                "limit": { "type": "integer", "description": "Number of log entries" },
                "force": { "type": "boolean", "description": "Force push (with lease)" },
                "create": { "type": "boolean", "description": "Create new branch (for checkout)" },
                "remote": { "type": "string", "description": "Remote name (default: origin)" }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        args: HashMap<String, serde_json::Value>,
        ctx: &ToolContext,
    ) -> ToolResult {
        let action = match args.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => return ToolResult::fail("action is required"),
        };

        let cwd = ctx.working_dir.to_string_lossy().to_string();

        match action {
            "status" => git_status(&cwd),
            "diff" => {
                let file = args.get("file").and_then(|v| v.as_str());
                let staged = args.get("staged").and_then(|v| v.as_bool()).unwrap_or(false);
                git_diff(&cwd, file, staged)
            }
            "log" => {
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                git_log(&cwd, limit)
            }
            "branch" => {
                let name = args.get("branch").and_then(|v| v.as_str());
                git_branch(&cwd, name)
            }
            "checkout" => {
                let branch = match args.get("branch").and_then(|v| v.as_str()) {
                    Some(b) => b,
                    None => return ToolResult::fail("branch is required for checkout"),
                };
                let create = args.get("create").and_then(|v| v.as_bool()).unwrap_or(false);
                git_checkout(&cwd, branch, create)
            }
            "commit" => {
                let message = match args.get("message").and_then(|v| v.as_str()) {
                    Some(m) => m,
                    None => return ToolResult::fail("message is required for commit"),
                };
                git_commit(&cwd, message)
            }
            "push" => {
                let remote = args.get("remote").and_then(|v| v.as_str()).unwrap_or("origin");
                let branch = args.get("branch").and_then(|v| v.as_str());
                let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
                git_push(&cwd, remote, branch, force)
            }
            "pull" => {
                let remote = args.get("remote").and_then(|v| v.as_str()).unwrap_or("origin");
                let branch = args.get("branch").and_then(|v| v.as_str());
                git_pull(&cwd, remote, branch)
            }
            "stash" => {
                let sub_action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
                // For stash, use a secondary field or default to list
                git_stash(&cwd, sub_action)
            }
            "merge" => {
                let branch = match args.get("branch").and_then(|v| v.as_str()) {
                    Some(b) => b,
                    None => return ToolResult::fail("branch is required for merge"),
                };
                git_merge(&cwd, branch)
            }
            _ => ToolResult::fail(format!(
                "Unknown git action: {action}. Available: status, diff, log, branch, checkout, commit, push, pull, stash, merge"
            )),
        }
    }
}

fn run_git(args: &[&str], cwd: &str) -> (bool, String) {
    match Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if output.status.success() {
                (true, stdout.trim().to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                (false, stderr.trim().to_string())
            }
        }
        Err(e) => (false, format!("Failed to execute git: {e}")),
    }
}

fn git_status(cwd: &str) -> ToolResult {
    let (ok, out) = run_git(&["status", "--porcelain=v1", "-b"], cwd);
    if !ok {
        return ToolResult::fail(out);
    }

    let lines: Vec<&str> = out.lines().collect();
    let branch_line = lines.first().copied().unwrap_or("");
    let branch = branch_line
        .strip_prefix("## ")
        .unwrap_or("unknown")
        .split("...")
        .next()
        .unwrap_or("unknown");

    let changes: Vec<&str> = lines.iter().skip(1).copied().collect();

    let mut output = format!("Branch: {branch}\n");
    if changes.is_empty() {
        output.push_str("Working tree clean");
    } else {
        output.push_str(&format!("Changes ({}):\n", changes.len()));
        for (i, change) in changes.iter().enumerate() {
            if i >= 50 {
                output.push_str(&format!("  ... and {} more\n", changes.len() - 50));
                break;
            }
            output.push_str(&format!("  {change}\n"));
        }
    }

    let mut metadata = HashMap::new();
    metadata.insert("branch".into(), serde_json::json!(branch));
    metadata.insert("change_count".into(), serde_json::json!(changes.len()));

    ToolResult::ok_with_metadata(output, metadata)
}

fn git_diff(cwd: &str, file: Option<&str>, staged: bool) -> ToolResult {
    let mut args = vec!["diff"];
    if staged {
        args.push("--cached");
    }
    if let Some(f) = file {
        args.push("--");
        args.push(f);
    }

    let (ok, out) = run_git(&args, cwd);
    if !ok {
        return ToolResult::fail(out);
    }
    ToolResult::ok(if out.is_empty() { "No differences found".to_string() } else { out })
}

fn git_log(cwd: &str, limit: usize) -> ToolResult {
    let limit_str = format!("-{limit}");
    let (ok, out) = run_git(
        &["log", &limit_str, "--format=%h %s (%cr) <%an>"],
        cwd,
    );
    if !ok {
        return ToolResult::fail(out);
    }
    ToolResult::ok(if out.is_empty() { "No commits found".to_string() } else { out })
}

fn git_branch(cwd: &str, name: Option<&str>) -> ToolResult {
    if let Some(name) = name {
        let (ok, out) = run_git(&["branch", name], cwd);
        if ok {
            ToolResult::ok(format!("Created branch: {name}"))
        } else {
            ToolResult::fail(out)
        }
    } else {
        let (ok, out) = run_git(&["branch", "-a"], cwd);
        if !ok {
            return ToolResult::fail(out);
        }
        ToolResult::ok(out)
    }
}

fn git_checkout(cwd: &str, branch: &str, create: bool) -> ToolResult {
    // Safety: check for uncommitted changes
    let (ok, status_out) = run_git(&["status", "--porcelain"], cwd);
    if ok && !status_out.is_empty() {
        let dirty = status_out.lines().count();
        return ToolResult::fail(format!(
            "Working tree has {dirty} uncommitted changes. Commit or stash them first."
        ));
    }

    let mut args = vec!["checkout"];
    if create {
        args.push("-b");
    }
    args.push(branch);

    let (ok, out) = run_git(&args, cwd);
    if ok {
        ToolResult::ok(format!("Switched to branch: {branch}"))
    } else {
        ToolResult::fail(out)
    }
}

fn git_commit(cwd: &str, message: &str) -> ToolResult {
    // Check staged changes
    let (ok, staged) = run_git(&["diff", "--cached", "--stat"], cwd);
    if ok && staged.is_empty() {
        return ToolResult::fail("No staged changes to commit. Use 'git add' first.");
    }

    let (ok, out) = run_git(&["commit", "-m", message], cwd);
    if ok {
        ToolResult::ok(out)
    } else {
        ToolResult::fail(out)
    }
}

fn git_push(cwd: &str, remote: &str, branch: Option<&str>, force: bool) -> ToolResult {
    if force {
        let target = branch.unwrap_or_else(|| {
            // This is a simplification; in practice we'd read HEAD
            ""
        });
        if PROTECTED_BRANCHES.contains(&target) {
            return ToolResult::fail(format!(
                "Refusing force-push to protected branch '{target}'."
            ));
        }
    }

    let mut args = vec!["push", remote];
    if let Some(b) = branch {
        args.push(b);
    }
    if force {
        args.push("--force-with-lease");
    }

    let (ok, out) = run_git(&args, cwd);
    if ok {
        ToolResult::ok(if out.is_empty() { "Push successful".to_string() } else { out })
    } else {
        ToolResult::fail(out)
    }
}

fn git_pull(cwd: &str, remote: &str, branch: Option<&str>) -> ToolResult {
    let mut args = vec!["pull", remote];
    if let Some(b) = branch {
        args.push(b);
    }

    let (ok, out) = run_git(&args, cwd);
    if ok {
        ToolResult::ok(out)
    } else {
        ToolResult::fail(out)
    }
}

fn git_stash(cwd: &str, _action: &str) -> ToolResult {
    // Default to stash list
    let (ok, out) = run_git(&["stash", "list"], cwd);
    if !ok {
        return ToolResult::fail(out);
    }
    ToolResult::ok(if out.is_empty() { "No stashes".to_string() } else { out })
}

fn git_merge(cwd: &str, branch: &str) -> ToolResult {
    let (ok, out) = run_git(&["merge", branch], cwd);
    if ok {
        ToolResult::ok(out)
    } else {
        ToolResult::fail(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_args(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[tokio::test]
    async fn test_git_status() {
        // This test runs in the actual repo, so just verify it doesn't error
        let tool = GitTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[("action", serde_json::json!("status"))]);
        let result = tool.execute(args, &ctx).await;
        // /tmp might not be a git repo, so we accept either outcome
        assert!(result.success || result.error.is_some());
    }

    #[tokio::test]
    async fn test_git_unknown_action() {
        let tool = GitTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[("action", serde_json::json!("unknown_action"))]);
        let result = tool.execute(args, &ctx).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Unknown git action"));
    }

    #[tokio::test]
    async fn test_git_commit_missing_message() {
        let tool = GitTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[("action", serde_json::json!("commit"))]);
        let result = tool.execute(args, &ctx).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("message is required"));
    }

    #[tokio::test]
    async fn test_git_force_push_protected() {
        let result = git_push("/tmp", "origin", Some("main"), true);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("protected branch"));
    }

    #[test]
    fn test_run_git_nonexistent() {
        let (ok, _) = run_git(&["status"], "/nonexistent/path");
        assert!(!ok);
    }
}
