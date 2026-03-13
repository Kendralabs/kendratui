//! Skills system for lazy-loaded knowledge modules.
//!
//! Skills are markdown files with YAML frontmatter that inject knowledge and
//! instructions into the main agent context on demand. Unlike subagents
//! (separate sessions), skills extend the current conversation's capabilities.
//!
//! ## Directory Structure
//! Skills are loaded from (in priority order):
//! - `<project>/.opendev/skills/` (project local, highest priority)
//! - `~/.opendev/skills/` (user global)
//! - Built-in skills embedded in the binary
//!
//! ## Skill File Format
//! ```markdown
//! ---
//! name: commit
//! description: Git commit best practices
//! namespace: default
//! ---
//!
//! # Git Commit Skill
//! When making commits: ...
//! ```

mod metadata;

pub use metadata::{LoadedSkill, SkillMetadata, SkillSource};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use regex::Regex;
use tracing::{debug, warn};

// ============================================================================
// Built-in skills, embedded at compile time
// ============================================================================

struct BuiltinSkill {
    filename: &'static str,
    content: &'static str,
}

const BUILTIN_SKILLS: &[BuiltinSkill] = &[
    BuiltinSkill {
        filename: "commit.md",
        content: include_str!("builtin/commit.md"),
    },
    BuiltinSkill {
        filename: "review-pr.md",
        content: include_str!("builtin/review-pr.md"),
    },
    BuiltinSkill {
        filename: "create-pr.md",
        content: include_str!("builtin/create-pr.md"),
    },
];

// ============================================================================
// SkillLoader
// ============================================================================

/// Discovers and loads skills from configured directories and builtins.
///
/// Skills are discovered lazily -- only metadata is read at startup.
/// Full content is loaded on-demand when the skill is invoked.
#[derive(Debug)]
pub struct SkillLoader {
    /// Directories to scan, in priority order (first = highest priority).
    dirs: Vec<PathBuf>,
    /// Cache of fully loaded skills (name -> LoadedSkill).
    cache: HashMap<String, LoadedSkill>,
    /// Cache of discovered metadata (full_name -> SkillMetadata).
    metadata_cache: HashMap<String, SkillMetadata>,
}

impl SkillLoader {
    /// Create a new skill loader.
    ///
    /// `skill_dirs` is in priority order: first directory has highest priority
    /// (typically project local). Directories that do not exist are tolerated.
    pub fn new(skill_dirs: Vec<PathBuf>) -> Self {
        Self {
            dirs: skill_dirs,
            cache: HashMap::new(),
            metadata_cache: HashMap::new(),
        }
    }

    /// Scan skill directories and builtins for `.md` files, extract metadata.
    ///
    /// Project-local skills override user-global skills with the same name.
    /// User skills override builtins with the same name.
    ///
    /// Returns a list of all discovered [`SkillMetadata`].
    pub fn discover_skills(&mut self) -> Vec<SkillMetadata> {
        let mut skills: HashMap<String, SkillMetadata> = HashMap::new();

        // Process builtins first (lowest priority).
        for builtin in BUILTIN_SKILLS {
            if let Some(mut meta) = parse_frontmatter_str(builtin.content) {
                meta.source = SkillSource::Builtin;
                // Use the filename stem as a fallback name.
                if meta.name.is_empty() {
                    meta.name = builtin
                        .filename
                        .strip_suffix(".md")
                        .unwrap_or(builtin.filename)
                        .to_string();
                }
                let full_name = meta.full_name();
                skills.insert(full_name, meta);
            }
        }

        // Process directories in reverse order so higher-priority dirs override.
        for skill_dir in self.dirs.iter().rev() {
            if !skill_dir.exists() {
                continue;
            }

            let source = detect_source(skill_dir);

            // Scan for markdown files (both flat *.md and dir/SKILL.md patterns).
            if let Ok(entries) = glob_md_files(skill_dir) {
                for md_file in entries {
                    if let Some(mut meta) = parse_frontmatter_file(&md_file) {
                        meta.path = Some(md_file);
                        meta.source = source.clone();
                        let full_name = meta.full_name();
                        skills.insert(full_name, meta);
                    }
                }
            }
        }

        self.metadata_cache = skills;
        self.metadata_cache.values().cloned().collect()
    }

    /// Load full skill content by name.
    ///
    /// `name` can be a plain name (e.g. `"commit"`) or namespaced
    /// (e.g. `"git:commit"`). Returns `None` if not found.
    pub fn load_skill(&mut self, name: &str) -> Option<LoadedSkill> {
        // Check cache.
        if let Some(cached) = self.cache.get(name) {
            return Some(cached.clone());
        }

        // Ensure metadata is loaded.
        if self.metadata_cache.is_empty() {
            self.discover_skills();
        }

        // Look up by full name first.
        let metadata = self.metadata_cache.get(name).cloned().or_else(|| {
            // Fall back: search by bare name.
            self.metadata_cache
                .values()
                .find(|m| m.name == name)
                .cloned()
        });

        let metadata = match metadata {
            Some(m) => m,
            None => {
                warn!(skill = name, "skill not found");
                return None;
            }
        };

        // Load full content.
        let raw_content = match &metadata.source {
            SkillSource::Builtin => {
                // Find the builtin by name.
                BUILTIN_SKILLS
                    .iter()
                    .find(|b| {
                        let stem = b.filename.strip_suffix(".md").unwrap_or(b.filename);
                        stem == metadata.name
                    })
                    .map(|b| b.content.to_string())
            }
            _ => {
                // Read from disk.
                metadata.path.as_ref().and_then(|p| {
                    std::fs::read_to_string(p)
                        .map_err(|e| {
                            warn!(path = %p.display(), error = %e, "failed to read skill file");
                            e
                        })
                        .ok()
                })
            }
        };

        let raw_content = raw_content?;
        let content = strip_frontmatter(&raw_content);

        let skill = LoadedSkill {
            metadata: metadata.clone(),
            content,
        };

        self.cache.insert(name.to_string(), skill.clone());
        Some(skill)
    }

    /// Build a formatted skills index for inclusion in system prompts.
    ///
    /// Returns an empty string if no skills are available.
    pub fn build_skills_index(&mut self) -> String {
        let skills = self.discover_skills();
        if skills.is_empty() {
            return String::new();
        }

        let mut sorted = skills;
        sorted.sort_by(|a, b| (&a.namespace, &a.name).cmp(&(&b.namespace, &b.name)));

        let mut lines = vec![
            "## Available Skills".to_string(),
            String::new(),
            "Use `invoke_skill` to load skill content into conversation context.".to_string(),
            String::new(),
        ];

        for skill in &sorted {
            if skill.namespace == "default" {
                lines.push(format!("- **{}**: {}", skill.name, skill.description));
            } else {
                lines.push(format!(
                    "- **{}:{}**: {}",
                    skill.namespace, skill.name, skill.description
                ));
            }
        }

        lines.join("\n")
    }

    /// Get all available skill names.
    ///
    /// Names use namespace prefix for non-default namespaces.
    pub fn get_skill_names(&mut self) -> Vec<String> {
        if self.metadata_cache.is_empty() {
            self.discover_skills();
        }

        self.metadata_cache
            .values()
            .map(|m| {
                if m.namespace == "default" {
                    m.name.clone()
                } else {
                    m.full_name()
                }
            })
            .collect()
    }

    /// Clear all caches. Useful for reloading skills after changes.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.metadata_cache.clear();
    }

    /// Expand variables in a skill's content.
    ///
    /// Replaces `{{variable}}` placeholders with values from the provided map.
    pub fn expand_variables(content: &str, variables: &HashMap<String, String>) -> String {
        let mut result = content.to_string();
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }
        result
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Detect whether a directory is user-global or project-local.
fn detect_source(skill_dir: &Path) -> SkillSource {
    // Check if the path is under the user home directory's .opendev/skills.
    if let Some(home) = dirs::home_dir() {
        let global_skills = home.join(".opendev").join("skills");
        if skill_dir.starts_with(&global_skills) {
            return SkillSource::UserGlobal;
        }
    }
    SkillSource::Project
}

/// Recursively find all `.md` files in a directory.
fn glob_md_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut results = Vec::new();
    collect_md_files(dir, &mut results)?;
    Ok(results)
}

fn collect_md_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_md_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(path);
        }
    }
    Ok(())
}

/// Parse frontmatter from a file on disk.
fn parse_frontmatter_file(path: &Path) -> Option<SkillMetadata> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            debug!(path = %path.display(), error = %e, "failed to read skill file");
            return None;
        }
    };
    let mut meta = parse_frontmatter_str(&content)?;
    if meta.name.is_empty() {
        // Fall back to filename stem.
        meta.name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
    }
    Some(meta)
}

/// Parse YAML frontmatter from a string.
///
/// Expects the format:
/// ```text
/// ---
/// name: foo
/// description: bar
/// namespace: baz
/// ---
/// ```
fn parse_frontmatter_str(content: &str) -> Option<SkillMetadata> {
    let re = Regex::new(r"(?s)^---\n(.*?)\n---").ok()?;
    let caps = re.captures(content)?;
    let frontmatter = caps.get(1)?.as_str();

    // Simple key-value parsing (handles the common case without a full YAML parser).
    let data = parse_simple_yaml(frontmatter);

    let name = data.get("name").cloned().unwrap_or_default();
    let description = data
        .get("description")
        .cloned()
        .unwrap_or_else(|| format!("Skill: {}", if name.is_empty() { "unknown" } else { &name }));
    let namespace = data
        .get("namespace")
        .cloned()
        .unwrap_or_else(|| "default".to_string());

    Some(SkillMetadata {
        name,
        description,
        namespace,
        path: None,
        source: SkillSource::Builtin,
    })
}

/// Simple YAML-like key:value parser for frontmatter.
///
/// Only handles flat `key: value` pairs. Strips surrounding quotes from values.
fn parse_simple_yaml(text: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            let mut value = value.trim().to_string();
            // Strip surrounding quotes.
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                value = value[1..value.len() - 1].to_string();
            }
            result.insert(key, value);
        }
    }
    result
}

/// Strip YAML frontmatter from markdown content, returning the body.
fn strip_frontmatter(content: &str) -> String {
    let re = match Regex::new(r"(?s)^---\n.*?\n---\n*") {
        Ok(r) => r,
        Err(_) => return content.to_string(),
    };
    re.replace(content, "").to_string()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ---- Frontmatter parsing ----

    #[test]
    fn test_parse_frontmatter_basic() {
        let content = "---\nname: commit\ndescription: Git commit skill\n---\n\n# Commit\n";
        let meta = parse_frontmatter_str(content).unwrap();
        assert_eq!(meta.name, "commit");
        assert_eq!(meta.description, "Git commit skill");
        assert_eq!(meta.namespace, "default");
    }

    #[test]
    fn test_parse_frontmatter_with_namespace() {
        let content = "---\nname: rebase\ndescription: Rebase skill\nnamespace: git\n---\n\nBody\n";
        let meta = parse_frontmatter_str(content).unwrap();
        assert_eq!(meta.name, "rebase");
        assert_eq!(meta.namespace, "git");
    }

    #[test]
    fn test_parse_frontmatter_quoted_values() {
        let content = "---\nname: \"my-skill\"\ndescription: 'Use when testing'\n---\n\nBody\n";
        let meta = parse_frontmatter_str(content).unwrap();
        assert_eq!(meta.name, "my-skill");
        assert_eq!(meta.description, "Use when testing");
    }

    #[test]
    fn test_parse_frontmatter_missing_returns_none() {
        let content = "# No frontmatter here\nJust a plain markdown file.\n";
        assert!(parse_frontmatter_str(content).is_none());
    }

    #[test]
    fn test_parse_frontmatter_empty_name_fallback() {
        let content = "---\ndescription: Some skill\n---\n\nBody\n";
        let meta = parse_frontmatter_str(content).unwrap();
        assert!(meta.name.is_empty()); // caller (parse_frontmatter_file) fills in
        assert_eq!(meta.description, "Some skill");
    }

    // ---- Strip frontmatter ----

    #[test]
    fn test_strip_frontmatter() {
        let content = "---\nname: foo\n---\n\n# Title\nBody text.";
        let body = strip_frontmatter(content);
        assert!(body.starts_with("# Title"));
        assert!(!body.contains("---"));
    }

    #[test]
    fn test_strip_frontmatter_no_frontmatter() {
        let content = "# Just markdown\nNo frontmatter.";
        let body = strip_frontmatter(content);
        assert_eq!(body, content);
    }

    // ---- Simple YAML parser ----

    #[test]
    fn test_parse_simple_yaml() {
        let text = "name: commit\ndescription: \"Git commit\"\n# comment\nnamespace: git";
        let data = parse_simple_yaml(text);
        assert_eq!(data.get("name").unwrap(), "commit");
        assert_eq!(data.get("description").unwrap(), "Git commit");
        assert_eq!(data.get("namespace").unwrap(), "git");
    }

    #[test]
    fn test_parse_simple_yaml_single_quotes() {
        let text = "name: 'my-skill'";
        let data = parse_simple_yaml(text);
        assert_eq!(data.get("name").unwrap(), "my-skill");
    }

    // ---- Variable expansion ----

    #[test]
    fn test_expand_variables() {
        let content = "Hello {{user}}, welcome to {{project}}.";
        let mut vars = HashMap::new();
        vars.insert("user".to_string(), "Alice".to_string());
        vars.insert("project".to_string(), "OpenDev".to_string());
        let result = SkillLoader::expand_variables(content, &vars);
        assert_eq!(result, "Hello Alice, welcome to OpenDev.");
    }

    #[test]
    fn test_expand_variables_no_match() {
        let content = "No variables here.";
        let vars = HashMap::new();
        let result = SkillLoader::expand_variables(content, &vars);
        assert_eq!(result, "No variables here.");
    }

    #[test]
    fn test_expand_variables_missing_key_left_intact() {
        let content = "Hello {{user}}, your role is {{role}}.";
        let mut vars = HashMap::new();
        vars.insert("user".to_string(), "Bob".to_string());
        let result = SkillLoader::expand_variables(content, &vars);
        assert_eq!(result, "Hello Bob, your role is {{role}}.");
    }

    // ---- SkillLoader with builtins ----

    #[test]
    fn test_discover_builtin_skills() {
        let mut loader = SkillLoader::new(vec![]);
        let skills = loader.discover_skills();

        // Should find all builtin skills.
        assert!(skills.len() >= 3);

        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"commit"));
        assert!(names.contains(&"review-pr"));
        assert!(names.contains(&"create-pr"));

        // All should be marked as builtin.
        for skill in &skills {
            assert_eq!(skill.source, SkillSource::Builtin);
        }
    }

    #[test]
    fn test_load_builtin_skill() {
        let mut loader = SkillLoader::new(vec![]);
        loader.discover_skills();

        let skill = loader.load_skill("commit").unwrap();
        assert_eq!(skill.metadata.name, "commit");
        assert!(!skill.content.is_empty());
        assert!(skill.content.contains("Git Commit"));
        // Content should NOT contain frontmatter.
        assert!(!skill.content.starts_with("---"));
    }

    #[test]
    fn test_load_nonexistent_skill_returns_none() {
        let mut loader = SkillLoader::new(vec![]);
        loader.discover_skills();
        assert!(loader.load_skill("nonexistent-skill-xyz").is_none());
    }

    // ---- SkillLoader with filesystem skills ----

    #[test]
    fn test_discover_filesystem_skills() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        fs::create_dir_all(&skill_dir).unwrap();

        // Create a flat skill file.
        fs::write(
            skill_dir.join("deploy.md"),
            "---\nname: deploy\ndescription: Deployment skill\n---\n\n# Deploy\nDeploy instructions.\n",
        )
        .unwrap();

        // Create a directory-style skill.
        let nested = skill_dir.join("testing");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            nested.join("SKILL.md"),
            "---\nname: testing\ndescription: Testing patterns\nnamespace: qa\n---\n\n# Testing\n",
        )
        .unwrap();

        let mut loader = SkillLoader::new(vec![skill_dir]);
        let skills = loader.discover_skills();

        let names: Vec<String> = skills.iter().map(|s| s.full_name()).collect();
        assert!(names.contains(&"deploy".to_string()));
        assert!(names.contains(&"qa:testing".to_string()));
    }

    #[test]
    fn test_project_skill_overrides_builtin() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        fs::create_dir_all(&skill_dir).unwrap();

        // Create a project-level "commit" skill that overrides the builtin.
        fs::write(
            skill_dir.join("commit.md"),
            "---\nname: commit\ndescription: Custom commit skill\n---\n\n# Custom Commit\nOverridden.\n",
        )
        .unwrap();

        let mut loader = SkillLoader::new(vec![skill_dir]);
        let skills = loader.discover_skills();

        let commit = skills.iter().find(|s| s.name == "commit").unwrap();
        assert_eq!(commit.description, "Custom commit skill");
        // Should NOT be builtin since the project overrode it.
        assert_ne!(commit.source, SkillSource::Builtin);
    }

    #[test]
    fn test_load_filesystem_skill() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        fs::create_dir_all(&skill_dir).unwrap();

        fs::write(
            skill_dir.join("deploy.md"),
            "---\nname: deploy\ndescription: Deploy skill\n---\n\n# Deploy\nStep 1: Push.\n",
        )
        .unwrap();

        let mut loader = SkillLoader::new(vec![skill_dir]);
        loader.discover_skills();

        let skill = loader.load_skill("deploy").unwrap();
        assert_eq!(skill.metadata.name, "deploy");
        assert!(skill.content.contains("Step 1: Push."));
        assert!(!skill.content.contains("---"));
    }

    #[test]
    fn test_skill_name_fallback_to_filename() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        fs::create_dir_all(&skill_dir).unwrap();

        // Frontmatter without a name field.
        fs::write(
            skill_dir.join("my-cool-skill.md"),
            "---\ndescription: A cool skill\n---\n\nContent here.\n",
        )
        .unwrap();

        let mut loader = SkillLoader::new(vec![skill_dir]);
        let skills = loader.discover_skills();

        let cool = skills.iter().find(|s| s.name == "my-cool-skill");
        assert!(cool.is_some(), "should fall back to filename stem");
    }

    // ---- Skills index ----

    #[test]
    fn test_build_skills_index() {
        let mut loader = SkillLoader::new(vec![]);
        let index = loader.build_skills_index();

        assert!(index.contains("## Available Skills"));
        assert!(index.contains("**commit**"));
        assert!(index.contains("**review-pr**"));
        assert!(index.contains("invoke_skill"));
    }

    #[test]
    fn test_build_skills_index_empty_when_no_skills() {
        // Create a loader with a non-existent dir and no builtins would
        // still have builtins, so this just verifies the format.
        let mut loader = SkillLoader::new(vec![]);
        let index = loader.build_skills_index();
        assert!(!index.is_empty()); // builtins are always present
    }

    // ---- get_skill_names ----

    #[test]
    fn test_get_skill_names() {
        let mut loader = SkillLoader::new(vec![]);
        let names = loader.get_skill_names();
        assert!(names.contains(&"commit".to_string()));
        assert!(names.contains(&"review-pr".to_string()));
    }

    // ---- Cache clearing ----

    #[test]
    fn test_clear_cache() {
        let mut loader = SkillLoader::new(vec![]);
        loader.discover_skills();
        assert!(!loader.metadata_cache.is_empty());

        loader.clear_cache();
        assert!(loader.metadata_cache.is_empty());
        assert!(loader.cache.is_empty());
    }

    // ---- Priority ordering ----

    #[test]
    fn test_first_dir_has_highest_priority() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let dir1 = tmp1.path().join("skills");
        let dir2 = tmp2.path().join("skills");
        fs::create_dir_all(&dir1).unwrap();
        fs::create_dir_all(&dir2).unwrap();

        fs::write(
            dir1.join("myskill.md"),
            "---\nname: myskill\ndescription: From dir1 (high prio)\n---\n\nDir1 content.\n",
        )
        .unwrap();

        fs::write(
            dir2.join("myskill.md"),
            "---\nname: myskill\ndescription: From dir2 (low prio)\n---\n\nDir2 content.\n",
        )
        .unwrap();

        // dir1 first = highest priority.
        let mut loader = SkillLoader::new(vec![dir1, dir2]);
        let skills = loader.discover_skills();

        let myskill = skills.iter().find(|s| s.name == "myskill").unwrap();
        assert_eq!(myskill.description, "From dir1 (high prio)");
    }

    // ---- Namespaced skill lookup ----

    #[test]
    fn test_load_namespaced_skill() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills");
        fs::create_dir_all(&skill_dir).unwrap();

        fs::write(
            skill_dir.join("rebase.md"),
            "---\nname: rebase\ndescription: Git rebase\nnamespace: git\n---\n\n# Rebase\n",
        )
        .unwrap();

        let mut loader = SkillLoader::new(vec![skill_dir]);
        loader.discover_skills();

        // Load by full namespaced name.
        let skill = loader.load_skill("git:rebase").unwrap();
        assert_eq!(skill.metadata.name, "rebase");
        assert_eq!(skill.metadata.namespace, "git");

        // Also loadable by bare name.
        let mut loader2 = SkillLoader::new(vec![tmp.path().join("skills")]);
        loader2.discover_skills();
        let skill2 = loader2.load_skill("rebase").unwrap();
        assert_eq!(skill2.metadata.name, "rebase");
    }
}
