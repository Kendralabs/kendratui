//! PDF tool — extract text from PDF files.
//!
//! This is a simplified implementation that shells out to `pdftotext`
//! (from poppler-utils) since pure-Rust PDF parsing is complex.
//! Falls back to a basic binary scan if pdftotext is unavailable.

use std::collections::HashMap;
use std::path::Path;

use opendev_tools_core::{BaseTool, ToolContext, ToolResult};

/// Maximum PDF file size (50 MB).
const MAX_PDF_SIZE: u64 = 50 * 1024 * 1024;

/// Tool for extracting text from PDF files.
#[derive(Debug)]
pub struct PdfTool;

#[async_trait::async_trait]
impl BaseTool for PdfTool {
    fn name(&self) -> &str {
        "pdf"
    }

    fn description(&self) -> &str {
        "Extract text content from a PDF file. Supports page range selection."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the PDF file"
                },
                "pages": {
                    "type": "string",
                    "description": "Page range (e.g., '1-5', '3', '10-20')"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(
        &self,
        args: HashMap<String, serde_json::Value>,
        ctx: &ToolContext,
    ) -> ToolResult {
        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::fail("file_path is required"),
        };

        let path = if Path::new(file_path).is_absolute() {
            std::path::PathBuf::from(file_path)
        } else {
            ctx.working_dir.join(file_path)
        };

        if !path.exists() {
            return ToolResult::fail(format!("File not found: {}", path.display()));
        }

        // Check file size
        match std::fs::metadata(&path) {
            Ok(m) if m.len() > MAX_PDF_SIZE => {
                return ToolResult::fail(format!(
                    "PDF too large ({} bytes, max {})",
                    m.len(),
                    MAX_PDF_SIZE
                ));
            }
            Err(e) => return ToolResult::fail(format!("Cannot stat file: {e}")),
            _ => {}
        }

        // Check PDF magic bytes
        let header = match std::fs::read(&path) {
            Ok(bytes) if bytes.len() >= 4 => bytes[..4].to_vec(),
            Ok(_) => return ToolResult::fail("File too small to be a PDF"),
            Err(e) => return ToolResult::fail(format!("Cannot read file: {e}")),
        };

        if &header != b"%PDF" {
            return ToolResult::fail("Not a valid PDF file (missing %PDF header)");
        }

        let pages = args.get("pages").and_then(|v| v.as_str());

        // Try pdftotext
        let result = extract_with_pdftotext(&path, pages).await;
        match result {
            Ok(text) => {
                let mut metadata = HashMap::new();
                metadata.insert("method".into(), serde_json::json!("pdftotext"));
                ToolResult::ok_with_metadata(text, metadata)
            }
            Err(_) => {
                // Fall back to basic extraction
                match extract_basic(&path) {
                    Ok(text) => {
                        let mut metadata = HashMap::new();
                        metadata.insert("method".into(), serde_json::json!("basic"));
                        metadata.insert(
                            "warning".into(),
                            serde_json::json!(
                                "pdftotext not available; text extraction may be incomplete"
                            ),
                        );
                        ToolResult::ok_with_metadata(text, metadata)
                    }
                    Err(e) => ToolResult::fail(format!("PDF extraction failed: {e}")),
                }
            }
        }
    }
}

async fn extract_with_pdftotext(path: &Path, pages: Option<&str>) -> Result<String, String> {
    let mut args = Vec::new();

    if let Some(pages) = pages {
        // Parse page range
        if let Some((start, end)) = pages.split_once('-') {
            args.push("-f".to_string());
            args.push(start.to_string());
            args.push("-l".to_string());
            args.push(end.to_string());
        } else {
            // Single page
            args.push("-f".to_string());
            args.push(pages.to_string());
            args.push("-l".to_string());
            args.push(pages.to_string());
        }
    }

    args.push(path.to_string_lossy().to_string());
    args.push("-".to_string()); // output to stdout

    let output = tokio::process::Command::new("pdftotext")
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("pdftotext not found: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// Basic text extraction by scanning for text-like sequences in the PDF.
fn extract_basic(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read file: {e}"))?;

    // Very simple: look for text between BT and ET operators (text objects)
    let content = String::from_utf8_lossy(&bytes);
    let mut text = String::new();

    // Extract text from parenthesized strings (Tj/TJ operators)
    let mut in_paren = false;
    let mut paren_depth = 0;
    let mut current = String::new();

    for ch in content.chars() {
        if ch == '(' && !in_paren {
            in_paren = true;
            paren_depth = 1;
            current.clear();
        } else if in_paren {
            if ch == '(' {
                paren_depth += 1;
                current.push(ch);
            } else if ch == ')' {
                paren_depth -= 1;
                if paren_depth == 0 {
                    in_paren = false;
                    // Filter to printable ASCII
                    let filtered: String = current
                        .chars()
                        .filter(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
                        .collect();
                    if !filtered.trim().is_empty() {
                        text.push_str(&filtered);
                        text.push(' ');
                    }
                } else {
                    current.push(ch);
                }
            } else {
                current.push(ch);
            }
        }
    }

    if text.is_empty() {
        Err("No text content could be extracted from this PDF".to_string())
    } else {
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_pdf_missing_path() {
        let tool = PdfTool;
        let ctx = ToolContext::new("/tmp");
        let result = tool.execute(HashMap::new(), &ctx).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("file_path is required"));
    }

    #[tokio::test]
    async fn test_pdf_file_not_found() {
        let tool = PdfTool;
        let ctx = ToolContext::new("/tmp");
        let args: HashMap<String, serde_json::Value> = [(
            "file_path".to_string(),
            serde_json::json!("/nonexistent/file.pdf"),
        )]
        .into_iter()
        .collect();
        let result = tool.execute(args, &ctx).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_pdf_not_a_pdf() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("fake.pdf");
        std::fs::write(&path, "not a pdf file at all").unwrap();

        let tool = PdfTool;
        let ctx = ToolContext::new(tmp.path());
        let args: HashMap<String, serde_json::Value> = [(
            "file_path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        )]
        .into_iter()
        .collect();
        let result = tool.execute(args, &ctx).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Not a valid PDF"));
    }
}
