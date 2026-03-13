//! Shared utilities for symbol tools.

use std::path::Path;

/// Validate that a string is a valid identifier (letter/underscore start, alphanumeric/underscore rest).
pub fn is_valid_identifier(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Detect file language category from extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangCategory {
    Python,
    CLike,
    Other,
}

#[allow(dead_code)]
pub fn detect_lang(path: &Path) -> LangCategory {
    match path.extension().and_then(|e| e.to_str()) {
        Some("py" | "pyi" | "pyw") => LangCategory::Python,
        Some(
            "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "java" | "js" | "ts" | "tsx" | "jsx" | "go"
            | "rs" | "cs" | "swift" | "kt" | "scala" | "m" | "mm",
        ) => LangCategory::CLike,
        _ => LangCategory::Other,
    }
}

/// Truncate a string to max chars, appending "..." if truncated.
pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

/// Make a path relative to a base, falling back to absolute.
pub fn relative_display(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_identifier() {
        assert!(is_valid_identifier("foo"));
        assert!(is_valid_identifier("_bar"));
        assert!(is_valid_identifier("Baz123"));
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("123"));
        assert!(!is_valid_identifier("foo-bar"));
        assert!(!is_valid_identifier("foo bar"));
    }

    #[test]
    fn test_detect_lang() {
        use std::path::PathBuf;
        assert_eq!(detect_lang(&PathBuf::from("a.py")), LangCategory::Python);
        assert_eq!(detect_lang(&PathBuf::from("a.rs")), LangCategory::CLike);
        assert_eq!(detect_lang(&PathBuf::from("a.txt")), LangCategory::Other);
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello...");
    }

    #[test]
    fn test_relative_display() {
        let base = std::path::PathBuf::from("/workspace");
        let path = std::path::PathBuf::from("/workspace/src/main.rs");
        assert_eq!(relative_display(&path, &base), "src/main.rs");

        let other = std::path::PathBuf::from("/other/file.rs");
        assert_eq!(relative_display(&other, &base), "/other/file.rs");
    }
}
