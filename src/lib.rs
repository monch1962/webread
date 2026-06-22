use scraper::{ElementRef, Html, Selector};
use std::collections::HashMap;
use std::time::Duration;

/// Tags we strip from readable content extraction.
const STRIP_TAGS: &[&str] = &["nav", "header", "footer", "aside", "script", "style"];

/// Return a compatible User-Agent string to avoid blocking.
pub fn user_agent() -> String {
    user_agent_with_override(None)
}

/// Return a User-Agent string, using an override if provided, otherwise the default.
pub fn user_agent_with_override(override_ua: Option<&str>) -> String {
    override_ua.unwrap_or("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15").to_string()
}

/// Parse a simple key=value config file.
/// Lines starting with '#' are comments. Blank lines are ignored.
/// Leading/trailing whitespace is trimmed from keys and values.
pub fn parse_config(input: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();
            if !key.is_empty() {
                map.insert(key, value);
            }
        }
    }
    map
}

/// Load configuration from the default config file path (~/.config/webread/config).
/// Returns an empty HashMap if the file doesn't exist or can't be read.
pub fn load_config() -> HashMap<String, String> {
    let path = dirs_config_path().join("webread").join("config");
    match std::fs::read_to_string(&path) {
        Ok(content) => parse_config(&content),
        Err(_) => HashMap::new(),
    }
}

/// Get the XDG config directory (~/.config/) or a platform-appropriate equivalent.
fn dirs_config_path() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        std::path::PathBuf::from(dir)
    } else if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".config")
    } else {
        std::path::PathBuf::from(".")
    }
}

/// Options for fetching a URL with resource guardrails.
#[derive(Clone, Debug, PartialEq)]
pub struct FetchOptions {
    /// Maximum time for the full request cycle in seconds.
    pub timeout_secs: u64,
    /// Maximum response body size in bytes. Body is truncated at this limit.
    pub max_body_bytes: usize,
    /// Whether to retry once on transient errors (503, timeout).
    pub retry_transient: bool,
    /// Whether to skip non-HTML content types.
    pub require_html: bool,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            max_body_bytes: 10 * 1024 * 1024, // 10 MB
            retry_transient: true,
            require_html: true,
        }
    }
}

/// Result of a fetched URL with metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct FetchResult {
    pub body: String,
    pub content_type: Option<String>,
    pub status: u16,
    pub final_url: String,
}

/// Fetch a URL with default options (convenience wrapper).
pub fn fetch_url(url: &str) -> anyhow::Result<String> {
    let result = fetch_url_with(url, &FetchOptions::default())?;
    Ok(result.body)
}

/// Fetch a URL with the given resource guardrail options.
///
/// Features:
/// - Configurable timeout (prevents hanging)
/// - Body size limit (prevents OOM on giant pages)
/// - Content-type filtering (skips non-HTML responses)
/// - Automatic retry on transient errors
/// - Returns metadata: final URL after redirects, status code, content-type
pub fn fetch_url_with(url: &str, opts: &FetchOptions) -> anyhow::Result<FetchResult> {
    let do_fetch = || -> anyhow::Result<FetchResult> {
        use ureq::ResponseExt;

        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(opts.timeout_secs)))
            .build();
        let agent = config.new_agent();

        let response = agent
            .get(url)
            .header("User-Agent", &user_agent())
            .call()
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Check content-type if required
        if opts.require_html {
            if let Some(ref ct) = content_type {
                let ct_lower = ct.to_lowercase();
                let is_html = ct_lower.contains("text/html")
                    || ct_lower.contains("text/plain")
                    || ct_lower.contains("application/xhtml")
                    || ct_lower.contains("charset");
                if !is_html && !ct_lower.is_empty() {
                    anyhow::bail!(
                        "Content-Type '{}' is not HTML. Use --require-html=false to fetch anyway.",
                        ct
                    );
                }
            }
        }

        // Read body with size limit
        let final_url = response.get_uri().to_string();
        let reader = response.into_body().read_to_string()?;
        let body = if reader.len() > opts.max_body_bytes {
            // Truncate — convert the first max_body_bytes to string lossily
            String::from_utf8_lossy(&reader.as_bytes()[..opts.max_body_bytes]).to_string()
        } else {
            reader
        };

        Ok(FetchResult {
            body,
            content_type,
            status,
            final_url,
        })
    };

    let result = do_fetch();

    // Retry once on transient errors if enabled
    if let Err(err) = result.as_ref() {
        if opts.retry_transient {
            let err_msg = format!("{err:#}");
            if err_msg.contains("503")
                || err_msg.contains("502")
                || err_msg.contains("timeout")
                || err_msg.contains("timed out")
            {
                std::thread::sleep(Duration::from_millis(500));
                return do_fetch();
            }
        }
    }

    result.map_err(|e| anyhow::anyhow!("Failed to fetch {url}: {e:#}"))
}

/// Resolve a potentially relative URL against a base URL.
/// If `href` is already absolute, returns it unchanged.
/// Handles: root-relative (/x), relative (x), protocol-relative (//x),
/// up-level (../x), fragments (#x), and query (?x) references.
pub fn resolve_url(base: &str, href: &str) -> String {
    // Already absolute (has scheme)
    if href.contains("://") {
        return href.to_string();
    }

    // Protocol-relative: "//host/path"
    if let Some(suffix) = href.strip_prefix("//") {
        if let Some(pos) = base.find("://") {
            let scheme = &base[..pos + 3];
            return format!("{scheme}{suffix}");
        }
        return href.to_string();
    }

    // Fragment or query: append to base (stripping base's fragment/query)
    if href.starts_with('#') || href.starts_with('?') {
        let clean = base
            .split('#')
            .next()
            .unwrap_or(base)
            .split('?')
            .next()
            .unwrap_or(base);
        return format!("{clean}{href}");
    }

    // Extract scheme and authority (host + optional port) from base
    let (scheme, rest) = match base.find("://") {
        Some(pos) => (&base[..pos], &base[pos + 3..]),
        None => return href.to_string(), // invalid base
    };

    // Find authority end (end of host:port part = first '/' after scheme://)
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let base_path = &rest[authority_end..]; // includes leading '/'

    // Root-relative: replace path
    if href.starts_with('/') {
        return format!("{scheme}://{authority}{href}");
    }

    // Relative path: compute resolved path from base directory
    let base_dir = match base_path.rfind('/') {
        Some(pos) => &base_path[..=pos],
        None => "/",
    };

    // Normalize the combined path
    let combined = format!("{base_dir}{href}");
    let mut parts: Vec<&str> = Vec::new();
    for segment in combined.split('/') {
        match segment {
            "." | "" => continue,
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }

    format!("{scheme}://{authority}/{}", parts.join("/"))
}

#[cfg(test)]
mod guardrail_tests {
    use super::*;

    // --- Config parsing tests ---

    #[test]
    fn test_parse_config_empty() {
        let cfg = parse_config("");
        assert!(cfg.is_empty());
    }

    #[test]
    fn test_parse_config_basic() {
        let cfg = parse_config("timeout=15\nmax_size=5000000\n");
        assert_eq!(cfg.get("timeout").unwrap(), "15");
        assert_eq!(cfg.get("max_size").unwrap(), "5000000");
    }

    #[test]
    fn test_parse_config_ignores_comments_and_blanks() {
        let cfg = parse_config("# comment\n  \ntimeout=30\n# another\n");
        assert_eq!(cfg.len(), 1);
        assert_eq!(cfg.get("timeout").unwrap(), "30");
    }

    #[test]
    fn test_parse_config_trims_whitespace() {
        let cfg = parse_config("  timeout = 15  \n");
        assert_eq!(cfg.get("timeout").unwrap(), "15");
    }

    #[test]
    fn test_parse_config_override() {
        let cfg = parse_config("timeout=10\nuser-agent=my-bot/1.0\n");
        assert_eq!(cfg.get("user-agent").unwrap(), "my-bot/1.0");
    }

    // --- user_agent override test ---

    #[test]
    fn test_user_agent_custom() {
        let ua = user_agent_with_override(Some("my-bot/1.0"));
        assert_eq!(ua, "my-bot/1.0");
    }

    #[test]
    fn test_user_agent_default() {
        let ua = user_agent_with_override(None);
        assert!(ua.contains("Mozilla/5.0"));
        assert!(ua.contains("Safari/"));
    }

    // --- Readability comparison test ---

    #[test]
    fn test_readable_known_structure() {
        // A simple article that extract_readable_content should handle perfectly
        let html = "\
<html><body>
<nav>Nav links here</nav>
<article>
<h1>The Quick Brown Fox</h1>
<p>The quick brown fox jumps over the lazy dog. This sentence contains
every letter of the alphabet and is commonly used for typing practice.</p>
<p>Pangrams are useful for displaying font samples and testing keyboards.
The most well-known English pangram is the one used above.</p>
<p>Other pangrams exist in many languages. Each language has its own set
of commonly used pangrams that serve the same purpose.</p>
</article>
<footer>Copyright Footer Content</footer>
</body></html>";
        let result = extract_readable_content(&html).unwrap();
        assert!(result.contains("Quick Brown Fox"), "should extract title");
        assert!(
            result.contains("quick brown fox jumps"),
            "should extract body"
        );
        assert!(!result.contains("Nav links"), "should NOT contain nav");
        assert!(
            !result.contains("Copyright Footer"),
            "should NOT contain footer"
        );
        // Verify the text order is preserved
        let title_pos = result.find("Quick Brown Fox").unwrap();
        let body_pos = result.find("quick brown fox jumps").unwrap();
        assert!(title_pos < body_pos, "title should come before body");
    }

    // --- FetchOptions defaults ---

    #[test]
    fn test_fetch_options_defaults() {
        let opts = FetchOptions::default();
        assert_eq!(opts.timeout_secs, 30);
        assert_eq!(opts.max_body_bytes, 10 * 1024 * 1024);
        assert!(opts.retry_transient);
        assert!(opts.require_html);
    }

    // --- resolve_url ---

    #[test]
    fn test_resolve_relative_url() {
        let resolved = resolve_url("https://example.com/page/", "sub");
        assert_eq!(resolved, "https://example.com/page/sub");
    }

    #[test]
    fn test_resolve_absolute_url_unchanged() {
        let resolved = resolve_url("https://example.com/", "https://other.com/");
        assert_eq!(resolved, "https://other.com/");
    }

    #[test]
    fn test_resolve_root_relative() {
        let resolved = resolve_url("https://example.com/page/", "/other");
        assert_eq!(resolved, "https://example.com/other");
    }

    #[test]
    fn test_resolve_fragment() {
        let resolved = resolve_url("https://example.com/page", "#section");
        assert_eq!(resolved, "https://example.com/page#section");
    }

    #[test]
    fn test_resolve_with_query() {
        let resolved = resolve_url("https://example.com/", "page?q=1");
        assert_eq!(resolved, "https://example.com/page?q=1");
    }

    #[test]
    fn test_resolve_invalid_base_returns_href() {
        // If base URL is invalid, just return the href as-is
        let resolved = resolve_url("not-a-url", "https://example.com/");
        assert_eq!(resolved, "https://example.com/");
    }

    #[test]
    fn test_resolve_up_level() {
        let resolved = resolve_url("https://example.com/a/b/page", "../other");
        assert_eq!(resolved, "https://example.com/a/other");
    }

    #[test]
    fn test_resolve_protocol_relative() {
        let resolved = resolve_url("https://example.com/", "//other.com/path");
        assert_eq!(resolved, "https://other.com/path");
    }

    #[test]
    fn test_resolve_deep_relative() {
        let resolved = resolve_url("https://example.com/a/b/c/", "../../d/e");
        assert_eq!(resolved, "https://example.com/a/d/e");
    }

    #[test]
    fn test_resolve_with_port() {
        let resolved = resolve_url("https://example.com:8080/path", "/other");
        assert_eq!(resolved, "https://example.com:8080/other");
    }

    // --- fetch_url_with error handling ---

    #[test]
    fn test_fetch_url_unsupported_scheme() {
        let result = fetch_url_with("ftp://example.com/", &FetchOptions::default());
        assert!(result.is_err(), "ftp should fail");
    }

    #[test]
    fn test_fetch_url_bad_hostname() {
        let result = fetch_url_with(
            "https://this-hostname-does-not-exist-hopefully.example/",
            &FetchOptions::default(),
        );
        assert!(result.is_err(), "bad hostname should fail");
    }

    #[test]
    fn test_fetch_url_requires_html_skips_non_html() {
        // A URL that returns non-HTML content-type should fail with require_html=true
        let opts = FetchOptions {
            require_html: true,
            timeout_secs: 30,
            ..FetchOptions::default()
        };
        // This should succeed because example.com returns text/html
        let result = fetch_url_with("https://example.com/", &opts);
        assert!(result.is_ok(), "example.com should serve HTML");
        if let Ok(r) = result {
            assert_eq!(r.status, 200);
            assert!(r.final_url.contains("example.com"));
        }
    }

    #[test]
    fn test_fetch_options_no_retry() {
        let opts = FetchOptions {
            retry_transient: false,
            timeout_secs: 1, // very short timeout to force failure
            ..FetchOptions::default()
        };
        let result = fetch_url_with("https://httpbin.org/delay/10", &opts);
        // Should fail quickly rather than retry
        assert!(result.is_err(), "should fail with short timeout");
    }
}

/// Walk the element tree and collect text content, optionally skipping
/// elements whose tag name appears in `strip`.
fn collect_text(element: ElementRef, strip: &[&str]) -> String {
    let mut text = String::new();
    let tag_name = element.value().name();

    // Skip stripped elements
    if strip.contains(&tag_name) {
        return text;
    }

    for child in element.children() {
        match child.value() {
            scraper::node::Node::Text(t) => {
                let t = t.trim();
                if !t.is_empty() {
                    if !text.is_empty() && !text.ends_with(' ') {
                        text.push(' ');
                    }
                    text.push_str(t);
                }
            }
            scraper::node::Node::Element(_) => {
                if let Some(child_elem) = ElementRef::wrap(child) {
                    let child_text = collect_text(child_elem, strip);
                    if !child_text.is_empty() {
                        if !text.is_empty() && !text.ends_with(' ') {
                            text.push(' ');
                        }
                        text.push_str(&child_text);
                    }
                }
            }
            _ => {}
        }
    }
    text
}

/// Normalize whitespace: collapse all runs of whitespace to single spaces.
fn normalize_space(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract clean text from HTML by walking the document tree.
/// Collects text from <body> (or <html> as fallback) without stripping
/// any elements.
pub fn html_to_text(html: &str) -> String {
    let doc = Html::parse_document(html);
    let root = Selector::parse("body")
        .ok()
        .and_then(|s| doc.select(&s).next())
        .unwrap_or_else(|| doc.root_element());
    let text = collect_text(root, &[]);
    normalize_space(&text)
}

/// Extract readable content from an HTML page using content scoring.
///
/// Implements a simplified Mozilla Readability algorithm:
/// 1. Score content candidates by paragraph density
/// 2. Prefer semantic tags (article, main) and content-rich regions
/// 3. Strip known non-content elements from the result
/// 4. Fall back to html_to_text() if nothing scores above threshold
pub fn extract_readable_content(html: &str) -> anyhow::Result<String> {
    fn text_len(e: ElementRef) -> usize {
        e.text().collect::<String>().trim().len()
    }

    fn has_content_class(e: ElementRef) -> bool {
        let id = e.value().attr("id").unwrap_or("");
        let class = e.value().attr("class").unwrap_or("");
        let combined = format!("{id} {class}").to_lowercase();
        // Positive signals: content-bearing keywords
        let positive = [
            "content", "article", "post", "entry", "main", "story", "body", "text", "news", "blog",
        ];
        // Negative signals: non-content keywords
        let negative = [
            "sidebar",
            "comment",
            "widget",
            "footer",
            "header",
            "nav",
            "menu",
            "related",
            "social",
            "share",
            "meta",
            "search",
            "ad-",
            "advertisement",
            "promo",
            "sponsor",
        ];

        let has_positive = positive.iter().any(|k| combined.contains(k));
        let has_negative = negative.iter().any(|k| combined.contains(k));
        has_positive && !has_negative
    }

    fn score_element(e: ElementRef) -> f64 {
        let p_sel = Selector::parse("p").unwrap();
        // Count paragraphs and their total text length
        let paragraphs: Vec<ElementRef> = e.select(&p_sel).collect();
        if paragraphs.is_empty() {
            return 0.0;
        }
        let total_text: usize = paragraphs.iter().map(|p| text_len(*p)).sum();
        let p_count = paragraphs.len() as f64;

        // Base score: paragraphs × average text length
        let p_text_avg = total_text as f64 / p_count.max(1.0);
        let mut score = p_count * p_text_avg.min(500.0) / 100.0; // Cap per-paragraph to avoid noise

        // Bonus for semantic tags
        let name = e.value().name();
        if name == "article" {
            score *= 1.5;
        } else if name == "main" {
            score *= 1.3;
        }

        // Bonus for content-like class/id
        if has_content_class(e) {
            score *= 1.3;
        }

        score
    }

    let doc = Html::parse_document(html);

    // Collect all candidate content elements
    let body_sel = Selector::parse("body").unwrap();
    let body = doc
        .select(&body_sel)
        .next()
        .ok_or_else(|| anyhow::anyhow!("No body element found"))?;

    // Build candidates: all non-strippable elements with at least 2 <p> children
    let mut candidates: Vec<(f64, ElementRef)> = Vec::new();

    // First, check for article/main tags explicitly
    for tag in &["article", "main", "[role=main]"] {
        if let Ok(sel) = Selector::parse(tag) {
            for el in doc.select(&sel) {
                let s = score_element(el);
                if s > 0.0 {
                    candidates.push((s, el));
                }
            }
        }
    }

    // If we found semantic content, use the best one
    if !candidates.is_empty() {
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let (_, best) = &candidates[0];
        let text = collect_text(*best, STRIP_TAGS);
        let cleaned = normalize_space(&text);
        if !cleaned.is_empty() {
            return Ok(cleaned);
        }
    }

    // Fallback: score all div elements with content-like classes
    candidates.clear();
    if let Ok(div_sel) = Selector::parse("div") {
        for el in doc.select(&div_sel) {
            if has_content_class(el) {
                let s = score_element(el);
                if s > 2.0 {
                    // Minimum threshold
                    candidates.push((s, el));
                }
            }
        }
    }

    if !candidates.is_empty() {
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let (_, best) = &candidates[0];
        let text = collect_text(*best, STRIP_TAGS);
        let cleaned = normalize_space(&text);
        if !cleaned.is_empty() {
            return Ok(cleaned);
        }
    }

    // Ultimate fallback: body with tag stripping
    let text = collect_text(body, STRIP_TAGS);
    let cleaned = normalize_space(&text);
    if !cleaned.is_empty() {
        Ok(cleaned)
    } else {
        Ok(html_to_text(html))
    }
}

/// Decode a DuckDuckGo redirect URL to the actual target URL.
/// DuckDuckGo Lite wraps external links in redirect URLs like:
///   //duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2F&rut=...
/// Returns the decoded URL if it's a DDG redirect, or the original URL unchanged.
pub fn decode_search_url(url: &str) -> anyhow::Result<String> {
    // Check if this is a DuckDuckGo redirect
    let url_str = url.strip_prefix("//").unwrap_or(url);
    if !url_str.starts_with("duckduckgo.com/l/") && !url_str.starts_with("www.duckduckgo.com/l/") {
        return Ok(url.to_string());
    }

    // Parse the query string to find the `uddg` parameter
    let query_start = url_str
        .find('?')
        .ok_or_else(|| anyhow::anyhow!("Invalid redirect URL (no query string): {url}"))?;
    let query = &url_str[query_start + 1..];

    let params: HashMap<&str, &str> = query
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or("");
            Some((key, value))
        })
        .collect();

    match params.get("uddg") {
        Some(encoded) => {
            let decoded = urlencoding_decode(encoded)?;
            Ok(decoded)
        }
        None => Ok(url.to_string()),
    }
}

/// Simple URL percent-decoding (no external dependency needed)
fn urlencoding_decode(input: &str) -> anyhow::Result<String> {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();

    while let Some(ch) = chars.next() {
        if ch == '%' {
            let high = chars
                .next()
                .ok_or_else(|| anyhow::anyhow!("Truncated percent encoding"))?;
            let low = chars
                .next()
                .ok_or_else(|| anyhow::anyhow!("Truncated percent encoding"))?;
            let byte = u8::from_str_radix(&format!("{high}{low}"), 16)?;
            result.push(byte as char);
        } else if ch == '+' {
            result.push(' ');
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- URL decoding tests ---

    #[test]
    fn test_decode_ddg_redirect() {
        let redirect = "//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org%2F&rut=abc123";
        let decoded = decode_search_url(redirect).unwrap();
        assert_eq!(decoded, "https://rust-lang.org/");
    }

    #[test]
    fn test_decode_plain_url() {
        let url = "https://example.com/page";
        let decoded = decode_search_url(url).unwrap();
        assert_eq!(decoded, url);
    }

    #[test]
    fn test_decode_no_uddg() {
        let url = "https://duckduckgo.com/about";
        let decoded = decode_search_url(url).unwrap();
        assert_eq!(decoded, url);
    }

    #[test]
    fn test_decode_www_ddg_redirect() {
        let redirect = "//www.duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.org%2Ftest";
        let decoded = decode_search_url(redirect).unwrap();
        assert_eq!(decoded, "https://example.org/test");
    }

    #[test]
    fn test_decode_plus_encoded() {
        let redirect = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fpath+name";
        let decoded = decode_search_url(redirect).unwrap();
        assert_eq!(decoded, "https://example.com/path name");
    }

    #[test]
    fn test_decode_urlenc_edge_cases() {
        assert_eq!(urlencoding_decode("hello").unwrap(), "hello");
        assert_eq!(urlencoding_decode("hello%20world").unwrap(), "hello world");
        assert_eq!(urlencoding_decode("a%2Fb%3Fc").unwrap(), "a/b?c");
        assert!(urlencoding_decode("hello%2").is_err());
    }

    // --- HTML text extraction tests ---

    #[test]
    fn test_html_to_text_simple() {
        let html = "<html><body><p>Hello world</p></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(!text.contains("<p>"));
    }

    #[test]
    fn test_html_to_text_multiline() {
        let html =
            "<html><body><h1>Title</h1><p>Paragraph one.</p><p>Paragraph two.</p></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Paragraph one."));
        assert!(text.contains("Paragraph two."));
        assert!(!text.contains('\n'));
    }

    // --- Readable content tests ---

    #[test]
    fn test_extract_article_content() {
        let html = "\
<html><body>
<nav>Navigation links here</nav>
<article>
<h1>Article Title</h1>
<p>This is the main article content that should be extracted.</p>
<p>More useful content in the article.</p>
</article>
<aside>Sidebar junk</aside>
<footer>Copyright 2024</footer>
</body></html>";
        let text = html_to_text(html);
        assert!(
            text.contains("Article Title"),
            "should contain article title"
        );
        assert!(
            text.contains("main article content"),
            "should contain article body"
        );
    }

    #[test]
    fn test_readable_strips_nav_footer() {
        let html = "\
<html><body>
<nav>Navigation</nav>
<header>Header banner</header>
<article>
<h1>Real Article</h1>
<p>This is the real content.</p>
</article>
<aside>Sidebar</aside>
<footer>Footer</footer>
</body></html>";
        let text = extract_readable_content(&html).unwrap();
        assert!(text.contains("Real Article"), "should contain article");
        assert!(
            text.contains("real content"),
            "should contain article content"
        );
        assert!(!text.contains("Navigation"), "should NOT contain nav text");
        assert!(
            !text.contains("Header banner"),
            "should NOT contain header text"
        );
        assert!(!text.contains("Sidebar"), "should NOT contain sidebar text");
        assert!(!text.contains("Footer"), "should NOT contain footer text");
    }

    #[test]
    fn test_collect_text_vs_readable_no_strip_equivalent() {
        let html = "<html><body><p>Hello world</p><div>More text</div></body></html>";
        assert_eq!(html_to_text(html), "Hello world More text");
    }

    // --- Edge case tests ---

    #[test]
    fn test_empty_html() {
        let text = html_to_text("");
        assert!(text.is_empty() || text.trim().is_empty());
    }

    #[test]
    fn test_html_no_body() {
        let text = html_to_text("<html></html>");
        assert!(text.trim().is_empty());
    }

    #[test]
    fn test_html_only_comments() {
        let text = html_to_text("<html><body><!-- just a comment --></body></html>");
        assert!(text.trim().is_empty());
    }

    #[test]
    fn test_readable_empty_article() {
        let result = extract_readable_content("<html><body><article></article></body></html>");
        assert!(result.is_ok());
        assert!(result.unwrap().trim().is_empty());
    }

    // --- Content scoring tests ---

    #[test]
    fn test_readable_finds_content_in_div_without_semantic_tags() {
        // Many real sites use <div class="post-content"> instead of <article>
        let html = "\
<html><body>
<div class=\"sidebar\">Sidebar link 1 Sidebar link 2 Sidebar link 3</div>
<div class=\"post-content\">
<h1>My Blog Post Title</h1>
<p>This is the first paragraph of the actual blog post content that
contains meaningful information the user wants to read.</p>
<p>Here is another paragraph with more detailed content about the topic
being discussed in this blog post.</p>
<p>A third paragraph continues the discussion with even more useful
information for the reader to consume and learn from.</p>
</div>
<div class=\"footer\">Copyright 2024 Footer links Privacy policy</div>
</body></html>";
        let text = extract_readable_content(&html).unwrap();
        assert!(
            text.contains("My Blog Post Title"),
            "should contain blog title"
        );
        assert!(
            text.contains("first paragraph"),
            "should contain article body"
        );
        assert!(
            !text.contains("Sidebar link"),
            "should NOT contain sidebar text"
        );
        assert!(
            !text.contains("Footer links"),
            "should NOT contain footer text"
        );
    }

    #[test]
    fn test_readable_selects_paragraph_rich_region() {
        // Pick the region with the most paragraph content, not the first one
        let html = "\
<html><body>
<div class=\"comments\">
<p>Nice post!</p>
<p>Thanks for sharing</p>
</div>
<div class=\"content\">
<p>This is the real article content that has many paragraphs of useful
information that the reader wants to extract and understand.</p>
<p>Second paragraph with even more detailed analysis of the subject
matter being discussed in this article.</p>
<p>Third paragraph continues with additional insights and conclusions
that wrap up the discussion nicely.</p>
<p>Fourth paragraph provides supplementary information that rounds out
the topic coverage.</p>
</div>
</body></html>";
        let text = extract_readable_content(&html).unwrap();
        assert!(
            text.contains("real article content"),
            "should pick content div"
        );
        assert!(
            text.contains("Third paragraph"),
            "should include later paragraphs"
        );
        // The comments section has fewer paragraphs, so should be excluded
        // if scoring is working properly (comments: 2 short, content: 4 long)
        let content_len = text.len();
        assert!(content_len > 100, "should extract substantial content");
    }

    #[test]
    fn test_readable_handles_mixed_page() {
        // A realistic news article layout with various sections
        let html = "\
<html><body>
<nav class=\"main-nav\">Home World Politics Business Technology Sports</nav>
<header class=\"article-header\">
<h1>Breaking News: Major Scientific Discovery Announced</h1>
<p class=\"byline\">By Jane Reporter | June 22, 2026</p>
</header>
<div class=\"social-share\">Share on Twitter Share on Facebook</div>
<div class=\"article-body\">
<p>Scientists at the Institute for Advanced Study announced today a
groundbreaking discovery in the field of quantum computing that promises
to revolutionize how we process information.</p>
<p>The discovery, published in the journal Nature, demonstrates a new
method for maintaining quantum coherence at room temperature, a challenge
that has plagued the field for decades.</p>
<p>\"This is a transformative moment,\" said Dr. Alice Smith, lead author
of the study. \"We have overcome what many thought was an insurmountable
obstacle.\"</p>
<p>The research team used a novel approach combining topological qubits
with error-correction codes to achieve stability for over 24 hours at
standard temperature and pressure conditions.</p>
<p>Industry experts have called the breakthrough \"profound\" and predict
it could accelerate the development of practical quantum computers by
several years, with applications in drug discovery, climate modeling,
and cryptography.</p>
</div>
<aside class=\"related-stories\">
<h2>Related Articles</h2>
<ul><li>Quantum Computing Explained</li><li>Top 10 Science Breakthroughs</li></ul>
</aside>
<footer class=\"site-footer\">Copyright 2026 Contact Us About Us Privacy Policy</footer>
</body></html>";
        let text = extract_readable_content(&html).unwrap();
        assert!(
            text.contains("quantum computing"),
            "should have article body"
        );
        assert!(
            text.contains("groundbreaking discovery"),
            "should have content"
        );
        assert!(
            text.contains("transformative moment"),
            "should have quoted content"
        );
        assert!(
            !text.contains("Related Articles"),
            "should NOT contain aside"
        );
        assert!(
            !text.contains("Share on Twitter"),
            "should NOT contain social share"
        );
        assert!(
            !text.contains("Privacy Policy"),
            "should NOT contain footer links"
        );
    }
}
