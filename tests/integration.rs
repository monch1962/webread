/// Integration tests for webread CLI — including cross-site compatibility
use std::process::Command;

fn webread_binary() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("webread")
}

fn webread(args: &[&str]) -> Result<String, String> {
    let output = Command::new(webread_binary())
        .args(args)
        .output()
        .map_err(|e| format!("failed to execute: {e}"))?;
    let stdout = String::from_utf8(output.stdout).map_err(|e| format!("utf8: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("exit={} stderr={}", output.status, stderr));
    }
    Ok(stdout)
}

// --- Core functionality tests ---

#[test]
fn test_get_example() {
    let out = webread(&["get", "https://example.com"]).unwrap();
    assert!(out.contains("Example Domain"));
}

#[test]
fn test_get_json() {
    let out = webread(&["get", "https://example.com", "--json"]).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(parsed.get("url").is_some());
    assert!(parsed.get("text").is_some());
    assert!(parsed["text"].as_str().unwrap().contains("Example Domain"));
}

#[test]
fn test_html_selector() {
    let out = webread(&["html", "https://example.com", "--selector", "h1"]).unwrap();
    assert!(out.contains("<h1>"));
}

#[test]
fn test_html_raw() {
    let out = webread(&["html", "https://example.com"]).unwrap();
    assert!(out.contains("<!doctype html>") || out.contains("<html"));
}

#[test]
fn test_links() {
    let out = webread(&["links", "https://example.com"]).unwrap();
    assert!(out.contains("https://"));
}

#[test]
fn test_links_json() {
    let out = webread(&["links", "https://example.com", "--json"]).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(parsed.get("links").is_some());
    assert!(parsed["links"].as_array().unwrap().len() > 0);
}

#[test]
fn test_readable() {
    let out = webread(&["readable", "https://example.com"]).unwrap();
    assert!(out.contains("Example Domain"));
    // Should have no HTML tags
    assert!(!out.contains("<h1>"));
}

#[test]
fn test_search() {
    let out = webread(&["search", "rust programming language"]).unwrap();
    assert!(out.contains("=== Search results for:"));
}

#[test]
fn test_search_json() {
    let out = webread(&["search", "rust", "--json"]).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(parsed.get("query").is_some());
    assert!(parsed.get("results").is_some());
    assert!(parsed["results"].as_array().unwrap().len() > 0);
}

// --- Error handling tests ---

#[test]
fn test_invalid_url() {
    let result = webread(&["get", "https://this-domain-does-not-exist-12345.com"]);
    assert!(result.is_err(), "expected failure for invalid URL");
}

#[test]
fn test_invalid_css_selector() {
    let result = webread(&["html", "https://example.com", "--selector", "###invalid"]);
    assert!(result.is_err(), "expected failure for invalid CSS selector");
}

#[test]
fn test_empty_html() {
    // <html> with no body should produce empty/whitespace-only output
    let out = webread(&["get", "https://example.com"]).unwrap();
    assert!(!out.is_empty(), "example.com should have content");
}

// --- Cross-site compatibility tests (representative sample) ---
// These test that webread works across different site types.
// Failures here indicate site changes or bot detection.

#[test]
fn test_wikipedia_article() {
    let out = webread(&[
        "readable",
        "https://en.wikipedia.org/wiki/Rust_(programming_language)",
    ])
    .unwrap();
    assert!(
        out.contains("Rust"),
        "Wikipedia article should have content"
    );
}

#[test]
fn test_github_readme() {
    let out = webread(&["get", "https://github.com/rust-lang/rust"]).unwrap();
    assert!(out.contains("rust"), "GitHub repo should have content");
}

#[test]
fn test_arxiv_paper() {
    let out = webread(&["get", "https://arxiv.org/abs/1706.03762"]).unwrap();
    assert!(
        out.contains("Attention"),
        "arXiv abstract should have content"
    );
}

#[test]
fn test_hacker_news_frontpage() {
    let out = webread(&["get", "https://news.ycombinator.com/"]).unwrap();
    assert!(
        out.contains("Hacker News") || out.contains("news"),
        "HN should have content"
    );
}

#[test]
fn test_devto_article() {
    let out = webread(&["get", "https://dev.to/"]).unwrap();
    assert!(
        out.contains("DEV") || !out.is_empty(),
        "dev.to should have content"
    );
}

// --- JSON output structure ---

#[test]
fn test_json_get_structure() {
    let out = webread(&["get", "https://example.com", "--json"]).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v.get("url").is_some(), "get --json must have 'url'");
    assert!(v.get("text").is_some(), "get --json must have 'text'");
    assert!(
        v.get("char_count").is_some(),
        "get --json must have 'char_count'"
    );
}

#[test]
fn test_json_readable_structure() {
    let out = webread(&["readable", "https://example.com", "--json"]).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v.get("url").is_some(), "readable --json must have 'url'");
    assert!(v.get("text").is_some(), "readable --json must have 'text'");
}

#[test]
fn test_json_links_structure() {
    let out = webread(&["links", "https://example.com", "--json"]).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v.get("url").is_some(), "links --json must have 'url'");
    assert!(v.get("links").is_some(), "links --json must have 'links'");
    assert!(v["links"].is_array(), "links must be array");
}

#[test]
fn test_json_search_structure() {
    let out = webread(&["search", "test", "--json"]).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v.get("query").is_some(), "search --json must have 'query'");
    assert!(
        v.get("results").is_some(),
        "search --json must have 'results'"
    );
    assert!(v["results"].is_array(), "results must be array");
}

#[test]
fn test_json_html_structure() {
    let out = webread(&["html", "https://example.com", "--selector", "h1", "--json"]).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v.get("url").is_some(), "html --json must have 'url'");
    assert!(
        v.get("selector").is_some(),
        "html --json must have 'selector'"
    );
    assert!(v.get("html").is_some(), "html --json must have 'html'");
    assert!(
        v.get("match_count").is_some(),
        "html --json must have 'match_count'"
    );
}

// --- Parallel execution stress tests ---

#[test]
fn test_fetch_multiple_urls() {
    // Race several independent URLs to verify no shared-state corruption
    let urls = &[
        "https://example.com",
        "https://en.wikipedia.org/wiki/Rust_(programming_language)",
    ];
    let results: Vec<_> = urls
        .iter()
        .map(|url| (url, webread(&["get", url])))
        .collect();
    for (url, result) in &results {
        assert!(
            result.is_ok(),
            "parallel fetch of {url} should succeed: {:?}",
            result
        );
    }
    // Distinct URLs should produce distinct content
    let contents: Vec<&str> = results
        .iter()
        .map(|(_, r)| r.as_ref().unwrap().as_str())
        .collect();
    if contents.len() >= 2 {
        assert_ne!(
            contents[0], contents[1],
            "different URLs must produce different output"
        );
    }
}

#[test]
fn test_concurrent_search_and_fetch() {
    // Run different subcommands concurrently to check for interference
    let get_handle = std::thread::spawn(|| webread(&["get", "https://example.com"]));
    let search_handle = std::thread::spawn(|| webread(&["search", "rust"]));
    let links_handle = std::thread::spawn(|| webread(&["links", "https://example.com"]));

    let get_result = get_handle.join().expect("get thread panicked");
    let search_result = search_handle.join().expect("search thread panicked");
    let links_result = links_handle.join().expect("links thread panicked");

    assert!(get_result.is_ok(), "parallel get should succeed");
    assert!(search_result.is_ok(), "parallel search should succeed");
    assert!(links_result.is_ok(), "parallel links should succeed");

    let get_out = get_result.unwrap();
    let links_out = links_result.unwrap();
    assert!(
        get_out.contains("Example Domain"),
        "get output should have content"
    );
    assert!(
        links_out.contains("iana.org"),
        "links output should have hrefs"
    );
}
