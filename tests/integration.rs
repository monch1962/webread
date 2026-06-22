/// Integration tests for webread CLI
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

#[test]
fn test_get_example() {
    let output = Command::new(webread_binary())
        .args(["get", "https://example.com"])
        .output()
        .expect("failed to run webread get");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "webread get failed: {stdout}");
    assert!(
        stdout.contains("Example Domain"),
        "expected 'Example Domain' in output"
    );
}

#[test]
fn test_html_selector() {
    let output = Command::new(webread_binary())
        .args(["html", "https://example.com", "--selector", "h1"])
        .output()
        .expect("failed to run webread html");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "webread html failed: {stdout}");
    assert!(stdout.contains("<h1>"), "expected <h1> tag in output");
}

#[test]
fn test_html_raw() {
    let output = Command::new(webread_binary())
        .args(["html", "https://example.com"])
        .output()
        .expect("failed to run webread html (raw)");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "webread html failed: {stdout}");
    assert!(
        stdout.contains("<!doctype html>") || stdout.contains("<html"),
        "expected raw HTML in output"
    );
}

#[test]
fn test_links() {
    let output = Command::new(webread_binary())
        .args(["links", "https://example.com"])
        .output()
        .expect("failed to run webread links");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "webread links failed: {stdout}");
    assert!(stdout.contains("https://"), "expected a link in output");
}

#[test]
fn test_search() {
    let output = Command::new(webread_binary())
        .args(["search", "rust programming language"])
        .output()
        .expect("failed to run webread search");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "webread search failed: {stdout}");
    assert!(
        stdout.contains("=== Search results for:"),
        "expected search header"
    );
}

#[test]
fn test_invalid_url() {
    let output = Command::new(webread_binary())
        .args(["get", "https://this-domain-does-not-exist-12345.com"])
        .output()
        .expect("failed to run webread get");
    assert!(!output.status.success(), "expected failure for invalid URL");
}

#[test]
fn test_invalid_css_selector() {
    let output = Command::new(webread_binary())
        .args(["html", "https://example.com", "--selector", "###invalid"])
        .output()
        .expect("failed to run webread html");
    assert!(
        !output.status.success(),
        "expected failure for invalid CSS selector"
    );
}

#[test]
fn test_readable() {
    let output = Command::new(webread_binary())
        .args(["readable", "https://example.com"])
        .output()
        .expect("failed to run webread readable");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "webread readable failed: {stdout}");
    assert!(
        stdout.contains("Example Domain"),
        "expected 'Example Domain' in readable output"
    );
    // Should be clean text, no HTML tags
    assert!(
        !stdout.contains("<h1>"),
        "readable should not contain HTML tags"
    );
}

#[test]
fn test_get_json() {
    let output = Command::new(webread_binary())
        .args(["get", "https://example.com", "--json"])
        .output()
        .expect("failed to run webread get --json");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.status.success(),
        "webread get --json failed: {stdout}"
    );
    // Should be valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");
    assert!(
        parsed.get("url").is_some(),
        "JSON should contain 'url' field"
    );
    assert!(
        parsed.get("text").is_some(),
        "JSON should contain 'text' field"
    );
    let text = parsed["text"].as_str().unwrap_or("");
    assert!(
        text.contains("Example Domain"),
        "JSON text should contain content"
    );
}

#[test]
fn test_links_json() {
    let output = Command::new(webread_binary())
        .args(["links", "https://example.com", "--json"])
        .output()
        .expect("failed to run webread links --json");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.status.success(),
        "webread links --json failed: {stdout}"
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");
    assert!(
        parsed.get("url").is_some(),
        "JSON should contain 'url' field"
    );
    assert!(
        parsed.get("links").is_some(),
        "JSON should contain 'links' array"
    );
}

#[test]
fn test_search_json() {
    let output = Command::new(webread_binary())
        .args(["search", "rust programming", "--json"])
        .output()
        .expect("failed to run webread search --json");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.status.success(),
        "webread search --json failed: {stdout}"
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");
    assert!(
        parsed.get("query").is_some(),
        "JSON should contain 'query' field"
    );
    assert!(
        parsed.get("results").is_some(),
        "JSON should contain 'results' array"
    );
}
