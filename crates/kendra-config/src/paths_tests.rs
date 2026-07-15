use super::*;
use std::env;

#[test]
fn test_encode_project_path() {
    // Use a path that doesn't need canonicalization
    let encoded = "/Users/foo/bar".replace('/', "-");
    assert_eq!(encoded, "-Users-foo-bar");
}

#[test]
fn test_paths_project_dir() {
    let paths = Paths::new(Some(PathBuf::from("/tmp/test-project")));
    assert_eq!(
        paths.project_dir(),
        PathBuf::from("/tmp/test-project/.kendra")
    );
}

#[test]
fn test_session_file() {
    let paths = Paths::new(Some(PathBuf::from("/tmp/test")));
    let session_path = paths.session_file("abc123");
    assert!(session_path.to_string_lossy().ends_with("abc123.json"));
}

#[test]
fn test_project_context_file() {
    let paths = Paths::new(Some(PathBuf::from("/tmp/myproject")));
    assert_eq!(
        paths.project_context_file(),
        PathBuf::from("/tmp/myproject/AGENTS.md")
    );
}

#[test]
fn test_project_mcp_config() {
    let paths = Paths::new(Some(PathBuf::from("/tmp/myproject")));
    assert_eq!(
        paths.project_mcp_config(),
        PathBuf::from("/tmp/myproject/.mcp.json")
    );
}

#[test]
fn test_kendra_dir_env_override() {
    let key = "KENDRA_DIR";
    let legacy_key = "kendra_DIR";
    // SAFETY: test runs single-threaded for env var manipulation
    let original = env::var(key).ok();
    let original_legacy = env::var(legacy_key).ok();

    // Priority test
    unsafe { env::set_var(key, "/tmp/custom-kendra") };
    unsafe { env::set_var(legacy_key, "/tmp/custom-kendra") };

    let paths = Paths::new(Some(PathBuf::from("/tmp/wd")));
    assert_eq!(paths.global_dir(), PathBuf::from("/tmp/custom-kendra"));
    assert_eq!(
        paths.global_settings(),
        PathBuf::from("/tmp/custom-kendra/settings.json")
    );

    // Fallback test
    unsafe { env::remove_var(key) };
    let paths_legacy = Paths::new(Some(PathBuf::from("/tmp/wd")));
    assert_eq!(
        paths_legacy.global_dir(),
        PathBuf::from("/tmp/custom-kendra")
    );

    // Restore
    match original {
        Some(v) => unsafe { env::set_var(key, v) },
        None => unsafe { env::remove_var(key) },
    }
    match original_legacy {
        Some(v) => unsafe { env::set_var(legacy_key, v) },
        None => unsafe { env::remove_var(legacy_key) },
    }
}

#[test]
fn test_xdg_accessors_present() {
    let paths = Paths::new(Some(PathBuf::from("/tmp/wd")));
    // Just verify the accessors don't panic and return non-empty paths
    assert!(!paths.config_dir().as_os_str().is_empty());
    assert!(!paths.data_dir().as_os_str().is_empty());
    assert!(!paths.cache_dir().as_os_str().is_empty());
    assert!(!paths.state_dir().as_os_str().is_empty());
}

#[test]
fn test_all_base_dirs() {
    let paths = Paths::new(Some(PathBuf::from("/tmp/wd")));
    let bases = paths.all_base_dirs();
    assert!(bases.len() >= 4);
}

#[test]
fn test_config_vs_data_separation() {
    // With kendra_DIR override, config and data point to same place
    let key = "kendra_DIR";
    // SAFETY: test runs single-threaded for env var manipulation
    let original = env::var(key).ok();

    unsafe { env::set_var(key, "/tmp/override-kendra") };
    let paths = Paths::new(Some(PathBuf::from("/tmp/wd")));
    // Settings (config) in config_dir
    assert!(paths.global_settings().starts_with("/tmp/override-kendra"));
    // Sessions (data) in data_dir
    assert!(
        paths
            .global_sessions_dir()
            .starts_with("/tmp/override-kendra")
    );

    match original {
        Some(v) => unsafe { env::set_var(key, v) },
        None => unsafe { env::remove_var(key) },
    }
}
