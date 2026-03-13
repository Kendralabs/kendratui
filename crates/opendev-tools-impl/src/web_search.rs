//! Web search tool — search the web via DuckDuckGo HTML scraping.
//!
//! Uses DuckDuckGo's HTML interface for privacy-respecting web searches
//! without requiring API keys. Results are parsed from the HTML response.

use std::collections::HashMap;

use opendev_tools_core::{BaseTool, ToolContext, ToolResult};

/// Default number of search results to return.
const DEFAULT_MAX_RESULTS: usize = 10;

/// Maximum body size to read from DuckDuckGo (256 KB).
const MAX_BODY_SIZE: usize = 256 * 1024;

/// Tool for searching the web using DuckDuckGo.
#[derive(Debug)]
pub struct WebSearchTool;

/// A single search result.
#[derive(Debug, Clone, serde::Serialize)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

#[async_trait::async_trait]
impl BaseTool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web using DuckDuckGo. Returns titles, URLs, and snippets."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query string"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 10)"
                },
                "allowed_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only include results from these domains"
                },
                "blocked_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Exclude results from these domains"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        args: HashMap<String, serde_json::Value>,
        _ctx: &ToolContext,
    ) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.trim().is_empty() => q.trim(),
            _ => return ToolResult::fail("Search query is required"),
        };

        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_MAX_RESULTS);

        let allowed_domains: Vec<String> = args
            .get("allowed_domains")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                    .collect()
            })
            .unwrap_or_default();

        let blocked_domains: Vec<String> = args
            .get("blocked_domains")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                    .collect()
            })
            .unwrap_or_default();

        // Build DuckDuckGo HTML search URL
        let encoded_query = urlencoded(query);
        let url = format!("https://html.duckduckgo.com/html/?q={encoded_query}");

        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                 AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .build()
        {
            Ok(c) => c,
            Err(e) => return ToolResult::fail(format!("Failed to create HTTP client: {e}")),
        };

        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => return ToolResult::fail(format!("Search request failed: {e}")),
        };

        if !response.status().is_success() {
            return ToolResult::fail(format!("DuckDuckGo returned HTTP {}", response.status()));
        }

        let body = match response.text().await {
            Ok(t) => {
                if t.len() > MAX_BODY_SIZE {
                    t[..MAX_BODY_SIZE].to_string()
                } else {
                    t
                }
            }
            Err(e) => return ToolResult::fail(format!("Failed to read response: {e}")),
        };

        // Parse results from HTML
        let mut results = parse_ddg_html(&body);

        // Filter by domain
        if !allowed_domains.is_empty() || !blocked_domains.is_empty() {
            results = filter_by_domain(results, &allowed_domains, &blocked_domains);
        }

        // Limit results
        results.truncate(max_results);

        let result_count = results.len();

        // Format output
        let mut output_parts = Vec::new();
        output_parts.push(format!(
            "Search results for \"{query}\" ({result_count} results):\n"
        ));

        for (i, result) in results.iter().enumerate() {
            output_parts.push(format!(
                "{}. {}\n   {}\n   {}\n",
                i + 1,
                result.title,
                result.url,
                result.snippet
            ));
        }

        if results.is_empty() {
            output_parts.push("No results found.".to_string());
        }

        let output = output_parts.join("");

        let mut metadata = HashMap::new();
        metadata.insert("query".into(), serde_json::json!(query));
        metadata.insert("result_count".into(), serde_json::json!(result_count));
        metadata.insert(
            "results".into(),
            serde_json::to_value(&results).unwrap_or_default(),
        );

        ToolResult::ok_with_metadata(output, metadata)
    }
}

/// URL-encode a string for query parameters.
fn urlencoded(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
            ' ' => result.push('+'),
            _ => {
                let mut buf = [0u8; 4];
                let encoded = c.encode_utf8(&mut buf);
                for b in encoded.bytes() {
                    result.push('%');
                    result.push_str(&format!("{b:02X}"));
                }
            }
        }
    }
    result
}

/// Extract the domain from a URL, stripping the `www.` prefix.
fn extract_domain(url: &str) -> Option<String> {
    // Simple domain extraction without pulling in the `url` crate.
    let after_scheme = if let Some(rest) = url.strip_prefix("https://") {
        rest
    } else if let Some(rest) = url.strip_prefix("http://") {
        rest
    } else {
        return None;
    };
    let domain = after_scheme.split('/').next().unwrap_or("");
    let domain = domain.split(':').next().unwrap_or(domain); // strip port
    let domain = domain.to_lowercase();
    let domain = domain.strip_prefix("www.").unwrap_or(&domain).to_string();
    if domain.is_empty() {
        None
    } else {
        Some(domain)
    }
}

/// Filter results by allowed/blocked domain lists.
fn filter_by_domain(
    results: Vec<SearchResult>,
    allowed: &[String],
    blocked: &[String],
) -> Vec<SearchResult> {
    results
        .into_iter()
        .filter(|r| {
            let domain = match extract_domain(&r.url) {
                Some(d) => d,
                None => return false,
            };

            // Check allowed
            if !allowed.is_empty() {
                let passes = allowed.iter().any(|a| {
                    let clean = a.strip_prefix("www.").unwrap_or(a);
                    domain == clean || domain.ends_with(&format!(".{clean}"))
                });
                if !passes {
                    return false;
                }
            }

            // Check blocked
            if !blocked.is_empty() {
                let is_blocked = blocked.iter().any(|b| {
                    let clean = b.strip_prefix("www.").unwrap_or(b);
                    domain == clean || domain.ends_with(&format!(".{clean}"))
                });
                if is_blocked {
                    return false;
                }
            }

            true
        })
        .collect()
}

/// Parse DuckDuckGo HTML search results.
///
/// DuckDuckGo's HTML-only endpoint returns results inside
/// `<div class="result ...">` blocks. Each block contains:
/// - `<a class="result__a" href="...">title</a>`
/// - `<a class="result__snippet" ...>snippet</a>`
fn parse_ddg_html(html: &str) -> Vec<SearchResult> {
    let mut results = Vec::new();

    // Split by result blocks
    let parts: Vec<&str> = html.split("class=\"result__a\"").collect();

    for part in parts.iter().skip(1) {
        // Extract URL from href="..."
        let url = extract_attr(part, "href=\"")
            .map(|u| {
                // DuckDuckGo wraps URLs in redirect links
                if let Some(actual) = extract_redirect_url(u) {
                    actual
                } else {
                    u.to_string()
                }
            })
            .unwrap_or_default();

        // Extract title (text between > and </a>)
        let title = extract_tag_text(part).unwrap_or_default();

        // Extract snippet
        let snippet = if let Some(snippet_start) = part.find("result__snippet") {
            let snippet_part = &part[snippet_start..];
            extract_tag_text(snippet_part).unwrap_or_default()
        } else {
            String::new()
        };

        if !url.is_empty() && !title.is_empty() {
            results.push(SearchResult {
                title: strip_html_tags(&title),
                url,
                snippet: strip_html_tags(&snippet),
            });
        }
    }

    results
}

/// Extract an attribute value after the given prefix.
fn extract_attr<'a>(html: &'a str, prefix: &str) -> Option<&'a str> {
    let start = html.find(prefix)?;
    let rest = &html[start + prefix.len()..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Extract text content after the first `>` until `</`.
fn extract_tag_text(html: &str) -> Option<String> {
    let start = html.find('>')? + 1;
    let rest = &html[start..];
    let end = rest.find("</").unwrap_or(rest.len().min(500));
    Some(html_decode(&rest[..end]).trim().to_string())
}

/// Extract the actual URL from DuckDuckGo's redirect URL.
///
/// DDG redirects look like: `//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com&rut=...`
fn extract_redirect_url(url: &str) -> Option<String> {
    if url.contains("duckduckgo.com/l/") || url.contains("uddg=") {
        // Find uddg= parameter
        let uddg_start = url.find("uddg=")?;
        let rest = &url[uddg_start + 5..];
        let end = rest.find('&').unwrap_or(rest.len());
        let encoded = &rest[..end];
        Some(urldecode(encoded))
    } else if url.starts_with("//") {
        Some(format!("https:{url}"))
    } else {
        None
    }
}

/// Decode percent-encoded URL strings.
fn urldecode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().unwrap_or(b'0');
            let lo = chars.next().unwrap_or(b'0');
            let val = hex_val(hi) * 16 + hex_val(lo);
            result.push(val as char);
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

fn hex_val(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

/// Decode common HTML entities.
fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
}

/// Strip HTML tags from a string, keeping only text content.
fn strip_html_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
    }
    // Collapse whitespace
    let collapsed: String = result.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_args(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn test_urlencoded() {
        assert_eq!(urlencoded("hello world"), "hello+world");
        assert_eq!(urlencoded("rust+lang"), "rust%2Blang");
        assert_eq!(urlencoded("test"), "test");
    }

    #[test]
    fn test_extract_domain() {
        assert_eq!(
            extract_domain("https://www.example.com/page"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_domain("https://docs.rust-lang.org/book/"),
            Some("docs.rust-lang.org".to_string())
        );
        assert_eq!(
            extract_domain("http://localhost:8080/test"),
            Some("localhost".to_string())
        );
        assert_eq!(extract_domain("ftp://files.example.com"), None);
    }

    #[test]
    fn test_filter_by_domain() {
        let results = vec![
            SearchResult {
                title: "Rust".into(),
                url: "https://www.rust-lang.org".into(),
                snippet: "A language".into(),
            },
            SearchResult {
                title: "Go".into(),
                url: "https://golang.org".into(),
                snippet: "Another language".into(),
            },
            SearchResult {
                title: "Docs".into(),
                url: "https://docs.rust-lang.org".into(),
                snippet: "Rust docs".into(),
            },
        ];

        // Allowed filter
        let filtered = filter_by_domain(results.clone(), &["rust-lang.org".to_string()], &[]);
        assert_eq!(filtered.len(), 2); // rust-lang.org and docs.rust-lang.org

        // Blocked filter
        let filtered = filter_by_domain(results.clone(), &[], &["golang.org".to_string()]);
        assert_eq!(filtered.len(), 2); // everything except golang.org
    }

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<b>bold</b> text"), "bold text");
        assert_eq!(strip_html_tags("no tags here"), "no tags here");
        assert_eq!(
            strip_html_tags("<a href=\"x\">link</a> and <em>emphasis</em>"),
            "link and emphasis"
        );
    }

    #[test]
    fn test_html_decode() {
        assert_eq!(html_decode("&amp;"), "&");
        assert_eq!(html_decode("&lt;div&gt;"), "<div>");
        assert_eq!(html_decode("it&#39;s"), "it's");
    }

    #[test]
    fn test_urldecode() {
        assert_eq!(urldecode("hello%20world"), "hello world");
        assert_eq!(
            urldecode("https%3A%2F%2Fexample.com"),
            "https://example.com"
        );
    }

    #[test]
    fn test_extract_redirect_url() {
        let url = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fpage&rut=abc";
        assert_eq!(
            extract_redirect_url(url),
            Some("https://example.com/page".to_string())
        );
    }

    #[tokio::test]
    async fn test_web_search_missing_query() {
        let tool = WebSearchTool;
        let ctx = ToolContext::new("/tmp");
        let result = tool.execute(HashMap::new(), &ctx).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("query is required"));
    }

    #[tokio::test]
    async fn test_web_search_empty_query() {
        let tool = WebSearchTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[("query", serde_json::json!("  "))]);
        let result = tool.execute(args, &ctx).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("query is required"));
    }

    #[test]
    fn test_parse_ddg_html_basic() {
        let html = r#"
        <div class="result results_links results_links_deep web-result">
            <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org&rut=abc">Rust Programming Language</a>
            <a class="result__snippet">A systems programming language focused on safety.</a>
        </div>
        "#;

        let results = parse_ddg_html(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming Language");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert!(results[0].snippet.contains("systems programming"));
    }
}
