use scraper::{ElementRef, Html, Selector};
use std::collections::HashMap;

/// Tags we strip from readable content extraction.
const STRIP_TAGS: &[&str] = &["nav", "header", "footer", "aside", "script", "style"];

/// Fetch a URL and return the HTML body as a String.
pub fn fetch_url(url: &str) -> anyhow::Result<String> {
    let response = ureq::get(url).header("User-Agent", &user_agent()).call()?;
    let body = response.into_body().read_to_string()?;
    Ok(body)
}

/// Return a compatible User-Agent string to avoid blocking.
pub fn user_agent() -> String {
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15".to_string()
}

#[cfg(test)]
mod fetch_tests {
    use super::*;

    #[test]
    fn test_user_agent_format() {
        let ua = user_agent();
        assert!(ua.contains("Mozilla/5.0"), "UA should look like a browser");
        assert!(ua.contains("AppleWebKit/"), "UA should contain AppleWebKit");
        assert!(ua.contains("Safari/"), "UA should contain Safari token");
    }

    #[test]
    fn test_fetch_url_unsupported_scheme() {
        let result = fetch_url("ftp://example.com/");
        assert!(result.is_err(), "ftp should fail");
    }

    #[test]
    fn test_fetch_url_bad_hostname() {
        let result = fetch_url("https://this-hostname-does-not-exist-hopefully.example/");
        assert!(result.is_err(), "bad hostname should fail");
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

/// Get the root content element from a parsed document: try <article>,
/// then <main>, then <body>, falling back to <html>.
fn content_root(doc: &Html) -> ElementRef<'_> {
    Selector::parse("article")
        .ok()
        .and_then(|s| doc.select(&s).next())
        .or_else(|| {
            Selector::parse("main")
                .ok()
                .and_then(|s| doc.select(&s).next())
        })
        .or_else(|| {
            Selector::parse("body")
                .ok()
                .and_then(|s| doc.select(&s).next())
        })
        .unwrap_or_else(|| doc.root_element())
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
    let root = content_root(&doc);
    let text = collect_text(root, &[]);
    normalize_space(&text)
}

/// Extract readable content from an HTML page, stripping navigation,
/// headers, footers, sidebars, and other non-content elements.
/// Targets <article> first, then <main>, then <body>.
/// Returns the cleaned text if readable content is found, or falls back
/// to html_to_text().
pub fn extract_readable_content(html: &str) -> anyhow::Result<String> {
    let doc = Html::parse_document(html);
    let root = content_root(&doc);
    let text = collect_text(root, STRIP_TAGS);
    let cleaned = normalize_space(&text);

    if cleaned.is_empty() {
        Ok(html_to_text(html))
    } else {
        Ok(cleaned)
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
}
