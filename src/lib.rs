use scraper::{Html, Selector};
use std::collections::HashMap;

/// Fetch a URL and return the HTML body as a String.
pub fn fetch_url(url: &str) -> anyhow::Result<String> {
    let response = ureq::get(url).call()?;
    let body = response.into_body().read_to_string()?;
    Ok(body)
}

/// Extract clean text from HTML by walking the document tree.
pub fn html_to_text(html: &str) -> String {
    let doc = Html::parse_document(html);
    fn collect_text(element: scraper::ElementRef) -> String {
        let mut text = String::new();

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
                    if let Some(child_elem) = scraper::ElementRef::wrap(child) {
                        let child_text = collect_text(child_elem);
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

    let root = Selector::parse("body")
        .ok()
        .and_then(|s| doc.select(&s).next());

    let text = if let Some(body) = root {
        collect_text(body)
    } else {
        let html_el = doc.root_element();
        collect_text(html_el)
    };

    text.split_whitespace().collect::<Vec<_>>().join(" ")
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

    #[test]
    fn test_html_to_text_simple() {
        let html = "<html><body><p>Hello world</p></body></html>";
        let text = html_to_text(html);
        // Should extract clean text without HTML tags
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
        // Should be single line
        assert!(!text.contains('\n'));
    }
}
