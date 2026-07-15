use super::*;
use std::path::PathBuf;

#[test]
fn test_ruleset_empty_returns_none() {
    let rs = PermissionRuleSet::new();
    assert_eq!(rs.evaluate_simple("bash", "ls"), None);
}

#[test]
fn test_ruleset_basic_allow() {
    let mut rs = PermissionRuleSet::new();
    rs.add_rule(PermissionRule {
        pattern: "bash:*".into(),
        action: PermissionAction::Allow,
        priority: 10,
        directory_scope: None,
    });
    assert_eq!(
        rs.evaluate_simple("bash", "ls -la"),
        Some(PermissionAction::Allow)
    );
    assert_eq!(rs.evaluate_simple("edit", "foo.rs"), None);
}

#[test]
fn test_ruleset_priority_ordering() {
    let mut rs = PermissionRuleSet::new();
    rs.add_rule(PermissionRule {
        pattern: "bash:*".into(),
        action: PermissionAction::Allow,
        priority: 1,
        directory_scope: None,
    });
    rs.add_rule(PermissionRule {
        pattern: "bash:rm *".into(),
        action: PermissionAction::Deny,
        priority: 10,
        directory_scope: None,
    });
    // "rm -rf /" matches the Deny rule with higher priority
    assert_eq!(
        rs.evaluate_simple("bash", "rm -rf /"),
        Some(PermissionAction::Deny)
    );
    // "ls" only matches the Allow rule
    assert_eq!(
        rs.evaluate_simple("bash", "ls"),
        Some(PermissionAction::Allow)
    );
}

#[test]
fn test_ruleset_directory_scope() {
    let mut rs = PermissionRuleSet::new();
    rs.add_rule(PermissionRule {
        pattern: "edit:*".into(),
        action: PermissionAction::Allow,
        priority: 10,
        directory_scope: Some("src/**".into()),
    });
    rs.add_rule(PermissionRule {
        pattern: "edit:*".into(),
        action: PermissionAction::Deny,
        priority: 10,
        directory_scope: Some("vendor/**".into()),
    });

    let src_dir = PathBuf::from("src/components/button.rs");
    let vendor_dir = PathBuf::from("vendor/lib/foo.rs");

    assert_eq!(
        rs.evaluate("edit", "foo.rs", Some(&src_dir)),
        Some(PermissionAction::Allow)
    );
    assert_eq!(
        rs.evaluate("edit", "foo.rs", Some(&vendor_dir)),
        Some(PermissionAction::Deny)
    );
    // No directory => scoped rules don't match
    assert_eq!(rs.evaluate("edit", "foo.rs", None), None);
}

#[test]
fn test_ruleset_scoped_and_unscoped_mix() {
    let mut rs = PermissionRuleSet::new();
    // Low-priority blanket allow
    rs.add_rule(PermissionRule {
        pattern: "edit:*".into(),
        action: PermissionAction::Allow,
        priority: 1,
        directory_scope: None,
    });
    // High-priority deny for vendor
    rs.add_rule(PermissionRule {
        pattern: "edit:*".into(),
        action: PermissionAction::Deny,
        priority: 100,
        directory_scope: Some("vendor/**".into()),
    });

    let vendor = PathBuf::from("vendor/lib.rs");
    let src = PathBuf::from("src/main.rs");

    // vendor => Deny wins
    assert_eq!(
        rs.evaluate("edit", "x", Some(&vendor)),
        Some(PermissionAction::Deny)
    );
    // src => scoped Deny doesn't match, blanket Allow applies
    assert_eq!(
        rs.evaluate("edit", "x", Some(&src)),
        Some(PermissionAction::Allow)
    );
}

#[test]
fn test_ruleset_prompt_action() {
    let mut rs = PermissionRuleSet::new();
    rs.add_rule(PermissionRule {
        pattern: "bash:sudo *".into(),
        action: PermissionAction::Prompt,
        priority: 50,
        directory_scope: None,
    });
    assert_eq!(
        rs.evaluate_simple("bash", "sudo rm -rf /"),
        Some(PermissionAction::Prompt)
    );
}

#[test]
fn test_is_sensitive_file() {
    // .env files
    assert!(is_sensitive_file(".env"));
    assert!(is_sensitive_file("/path/to/.env"));
    assert!(is_sensitive_file(".env.local"));
    assert!(is_sensitive_file(".env.production"));
    assert!(is_sensitive_file("/app/.env.staging"));

    // Allowed .env variants
    assert!(!is_sensitive_file(".env.example"));
    assert!(!is_sensitive_file(".env.sample"));
    assert!(!is_sensitive_file(".env.template"));

    // Other credential files
    assert!(is_sensitive_file("credentials.json"));
    assert!(is_sensitive_file("id_rsa"));
    assert!(is_sensitive_file("id_ed25519"));
    assert!(is_sensitive_file(".npmrc"));
    assert!(is_sensitive_file(".pypirc"));

    // Non-sensitive files
    assert!(!is_sensitive_file("main.rs"));
    assert!(!is_sensitive_file("Cargo.toml"));
    assert!(!is_sensitive_file("README.md"));
    assert!(!is_sensitive_file(".envrc")); // not .env
}

#[test]
fn test_defaults_deny_env_files() {
    let rs = PermissionRuleSet::with_defaults();

    // .env denied
    assert_eq!(
        rs.evaluate_simple("read_file", "/app/.env"),
        Some(PermissionAction::Deny)
    );
    assert_eq!(
        rs.evaluate_simple("read_file", "/app/.env.local"),
        Some(PermissionAction::Deny)
    );
    assert_eq!(
        rs.evaluate_simple("edit_file", ".env"),
        Some(PermissionAction::Deny)
    );
    assert_eq!(
        rs.evaluate_simple("write_file", "/app/.env.production"),
        Some(PermissionAction::Deny)
    );

    // .env.example allowed
    assert_eq!(
        rs.evaluate_simple("read_file", "/app/.env.example"),
        Some(PermissionAction::Allow)
    );
    assert_eq!(
        rs.evaluate_simple("read_file", ".env.sample"),
        Some(PermissionAction::Allow)
    );

    // Normal files unaffected
    assert_eq!(rs.evaluate_simple("read_file", "main.rs"), None);
}

#[test]
fn test_ruleset_remove_rules() {
    let mut rs = PermissionRuleSet::new();
    rs.add_rule(PermissionRule {
        pattern: "bash:*".into(),
        action: PermissionAction::Allow,
        priority: 1,
        directory_scope: None,
    });
    rs.add_rule(PermissionRule {
        pattern: "edit:*".into(),
        action: PermissionAction::Deny,
        priority: 1,
        directory_scope: None,
    });
    assert_eq!(rs.rules().len(), 2);
    rs.remove_rules(|r| r.action == PermissionAction::Deny);
    assert_eq!(rs.rules().len(), 1);
    assert_eq!(rs.rules()[0].pattern, "bash:*");
}

#[test]
fn test_ruleset_load_from_json() {
    let temp_dir = tempfile::tempdir().unwrap();
    let policy_path = temp_dir.path().join("kendra.policy.json");
    let content = r#"[
        {
            "pattern": "bash:*",
            "action": "allow",
            "priority": 10
        },
        {
            "pattern": "edit:*",
            "action": "deny",
            "priority": 20,
            "directory_scope": "vendor/**"
        }
    ]"#;
    std::fs::write(&policy_path, content).unwrap();

    let rs = PermissionRuleSet::load_from_json(&policy_path).unwrap();
    assert_eq!(rs.rules().len(), 2);
    assert_eq!(
        rs.evaluate_simple("bash", "ls"),
        Some(PermissionAction::Allow)
    );

    let vendor = PathBuf::from("vendor/lib.rs");
    assert_eq!(
        rs.evaluate("edit", "x", Some(&vendor)),
        Some(PermissionAction::Deny)
    );
}

#[test]
fn test_permission_audit_logger() {
    let temp_dir = tempfile::tempdir().unwrap();
    let log_path = temp_dir.path().join("audit.ndjson");
    let logger = PermissionAuditLogger::new(log_path.clone());

    let mut rs = PermissionRuleSet::new();
    rs.add_rule(PermissionRule {
        pattern: "bash:*".into(),
        action: PermissionAction::Allow,
        priority: 1,
        directory_scope: None,
    });
    rs.set_audit_logger(logger);

    // Evaluate something to trigger logging
    rs.evaluate_simple("bash", "ls");

    // Check log content
    let log_content = std::fs::read_to_string(&log_path).unwrap();
    assert!(log_content.contains("\"tool_name\":\"bash\""));
    assert!(log_content.contains("\"action\":\"allow\""));
    assert!(log_content.contains("\"args\":\"ls\""));
}
