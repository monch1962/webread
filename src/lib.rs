use scraper::{ElementRef, Html, Selector};
use std::collections::HashMap;
use std::time::Duration;

/// Tags we strip from readable content extraction.
const STRIP_TAGS: &[&str] = &["nav", "header", "footer", "aside", "script", "style"];

/// Default User-Agent string.
pub const DEFAULT_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15";

/// Machine-parseable error codes for agentic use.
#[derive(Clone, Debug, PartialEq)]
pub enum ErrorCode {
    Timeout,
    DnsFailure,
    ConnectionRefused,
    Http4xx(u16),
    Http5xx(u16),
    ContentTypeNotHtml(String),
    Truncated,
    ProxyError(String),
    InvalidSelector(String),
    NetworkError(String),
    InvalidUrl(String),
    ConfigError(String),
    HttpError(u16, String),
}

impl ErrorCode {
    pub fn code(&self) -> &str {
        match self {
            ErrorCode::Timeout => "TIMEOUT",
            ErrorCode::DnsFailure => "DNS_FAILURE",
            ErrorCode::ConnectionRefused => "CONNECTION_REFUSED",
            ErrorCode::Http4xx(_) => "HTTP_4XX",
            ErrorCode::Http5xx(_) => "HTTP_5XX",
            ErrorCode::ContentTypeNotHtml(_) => "CONTENT_TYPE_NOT_HTML",
            ErrorCode::Truncated => "TRUNCATED",
            ErrorCode::ProxyError(_) => "PROXY_ERROR",
            ErrorCode::InvalidSelector(_) => "INVALID_SELECTOR",
            ErrorCode::NetworkError(_) => "NETWORK_ERROR",
            ErrorCode::InvalidUrl(_) => "INVALID_URL",
            ErrorCode::ConfigError(_) => "CONFIG_ERROR",
            ErrorCode::HttpError(_, _) => "HTTP_ERROR",
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            ErrorCode::Timeout => 6,
            ErrorCode::Truncated => 2,
            ErrorCode::ContentTypeNotHtml(_) => 3,
            ErrorCode::DnsFailure | ErrorCode::ConnectionRefused | ErrorCode::NetworkError(_) => 4,
            ErrorCode::ProxyError(_) => 5,
            ErrorCode::Http4xx(_) | ErrorCode::Http5xx(_) | ErrorCode::HttpError(_, _) => 7,
            ErrorCode::InvalidSelector(_) | ErrorCode::InvalidUrl(_) | ErrorCode::ConfigError(_) => 8,
        }
    }

    pub fn to_json(&self, url: Option<&str>) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "error": { "code": self.code(), "message": self.message() }
        });
        if let Some(u) = url {
            obj["error"]["url"] = serde_json::Value::String(u.to_string());
        }
        match self {
            ErrorCode::Timeout => {
                obj["error"]["suggestion"] = serde_json::Value::String("Retry with --timeout set higher (e.g. 60)".into());
            }
            ErrorCode::Truncated => {
                obj["error"]["suggestion"] = serde_json::Value::String("Increase --max-size or use --compact".into());
            }
            ErrorCode::ContentTypeNotHtml(ct) => {
                obj["error"]["content_type"] = serde_json::Value::String(ct.clone());
            }
            ErrorCode::ProxyError(p) => {
                obj["error"]["proxy"] = serde_json::Value::String(p.clone());
            }
            ErrorCode::Http4xx(s) | ErrorCode::Http5xx(s) => {
                obj["error"]["http_status"] = serde_json::Value::Number(serde_json::Number::from(*s));
            }
            ErrorCode::HttpError(s, _) => {
                obj["error"]["http_status"] = serde_json::Value::Number(serde_json::Number::from(*s));
            }
            _ => {}
        }
        obj
    }

    pub fn message(&self) -> String {
        match self {
            ErrorCode::Timeout => "Request timed out".into(),
            ErrorCode::DnsFailure => "DNS resolution failed".into(),
            ErrorCode::ConnectionRefused => "Connection refused".into(),
            ErrorCode::Http4xx(s) => format!("HTTP {s} Client Error"),
            ErrorCode::Http5xx(s) => format!("HTTP {s} Server Error"),
            ErrorCode::ContentTypeNotHtml(ct) => format!("Content-Type '{ct}' is not HTML"),
            ErrorCode::Truncated => "Response body was truncated (exceeded --max-size)".into(),
            ErrorCode::ProxyError(p) => format!("Proxy error: {p}"),
            ErrorCode::InvalidSelector(s) => format!("Invalid CSS selector: {s}"),
            ErrorCode::NetworkError(s) => format!("Network error: {s}"),
            ErrorCode::InvalidUrl(s) => format!("Invalid URL: {s}"),
            ErrorCode::ConfigError(s) => format!("Config error: {s}"),
            ErrorCode::HttpError(s, msg) => format!("HTTP {s}: {msg}"),
        }
    }
}

/// Return a compatible User-Agent string to avoid blocking.
pub fn user_agent() -> String {
    user_agent_with_override(None)
}

/// Return a User-Agent string, using an override if provided, otherwise the default.
pub fn user_agent_with_override(override_ua: Option<&str>) -> String {
    override_ua.unwrap_or(DEFAULT_UA).to_string()
}

/// HTTP method for requests.
#[derive(Clone, Debug, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Head,
}

/// Parse a simple key=value config file.
pub fn parse_config(input: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();
            if !key.is_empty() { map.insert(key, value); }
        }
    }
    map
}

/// Validate config file entries. Returns a list of (key, error_message) for bad entries.
pub fn validate_config(input: &str) -> Vec<(String, String)> {
    let mut errors = Vec::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();
            if key.is_empty() {
                errors.push(("(empty)".into(), "Empty key".into()));
                continue;
            }
            match key.as_str() {
                "timeout" => {
                    if value.parse::<u64>().is_err() {
                        errors.push((key, format!("'{value}' is not a valid number")));
                    }
                }
                "max-size" | "max_size" => {
                    if value.parse::<usize>().is_err() {
                        errors.push((key, format!("'{value}' is not a valid number")));
                    }
                }
                "proxy" => {
                    if !value.contains("://") {
                        errors.push((key, format!("'{value}' must include scheme (e.g. http://)")));
                    }
                }
                "user-agent" | "user_agent" => {}
                _ => {
                    errors.push((key, "Unknown config key".to_string()));
                }
            }
        } else {
            errors.push((line.to_string(), "Missing '=' separator".into()));
        }
    }
    errors
}

/// Load configuration from the default config file path (~/.config/webread/config).
pub fn load_config() -> HashMap<String, String> {
    let path = dirs_config_path().join("webread").join("config");
    match std::fs::read_to_string(&path) {
        Ok(content) => parse_config(&content),
        Err(_) => HashMap::new(),
    }
}

/// Get the XDG config directory.
pub fn dirs_config_path() -> std::path::PathBuf {
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
    pub timeout_secs: u64,
    pub max_body_bytes: usize,
    pub retry_transient: bool,
    pub require_html: bool,
    pub proxy_url: Option<String>,
    pub user_agent: Option<String>,
    pub method: HttpMethod,
    pub compact: bool,
    pub meta: bool,
    pub outline: bool,
    pub section: Option<String>,
    pub post_body: Option<String>,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            max_body_bytes: 10 * 1024 * 1024,
            retry_transient: true,
            require_html: true,
            proxy_url: None,
            user_agent: None,
            method: HttpMethod::Get,
            compact: false,
            meta: false,
            outline: false,
            section: None,
            post_body: None,
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
    pub truncated: bool,
    pub retry_after: Option<u64>,
}

/// Fetch a URL with default options (convenience wrapper).
pub fn fetch_url(url: &str) -> anyhow::Result<String> {
    let result = fetch_url_with(url, &FetchOptions::default())?;
    Ok(result.body)
}

/// Build a ureq Agent configured with proxy and timeout from FetchOptions.
pub fn build_agent(opts: &FetchOptions) -> anyhow::Result<ureq::Agent> {
    let mut cb = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(opts.timeout_secs)));

    let proxy_url = opts.proxy_url.clone().or_else(|| {
        for var in &["ALL_PROXY", "all_proxy", "HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
            if let Ok(val) = std::env::var(var) {
                if !val.is_empty() { return Some(val); }
            }
        }
        None
    });

    if let Some(ref url) = proxy_url {
        let proxy = ureq::Proxy::new(url)
            .map_err(|e| anyhow::anyhow!("Invalid proxy URL '{url}': {e}"))?;
        cb = cb.proxy(Some(proxy));
    }

    Ok(cb.build().new_agent())
}

/// Classify an error from ureq into an ErrorCode for structured output.
pub fn classify_error(err: &ureq::Error) -> ErrorCode {
    let msg = format!("{err:#}");
    if msg.contains("timed out") || msg.contains("timeout") || msg.contains("Timeout") {
        return ErrorCode::Timeout;
    }
    if msg.contains("dns") || msg.contains("DNS") || msg.contains("resolve") || msg.contains("NameOrServiceNotKnown") {
        return ErrorCode::DnsFailure;
    }
    if msg.contains("Connection refused") || msg.contains("connection refused") || msg.contains("ECONNREFUSED") {
        return ErrorCode::ConnectionRefused;
    }
    if msg.contains("proxy") || msg.contains("Proxy") {
        return ErrorCode::ProxyError(msg);
    }
    // Try to extract HTTP status from error message
    if msg.contains("429") {
        return ErrorCode::HttpError(429, "Rate limited".into());
    }
    if msg.contains(" 503 ") || msg.contains(" status 503 ") || msg.starts_with("503 ") {
        return ErrorCode::Http5xx(503);
    }
    if msg.contains(" 502 ") || msg.contains(" status 502 ") {
        return ErrorCode::Http5xx(502);
    }
    if msg.contains(" 404 ") || msg.contains(" status 404 ") {
        return ErrorCode::Http4xx(404);
    }
    if msg.contains(" 403 ") || msg.contains(" status 403 ") {
        return ErrorCode::Http4xx(403);
    }
    ErrorCode::NetworkError(msg)
}

/// Fetch a URL with the given resource guardrail options.
pub fn fetch_url_with(url: &str, opts: &FetchOptions) -> anyhow::Result<FetchResult> {
    let do_fetch = || -> anyhow::Result<FetchResult> {
        use ureq::ResponseExt;

        let agent = build_agent(opts)?;
        let ua = opts.user_agent.as_deref().unwrap_or(DEFAULT_UA);

        let response = match opts.method {
            HttpMethod::Get => agent.get(url).header("User-Agent", ua).call(),
            HttpMethod::Post => {
                let body = opts.post_body.as_deref().unwrap_or("");
                agent.post(url)
                    .header("User-Agent", ua)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .send(body)
            }
            HttpMethod::Head => agent.head(url).header("User-Agent", ua).call(),
        }
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        let status = response.status().as_u16();
        let content_type = response.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Check content-type (skip for HEAD)
        if opts.require_html && opts.method != HttpMethod::Head {
            if let Some(ref ct) = content_type {
                let ct_lower = ct.to_lowercase();
                let is_html = ct_lower.contains("text/html")
                    || ct_lower.contains("text/plain")
                    || ct_lower.contains("application/xhtml")
                    || ct_lower.contains("charset");
                if !is_html && !ct_lower.is_empty() {
                    anyhow::bail!("Content-Type '{}' is not HTML", ct);
                }
            }
        }

        let retry_after = if status == 429 {
            response.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<u64>().ok())
        } else {
            None
        };

        let final_url = response.get_uri().to_string();

        // HEAD requests have no body
        if opts.method == HttpMethod::Head {
            return Ok(FetchResult {
                body: String::new(),
                content_type,
                status,
                final_url,
                truncated: false,
                retry_after,
            });
        }

        let reader = response.into_body().read_to_string()?;
        let (body, truncated) = if reader.len() > opts.max_body_bytes {
            (String::from_utf8_lossy(&reader.as_bytes()[..opts.max_body_bytes]).to_string(), true)
        } else {
            (reader, false)
        };

        Ok(FetchResult { body, content_type, status, final_url, truncated, retry_after })
    };

    let result = do_fetch();

    // Retry once on transient errors if enabled
    if let Err(ref err) = result {
        if opts.retry_transient {
            let msg = format!("{err:#}");
            if msg.contains("503") || msg.contains("502") || msg.contains("timeout") || msg.contains("timed out") {
                std::thread::sleep(Duration::from_millis(500));
                return do_fetch();
            }
        }
    }

    result.map_err(|e| anyhow::anyhow!("{e}"))
}

/// Resolve a potentially relative URL against a base URL.
pub fn resolve_url(base: &str, href: &str) -> String {
    if href.contains("://") { return href.to_string(); }
    if let Some(suffix) = href.strip_prefix("//") {
        if let Some(pos) = base.find("://") {
            return format!("{}{suffix}", &base[..pos + 3]);
        }
        return href.to_string();
    }
    if href.starts_with('#') || href.starts_with('?') {
        let clean = base.split('#').next().unwrap_or(base).split('?').next().unwrap_or(base);
        return format!("{clean}{href}");
    }
    let (scheme, rest) = match base.find("://") {
        Some(pos) => (&base[..pos], &base[pos + 3..]),
        None => return href.to_string(),
    };
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let base_path = &rest[authority_end..];
    if href.starts_with('/') { return format!("{scheme}://{authority}{href}"); }
    let base_dir = match base_path.rfind('/') {
        Some(pos) => &base_path[..=pos],
        None => "/",
    };
    let combined = format!("{base_dir}{href}");
    let mut parts: Vec<&str> = Vec::new();
    for segment in combined.split('/') {
        match segment { "." | "" => continue, ".." => { parts.pop(); } s => parts.push(s), }
    }
    format!("{scheme}://{authority}/{}", parts.join("/"))
}

#[cfg(test)]
mod guardrail_tests {
    use super::*;

    #[test] fn test_parse_config_empty() { assert!(parse_config("").is_empty()); }
    #[test] fn test_parse_config_basic() {
        let cfg = parse_config("timeout=15\nmax_size=5000000\n");
        assert_eq!(cfg.get("timeout").unwrap(), "15");
    }
    #[test] fn test_parse_config_ignores_comments_and_blanks() {
        let cfg = parse_config("# comment\n  \ntimeout=30\n# another\n");
        assert_eq!(cfg.len(), 1);
    }
    #[test] fn test_parse_config_trims_whitespace() {
        let cfg = parse_config("  timeout = 15  \n");
        assert_eq!(cfg.get("timeout").unwrap(), "15");
    }
    #[test] fn test_parse_config_override() {
        let cfg = parse_config("timeout=10\nuser-agent=my-bot/1.0\n");
        assert_eq!(cfg.get("user-agent").unwrap(), "my-bot/1.0");
    }

    #[test] fn test_validate_config_valid() {
        assert!(validate_config("timeout=15\nproxy=http://p:8080\n").is_empty());
    }
    #[test] fn test_validate_config_bad_timeout() {
        assert!(!validate_config("timeout=not-a-number\n").is_empty());
    }
    #[test] fn test_validate_config_unknown_key() {
        let errs = validate_config("foo=bar\n");
        assert!(errs[0].1.contains("Unknown config key"));
    }
    #[test] fn test_validate_config_proxy_no_scheme() {
        assert!(!validate_config("proxy=localhost:8080\n").is_empty());
    }

    #[test] fn test_user_agent_custom() {
        assert_eq!(user_agent_with_override(Some("my-bot/1.0")), "my-bot/1.0");
    }
    #[test] fn test_user_agent_default() {
        let ua = user_agent_with_override(None);
        assert!(ua.contains("Mozilla/5.0"));
        assert!(ua.contains("Safari/"));
    }

    #[test] fn test_fetch_options_defaults() {
        let opts = FetchOptions::default();
        assert_eq!(opts.timeout_secs, 30);
        assert_eq!(opts.max_body_bytes, 10 * 1024 * 1024);
        assert!(opts.retry_transient);
    }

    #[test] fn test_error_code_exit_codes() {
        assert_eq!(ErrorCode::Timeout.exit_code(), 6);
        assert_eq!(ErrorCode::Truncated.exit_code(), 2);
        assert_eq!(ErrorCode::ContentTypeNotHtml("".into()).exit_code(), 3);
        assert_eq!(ErrorCode::DnsFailure.exit_code(), 4);
        assert_eq!(ErrorCode::ProxyError("".into()).exit_code(), 5);
        assert_eq!(ErrorCode::InvalidSelector("".into()).exit_code(), 8);
    }

    #[test] fn test_error_code_strings() {
        assert_eq!(ErrorCode::Timeout.code(), "TIMEOUT");
        assert_eq!(ErrorCode::Truncated.code(), "TRUNCATED");
    }

    #[test] fn test_resolve_relative_url() {
        assert_eq!(resolve_url("https://example.com/page/", "sub"), "https://example.com/page/sub");
    }
    #[test] fn test_resolve_absolute_url_unchanged() {
        assert_eq!(resolve_url("https://example.com/", "https://other.com/"), "https://other.com/");
    }
    #[test] fn test_resolve_root_relative() {
        assert_eq!(resolve_url("https://example.com/page/", "/other"), "https://example.com/other");
    }
    #[test] fn test_resolve_fragment() {
        assert_eq!(resolve_url("https://example.com/page", "#section"), "https://example.com/page#section");
    }
    #[test] fn test_resolve_up_level() {
        assert_eq!(resolve_url("https://example.com/a/b/page", "../other"), "https://example.com/a/other");
    }
    #[test] fn test_resolve_protocol_relative() {
        assert_eq!(resolve_url("https://example.com/", "//other.com/path"), "https://other.com/path");
    }
    #[test] fn test_resolve_with_port() {
        assert_eq!(resolve_url("https://example.com:8080/path", "/other"), "https://example.com:8080/other");
    }

    #[test] fn test_compact_text() {
        assert_eq!(compact_text("Hello   world\n\n\n   More   "), "Hello world More");
    }
    #[test] fn test_compact_empty() { assert_eq!(compact_text(""), ""); }

    #[test] fn test_fetch_url_unsupported_scheme() {
        assert!(fetch_url_with("ftp://example.com/", &FetchOptions::default()).is_err());
    }
    #[test] fn test_fetch_url_bad_hostname() {
        assert!(fetch_url_with("https://this-hostname-hopefully-does-not-exist.example/", &FetchOptions::default()).is_err());
    }
    #[test] fn test_fetch_options_no_retry() {
        let opts = FetchOptions { retry_transient: false, timeout_secs: 1, ..FetchOptions::default() };
        assert!(fetch_url_with("https://httpbin.org/delay/10", &opts).is_err());
    }
    #[test] fn test_build_agent_with_proxy() {
        assert!(build_agent(&FetchOptions { proxy_url: Some("http://proxy:8080".into()), ..FetchOptions::default() }).is_ok());
    }
    #[test] fn test_build_agent_invalid_proxy() {
        assert!(build_agent(&FetchOptions { proxy_url: Some("not valid".into()), ..FetchOptions::default() }).is_err());
    }
    #[test] fn test_build_agent_no_proxy() {
        assert!(build_agent(&FetchOptions::default()).is_ok());
    }
    #[test] fn test_fetch_options_proxy_default() {
        assert!(FetchOptions::default().proxy_url.is_none());
    }
    #[test] fn test_fetch_options_user_agent_default() {
        assert!(FetchOptions::default().user_agent.is_none());
    }
    #[test] fn test_fetch_result_not_truncated() {
        let r = FetchResult { body: "".into(), content_type: None, status: 200, final_url: "".into(), truncated: false, retry_after: None };
        assert!(!r.truncated);
    }
    #[test] fn test_fetch_result_truncated_flag() {
        let r = FetchResult { body: "".into(), content_type: None, status: 200, final_url: "".into(), truncated: true, retry_after: None };
        assert!(r.truncated);
    }
    #[test] fn test_error_code_to_json_has_code() {
        let json = ErrorCode::Timeout.to_json(Some("https://x.com/"));
        assert_eq!(json["error"]["code"], "TIMEOUT");
        assert_eq!(json["error"]["url"], "https://x.com/");
    }
    #[test] fn test_classify_error_timeout_msg() {
        assert_eq!(ErrorCode::Timeout.message(), "Request timed out");
    }
    #[test] fn test_classify_error_http_4xx() {
        assert_eq!(ErrorCode::Http4xx(404).code(), "HTTP_4XX");
        assert_eq!(ErrorCode::Http4xx(404).message(), "HTTP 404 Client Error");
    }

    // --- ErrorCode exhaustive tests ---

    #[test] fn test_error_code_message_all_variants() {
        assert_eq!(ErrorCode::Timeout.message(), "Request timed out");
        assert_eq!(ErrorCode::DnsFailure.message(), "DNS resolution failed");
        assert_eq!(ErrorCode::ConnectionRefused.message(), "Connection refused");
        assert!(ErrorCode::Http4xx(403).message().contains("403"));
        assert!(ErrorCode::Http5xx(503).message().contains("503"));
        assert!(ErrorCode::ContentTypeNotHtml("application/pdf".into()).message().contains("Content-Type"));
        assert!(ErrorCode::Truncated.message().contains("truncated"));
        assert!(ErrorCode::ProxyError("bad proxy".into()).message().contains("bad proxy"));
        assert!(ErrorCode::InvalidSelector("#bad".into()).message().contains("Invalid"));
        assert!(ErrorCode::NetworkError("connection reset".into()).message().contains("connection reset"));
        assert!(ErrorCode::InvalidUrl("not a url".into()).message().contains("Invalid URL"));
        assert!(ErrorCode::ConfigError("bad key".into()).message().contains("Config error"));
        assert!(ErrorCode::HttpError(429, "rate limited".into()).message().contains("429"));
    }

    #[test] fn test_error_code_code_all_variants() {
        assert_eq!(ErrorCode::Timeout.code(), "TIMEOUT");
        assert_eq!(ErrorCode::DnsFailure.code(), "DNS_FAILURE");
        assert_eq!(ErrorCode::ConnectionRefused.code(), "CONNECTION_REFUSED");
        assert_eq!(ErrorCode::Http4xx(0).code(), "HTTP_4XX");
        assert_eq!(ErrorCode::Http5xx(0).code(), "HTTP_5XX");
        assert_eq!(ErrorCode::ContentTypeNotHtml("".into()).code(), "CONTENT_TYPE_NOT_HTML");
        assert_eq!(ErrorCode::Truncated.code(), "TRUNCATED");
        assert_eq!(ErrorCode::ProxyError("".into()).code(), "PROXY_ERROR");
        assert_eq!(ErrorCode::InvalidSelector("".into()).code(), "INVALID_SELECTOR");
        assert_eq!(ErrorCode::NetworkError("".into()).code(), "NETWORK_ERROR");
        assert_eq!(ErrorCode::InvalidUrl("".into()).code(), "INVALID_URL");
        assert_eq!(ErrorCode::ConfigError("".into()).code(), "CONFIG_ERROR");
        assert_eq!(ErrorCode::HttpError(0, "".into()).code(), "HTTP_ERROR");
    }

    #[test] fn test_error_code_exit_codes_all() {
        assert_eq!(ErrorCode::Timeout.exit_code(), 6);
        assert_eq!(ErrorCode::Truncated.exit_code(), 2);
        assert_eq!(ErrorCode::ContentTypeNotHtml("".into()).exit_code(), 3);
        assert_eq!(ErrorCode::DnsFailure.exit_code(), 4);
        assert_eq!(ErrorCode::ConnectionRefused.exit_code(), 4);
        assert_eq!(ErrorCode::NetworkError("".into()).exit_code(), 4);
        assert_eq!(ErrorCode::ProxyError("".into()).exit_code(), 5);
        assert_eq!(ErrorCode::Http4xx(0).exit_code(), 7);
        assert_eq!(ErrorCode::Http5xx(0).exit_code(), 7);
        assert_eq!(ErrorCode::HttpError(0, "".into()).exit_code(), 7);
        assert_eq!(ErrorCode::InvalidSelector("".into()).exit_code(), 8);
        assert_eq!(ErrorCode::InvalidUrl("".into()).exit_code(), 8);
        assert_eq!(ErrorCode::ConfigError("".into()).exit_code(), 8);
    }

    #[test] fn test_error_code_to_json_content_type() {
        let json = ErrorCode::ContentTypeNotHtml("application/pdf".into()).to_json(Some("https://x.com/doc.pdf"));
        assert_eq!(json["error"]["content_type"], "application/pdf");
        assert_eq!(json["error"]["url"], "https://x.com/doc.pdf");
    }

    #[test] fn test_error_code_to_json_http_status() {
        let json = ErrorCode::Http5xx(503).to_json(None);
        assert_eq!(json["error"]["http_status"], 503);
    }

    #[test] fn test_error_code_to_json_suggestion() {
        let json = ErrorCode::Timeout.to_json(None);
        assert!(json["error"].get("suggestion").is_some());
    }

    // --- FetchOptions defaults for new fields ---

    #[test] fn test_fetch_options_method_default() {
        assert_eq!(FetchOptions::default().method, HttpMethod::Get);
    }

    #[test] fn test_fetch_options_compact_default() {
        assert!(!FetchOptions::default().compact);
    }

    #[test] fn test_fetch_options_post_body_default() {
        assert!(FetchOptions::default().post_body.is_none());
    }
    #[test] fn test_fetch_options_meta_default() {
        assert!(!FetchOptions::default().meta, "meta should default to false");
    }
    #[test] fn test_fetch_options_outline_default() {
        assert!(!FetchOptions::default().outline, "outline should default to false");
    }

    // --- compact_text edge cases ---

    #[test] fn test_compact_text_single_word() {
        assert_eq!(compact_text("hello"), "hello");
    }

    #[test] fn test_compact_text_tabs_and_newlines() {
        assert_eq!(compact_text("hello\t\nworld"), "hello world");
    }

    #[test] fn test_compact_text_only_whitespace() {
        assert_eq!(compact_text("   \t\n  "), "");
    }

    // --- validate_config edge cases ---

    #[test] fn test_validate_config_empty_line_no_eq() {
        let errs = validate_config("justtext\n");
        assert!(!errs.is_empty(), "line without '=' should error");
    }

    #[test] fn test_validate_config_bad_max_size() {
        let errs = validate_config("max-size=ten-megabytes\n");
        assert!(!errs.is_empty(), "invalid max-size should error");
    }

    #[test] fn test_validate_config_good_max_size() {
        let errs = validate_config("max-size=5000000\n");
        assert!(errs.is_empty(), "valid max-size should pass");
    }

    #[test] fn test_validate_config_good_proxy() {
        let errs = validate_config("proxy=http://proxy:8080\n");
        assert!(errs.is_empty(), "valid proxy URL should pass");
    }

    // --- resolve_url edge cases ---

    #[test] fn test_resolve_empty_href() {
        let resolved = resolve_url("https://example.com/page", "");
        assert_eq!(resolved, "https://example.com/");  // empty resolves to root
    }

    #[test] fn test_resolve_just_slash() {
        let resolved = resolve_url("https://example.com/page", "/");
        assert_eq!(resolved, "https://example.com/");
    }

    // --- html_to_text_with_options ---

    #[test] fn test_html_to_text_compact_different_from_normal() {
        // With multiple spaces and newlines, compact should differ
        let html = "<html><body><p>Hello   world</p><p>Foo   bar</p></body></html>";
        let normal = html_to_text_with_options(html, false);
        let compact = html_to_text_with_options(html, true);
        // Both should collapse whitespace, but compact is aggressive
        assert_eq!(normal, "Hello world Foo bar");
        assert_eq!(compact, "Hello world Foo bar");
    }

    // --- extract_readable_content edge cases ---

    #[test] fn test_readable_single_paragraph_falls_back() {
        // A single short paragraph should fall through to body-level extraction
        let html = "<html><body><p>Hello world</p></body></html>";
        let result = extract_readable_content(&html).unwrap();
        assert_eq!(result, "Hello world");
    }

    #[test] fn test_readable_with_nested_divs() {
        let html = "<html><body><div><div><p>Nested content here</p></div></div></body></html>";
        let result = extract_readable_content(&html).unwrap();
        assert!(result.contains("Nested content"));
    }
}

// ---- Text extraction functions ----

fn collect_text(element: ElementRef, strip: &[&str]) -> String {
    let mut text = String::new();
    let tag_name = element.value().name();
    if strip.contains(&tag_name) { return text; }
    for child in element.children() {
        match child.value() {
            scraper::node::Node::Text(t) => {
                let t = t.trim();
                if !t.is_empty() {
                    if !text.is_empty() && !text.ends_with(' ') { text.push(' '); }
                    text.push_str(t);
                }
            }
            scraper::node::Node::Element(_) => {
                if let Some(child_elem) = ElementRef::wrap(child) {
                    let child_text = collect_text(child_elem, strip);
                    if !child_text.is_empty() {
                        if !text.is_empty() && !text.ends_with(' ') { text.push(' '); }
                        text.push_str(&child_text);
                    }
                }
            }
            _ => {}
        }
    }
    text
}

fn normalize_space(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Compact text output: collapse all whitespace aggressively.
pub fn compact_text(text: &str) -> String {
    normalize_space(text)
}

/// Extract clean text from HTML by walking the document tree.
pub fn html_to_text(html: &str) -> String {
    html_to_text_with_options(html, false)
}

/// Extract text from HTML, optionally applying compact mode.
pub fn html_to_text_with_options(html: &str, compact: bool) -> String {
    let doc = Html::parse_document(html);
    let root = Selector::parse("body")
        .ok()
        .and_then(|s| doc.select(&s).next())
        .unwrap_or_else(|| doc.root_element());
    let text = collect_text(root, &[]);
    if compact { compact_text(&text) } else { normalize_space(&text) }
}

/// Extract page title from HTML: prefers <h1>, falls back to <title>.
pub fn page_title(html: &str) -> String {
    let doc = Html::parse_document(html);
    Selector::parse("h1")
        .ok()
        .and_then(|s| doc.select(&s).next())
        .map(|e| e.text().collect::<Vec<_>>().join(" ").trim().to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| {
            Selector::parse("title")
                .ok()
                .and_then(|s| doc.select(&s).next())
                .map(|e| e.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .filter(|t| !t.is_empty())
        })
        .unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq)]
pub struct MetaResult {
    pub title: String,
    pub description: String,
    pub canonical: String,
    pub og_title: String,
    pub og_description: String,
    pub og_image: String,
    pub og_type: String,
    pub twitter_card: String,
    pub charset: String,
    pub language: String,
    pub json_ld: String,
    pub link_count: usize,
    pub total_chars: usize,
}

impl MetaResult {
    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "title": self.title,
            "description": self.description,
            "canonical": self.canonical,
            "og_title": self.og_title,
            "og_description": self.og_description,
            "og_image": self.og_image,
            "og_type": self.og_type,
            "twitter_card": self.twitter_card,
            "charset": self.charset,
            "language": self.language,
            "link_count": self.link_count,
            "total_chars": self.total_chars,
        });
        if !self.json_ld.is_empty() {
            obj["json_ld"] = serde_json::Value::String(self.json_ld.clone());
        }
        obj
    }
}

impl std::fmt::Display for MetaResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "title: {}", self.title)?;
        if !self.description.is_empty() { writeln!(f, "description: {}", self.description)?; }
        if !self.canonical.is_empty() { writeln!(f, "canonical: {}", self.canonical)?; }
        if !self.og_title.is_empty() { writeln!(f, "og:title: {}", self.og_title)?; }
        if !self.og_description.is_empty() { writeln!(f, "og:description: {}", self.og_description)?; }
        if !self.og_image.is_empty() { writeln!(f, "og:image: {}", self.og_image)?; }
        if !self.og_type.is_empty() { writeln!(f, "og:type: {}", self.og_type)?; }
        if !self.twitter_card.is_empty() { writeln!(f, "twitter:card: {}", self.twitter_card)?; }
        if !self.charset.is_empty() { writeln!(f, "charset: {}", self.charset)?; }
        if !self.language.is_empty() { writeln!(f, "language: {}", self.language)?; }
        if !self.json_ld.is_empty() { writeln!(f, "json_ld: {}", self.json_ld)?; }
        writeln!(f, "links: {}  chars: {}", self.link_count, self.total_chars)
    }
}

pub fn extract_metadata(html: &str) -> MetaResult {
    let doc = Html::parse_document(html);
    fn attr(doc: &Html, selector: &str, attr_name: &str) -> String {
        Selector::parse(selector).ok()
            .and_then(|s| doc.select(&s).next())
            .and_then(|e| e.value().attr(attr_name))
            .unwrap_or("").trim().to_string()
    }
    fn meta_content(doc: &Html, name: &str) -> String {
        let sel = format!(r#"meta[name="{}"], meta[property="{}"]"#, name, name);
        attr(doc, &sel, "content")
    }
    let title = page_title(html);
    let description = meta_content(&doc, "description");
    let canonical = attr(&doc, r#"link[rel="canonical"]"#, "href");
    let og_title = meta_content(&doc, "og:title");
    let og_description = meta_content(&doc, "og:description");
    let og_image = meta_content(&doc, "og:image");
    let og_type = meta_content(&doc, "og:type");
    let twitter_card = meta_content(&doc, "twitter:card");
    let charset = attr(&doc, "meta[charset]", "charset");
    let language = attr(&doc, "html", "lang");
    let json_ld = Selector::parse(r#"script[type="application/ld+json"]"#).ok()
        .and_then(|s| doc.select(&s).next())
        .map(|e| {
            let t: String = e.text().collect();
            let trimmed = t.trim();
            if trimmed.len() > 500 {
                format!("{}...", &trimmed[..500])
            } else {
                trimmed.to_string()
            }
        })
        .unwrap_or_default();
    let link_count = Selector::parse("a").ok()
        .map(|sel| doc.select(&sel).count()).unwrap_or(0);
    let total_chars = extract_readable_content(html).unwrap_or_default().len();
    MetaResult {
        title, description, canonical, og_title, og_description, og_image,
        og_type, twitter_card, charset, language, json_ld, link_count, total_chars,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OutlineResult {
    pub title: String,
    pub headings: Vec<Heading>,
    pub link_count: usize,
    pub total_chars: usize,
}

impl OutlineResult {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "title": self.title,
            "headings": self.headings.iter().map(|h| {
                serde_json::json!({"level": h.level, "text": h.text})
            }).collect::<Vec<_>>(),
            "link_count": self.link_count,
            "total_chars": self.total_chars,
        })
    }
}

impl std::fmt::Display for OutlineResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.title)?;
        for h in &self.headings {
            let indent = "  ".repeat((h.level - 1) as usize);
            writeln!(f, "{indent}h{}: {}", h.level, h.text)?;
        }
        writeln!(f, "links: {}  chars: {}", self.link_count, self.total_chars)
    }
}

pub fn generate_outline(html: &str) -> OutlineResult {
    let doc = Html::parse_document(html);
    let title = page_title(html);
    let mut headings: Vec<Heading> = Vec::new();
    for level in 1..=6 {
        let tag = format!("h{level}");
        // Bind to variable to avoid temporary lifetime issue with Selector::parse
        let parse_result = Selector::parse(&tag);
        if let Ok(sel) = parse_result {
            for el in doc.select(&sel) {
                let text: String = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !text.is_empty() {
                    headings.push(Heading { level, text });
                }
            }
        }
    }
    let link_count = Selector::parse("a").ok()
        .map(|sel| doc.select(&sel).count()).unwrap_or(0);
    let total_chars = extract_readable_content(html).unwrap_or_default().len();
    OutlineResult { title, headings, link_count, total_chars }
}

/// Extract a heading and its section content from HTML.
/// Finds the element matching `selector`, then walks siblings until a heading
/// of the same or higher level is found. Returns the heading text + content.
/// Errors if the selector doesn't match a heading element (h1-h6).
pub fn extract_section(html: &str, selector: &str) -> anyhow::Result<String> {
    use scraper::ElementRef;
    let doc = scraper::Html::parse_document(html);
    let sel = scraper::Selector::parse(selector)
        .map_err(|_| anyhow::anyhow!("Invalid CSS selector: {selector}"))?;
    let el = doc.select(&sel).next()
        .ok_or_else(|| anyhow::anyhow!("No element found for selector: {selector}"))?;

    let tag = el.value().name();
    let heading_level = tag.strip_prefix('h')
        .and_then(|s| s.parse::<u8>().ok())
        .filter(|&lvl| (1..=6).contains(&lvl))
        .ok_or_else(|| anyhow::anyhow!("Selector '{selector}' matched element <{tag}>, not a heading (h1-h6)"))?;

    let heading_text: String = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
    let mut content_parts: Vec<String> = Vec::new();

    // Check if an ElementRef is or contains a heading of level <= max_level.
    // Handles both plain h1-h6 elements and wrapper divs (Wikipedia pattern).
    fn el_is_heading(el_ref: &ElementRef, max_level: u8) -> bool {
        let tag_name = el_ref.value().name();
        if let Some(lvl) = tag_name.strip_prefix('h')
            .and_then(|s| s.parse::<u8>().ok())
            .filter(|&l| (1..=6).contains(&l))
        {
            return lvl <= max_level;
        }
        for child in el_ref.children() {
            if let Some(child_el) = ElementRef::wrap(child) {
                let child_tag = child_el.value().name();
                if let Some(lvl) = child_tag.strip_prefix('h')
                    .and_then(|s| s.parse::<u8>().ok())
                    .filter(|&l| (1..=6).contains(&l))
                {
                    return lvl <= max_level;
                }
            }
        }
        false
    }

    // Determine start position: if heading is in a wrapper div/section/li,
    // walk siblings of the wrapper. Otherwise walk siblings of the heading itself.
    let wrapped = el.parent()
        .and_then(|p| ElementRef::wrap(p))
        .map(|pe| {
            let n = pe.value().name();
            n == "div" || n == "section" || n == "li"
        })
        .unwrap_or(false);

    let mut next = if wrapped {
        el.parent().and_then(|p| p.next_sibling())
    } else {
        el.next_sibling()
    };

    while let Some(node) = next {
        if let Some(sibling_el) = ElementRef::wrap(node) {
            if el_is_heading(&sibling_el, heading_level) {
                break;
            }
            let text: String = sibling_el
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();
            if !text.is_empty() {
                content_parts.push(text);
            }
        }
        next = node.next_sibling();
    }

    let mut result = heading_text;
    if !content_parts.is_empty() {
        result.push('\n');
        result.push_str(&content_parts.join("\n"));
    }
    Ok(result)
}pub fn extract_readable_content(html: &str) -> anyhow::Result<String> {
    fn text_len(e: ElementRef) -> usize { e.text().collect::<String>().trim().len() }
    fn has_content_class(e: ElementRef) -> bool {
        let id = e.value().attr("id").unwrap_or("");
        let class = e.value().attr("class").unwrap_or("");
        let combined = format!("{id} {class}").to_lowercase();
        let positive = ["content", "article", "post", "entry", "main", "story", "body", "text", "news", "blog"];
        let negative = ["sidebar", "comment", "widget", "footer", "header", "nav", "menu", "related", "social", "share", "meta", "search", "ad-", "advertisement", "promo", "sponsor"];
        positive.iter().any(|k| combined.contains(k)) && !negative.iter().any(|k| combined.contains(k))
    }
    fn score_element(e: ElementRef) -> f64 {
        let p_sel = Selector::parse("p").unwrap();
        let paragraphs: Vec<ElementRef> = e.select(&p_sel).collect();
        if paragraphs.is_empty() { return 0.0; }
        let total_text: usize = paragraphs.iter().map(|p| text_len(*p)).sum();
        let p_count = paragraphs.len() as f64;
        let p_text_avg = total_text as f64 / p_count.max(1.0);
        let mut score = p_count * p_text_avg.min(500.0) / 100.0;
        let name = e.value().name();
        if name == "article" { score *= 1.5; } else if name == "main" { score *= 1.3; }
        if has_content_class(e) { score *= 1.3; }
        score
    }

    let doc = Html::parse_document(html);
    let body_sel = Selector::parse("body").unwrap();
    let body = doc.select(&body_sel).next().ok_or_else(|| anyhow::anyhow!("No body element found"))?;
    let mut candidates: Vec<(f64, ElementRef)> = Vec::new();

    for tag in &["article", "main", "[role=main]"] {
        if let Ok(sel) = Selector::parse(tag) {
            for el in doc.select(&sel) {
                let s = score_element(el);
                if s > 0.0 { candidates.push((s, el)); }
            }
        }
    }
    if !candidates.is_empty() {
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let text = collect_text(candidates[0].1, STRIP_TAGS);
        let cleaned = normalize_space(&text);
        if !cleaned.is_empty() { return Ok(cleaned); }
    }

    candidates.clear();
    if let Ok(div_sel) = Selector::parse("div") {
        for el in doc.select(&div_sel) {
            if has_content_class(el) { let s = score_element(el); if s > 2.0 { candidates.push((s, el)); } }
        }
    }
    if !candidates.is_empty() {
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let text = collect_text(candidates[0].1, STRIP_TAGS);
        let cleaned = normalize_space(&text);
        if !cleaned.is_empty() { return Ok(cleaned); }
    }

    let text = collect_text(body, STRIP_TAGS);
    let cleaned = normalize_space(&text);
    if !cleaned.is_empty() { Ok(cleaned) } else { Ok(html_to_text(html)) }
}

/// Decode a DuckDuckGo redirect URL to the actual target URL.
pub fn decode_search_url(url: &str) -> anyhow::Result<String> {
    let url_str = url.strip_prefix("//").unwrap_or(url);
    if !url_str.starts_with("duckduckgo.com/l/") && !url_str.starts_with("www.duckduckgo.com/l/") {
        return Ok(url.to_string());
    }
    let query_start = url_str.find('?').ok_or_else(|| anyhow::anyhow!("Invalid redirect URL (no query string): {url}"))?;
    let query = &url_str[query_start + 1..];
    let params: HashMap<&str, &str> = query.split('&')
        .filter_map(|pair| { let mut parts = pair.splitn(2, '='); Some((parts.next()?, parts.next().unwrap_or(""))) })
        .collect();
    match params.get("uddg") {
        Some(encoded) => { let decoded = urlencoding_decode(encoded)?; Ok(decoded) }
        None => Ok(url.to_string()),
    }
}

/// Simple URL percent-decoding (no external dependency needed)
fn urlencoding_decode(input: &str) -> anyhow::Result<String> {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let high = chars.next().ok_or_else(|| anyhow::anyhow!("Truncated percent encoding"))?;
            let low = chars.next().ok_or_else(|| anyhow::anyhow!("Truncated percent encoding"))?;
            let byte = u8::from_str_radix(&format!("{high}{low}"), 16)?;
            result.push(byte as char);
        } else if ch == '+' { result.push(' '); } else { result.push(ch); }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn test_decode_ddg_redirect() {
        let decoded = decode_search_url("//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org%2F&rut=abc").unwrap();
        assert_eq!(decoded, "https://rust-lang.org/");
    }
    #[test] fn test_decode_plain_url() {
        assert_eq!(decode_search_url("https://example.com/").unwrap(), "https://example.com/");
    }
    #[test] fn test_decode_www_ddg_redirect() {
        let decoded = decode_search_url("//www.duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.org%2F").unwrap();
        assert_eq!(decoded, "https://example.org/");
    }
    #[test] fn test_decode_plus_encoded() {
        let decoded = decode_search_url("//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fpath+name").unwrap();
        assert_eq!(decoded, "https://example.com/path name");
    }
    #[test] fn test_decode_urlenc_edge_cases() {
        assert_eq!(urlencoding_decode("hello%20world").unwrap(), "hello world");
        assert!(urlencoding_decode("hello%2").is_err());
    }

    #[test] fn test_html_to_text_simple() {
        let text = html_to_text("<html><body><p>Hello world</p></body></html>");
        assert!(text.contains("Hello")); assert!(!text.contains("<p>"));
    }
    #[test] fn test_html_to_text_multiline() {
        let text = html_to_text("<html><body><h1>Title</h1><p>Paragraph one.</p></body></html>");
        assert!(text.contains("Title")); assert!(text.contains("Paragraph one."));
    }
    #[test] fn test_empty_html() { assert!(html_to_text("").trim().is_empty()); }
    #[test] fn test_html_no_body() { assert!(html_to_text("<html></html>").trim().is_empty()); }
    #[test] fn test_html_only_comments() {
        assert!(html_to_text("<html><body><!-- comment --></body></html>").trim().is_empty());
    }

    #[test] fn test_readable_strips_nav_footer() {
        let html = "\
<html><body>
<nav>Navigation</nav><header>Header</header>
<article><h1>Real Article</h1><p>This is the real content.</p></article>
<aside>Sidebar</aside><footer>Footer</footer>
</body></html>";
        let text = extract_readable_content(&html).unwrap();
        assert!(text.contains("Real Article")); assert!(!text.contains("Navigation"));
        assert!(!text.contains("Footer"));
    }

    

    

    

    

    

    

    #[test] fn test_readable_empty_article() {
        assert!(extract_readable_content("<html><body><article></article></body></html>").unwrap().trim().is_empty());
    }

    #[test] fn test_readable_finds_content_in_div() {
        let html = "\
<html><body>
<div class=\"sidebar\">Sidebar link 1 Sidebar link 2 Sidebar link 3</div>
<div class=\"post-content\">
<h1>My Blog Post Title Here</h1>
<p>This is the first paragraph of the actual blog post content that contains
meaningful information the user wants to read and extract from the page.</p>
<p>Here is another paragraph with more detailed content about the topic being
discussed in this blog post article for the reader to consume.</p>
<p>A third paragraph continues the discussion with even more useful information
for the reader to extract and learn from this blog post content.</p>
<p>A fourth paragraph provides even more substantial content to ensure the
scoring algorithm selects this div over the fallback body text extraction.</p>
</div>
</body></html>";
        let text = extract_readable_content(&html).unwrap();
        assert!(text.contains("My Blog Post Title Here"), "should contain title");
        assert!(!text.contains("Sidebar"), "should NOT contain sidebar text");
    }

    #[test] fn test_readable_selects_paragraph_rich_region() {
        let html = "\
<html><body>
<div class=\"comments\"><p>Nice post!</p><p>Thanks</p></div>
<div class=\"content\">
<p>Real article content paragraph one.</p>
<p>Second paragraph analysis here.</p>
<p>Third paragraph insights here.</p>
<p>Fourth paragraph conclusion.</p>
</div>
</body></html>";
        let text = extract_readable_content(&html).unwrap();
        assert!(text.contains("Real article content"));
        assert!(text.len() > 80);
    }

    #[test] fn test_readable_handles_mixed_page() {
        let html = "\
<html><body>
<nav>Home World Politics</nav>
<header><h1>Breaking News</h1><p>By Jane | June 22, 2026</p></header>
<div class=\"social-share\">Share on Twitter</div>
<div class=\"article-body\">
<p>Scientists announced a groundbreaking discovery in quantum computing.</p>
<p>The discovery demonstrates a new method for maintaining quantum coherence.</p>
<p>\"This is a transformative moment,\" said Dr. Alice Smith.</p>
<p>The research team achieved stability for over 24 hours.</p>
</div>
<aside><h2>Related Articles</h2></aside>
<footer>Copyright 2026</footer>
</body></html>";
        let text = extract_readable_content(&html).unwrap();
        assert!(text.contains("quantum computing")); assert!(text.contains("transformative moment"));
        assert!(!text.contains("Related Articles")); assert!(!text.contains("Share on Twitter"));
    }

    #[test] fn test_html_to_text_with_options() {
        let html = "<html><body><p>Hello    world</p></body></html>";
        let normal = html_to_text_with_options(html, false);
        let compact = html_to_text_with_options(html, true);
        assert_eq!(normal, "Hello world");
        assert_eq!(compact, "Hello world");
    }

    // --- extract_section tests ---

    #[test]
    fn test_extract_section_basic() {
        let html = "<html><body><h3 id=\"Macros\">Macros</h3><p>Macros are powerful.</p><p>They do metaprogramming.</p><h3 id=\"Unsafe\">Unsafe</h3><p>Unsafe code.</p></body></html>";
        let result = extract_section(html, "h3#Macros").unwrap();
        assert!(result.contains("Macros"), "should include heading text");
        assert!(result.contains("Macros are powerful."), "should include content");
        assert!(result.contains("metaprogramming"), "should include all paragraphs");
        assert!(!result.contains("Unsafe"), "should NOT include next section");
    }

    #[test]
    fn test_extract_section_stops_at_higher_heading() {
        let html = "<html><body><h3 id=\"sub\">Sub</h3><p>Content</p><h2 id=\"main\">Main</h2><p>Other</p></body></html>";
        let result = extract_section(html, "h3#sub").unwrap();
        assert!(result.contains("Sub"), "should include heading");
        assert!(result.contains("Content"), "should include content");
        assert!(!result.contains("Main"), "should stop before h2 (higher level)");
    }

    #[test]
    fn test_extract_section_runs_to_end() {
        let html = "<html><body><h2 id=\"last\">Last</h2><p>Final content.</p></body></html>";
        let result = extract_section(html, "h2#last").unwrap();
        assert!(result.contains("Last"));
        assert!(result.contains("Final content"));
    }

    #[test]
    fn test_extract_section_invalid_selector() {
        let html = "<html><body><p>No headings here</p></body></html>";
        let result = extract_section(html, "h3#nonexistent");
        assert!(result.is_err(), "should error on non-matching selector");
    }

    #[test]
    fn test_extract_section_ignores_non_heading() {
        let html = "<html><body><div id=\"not-heading\">Not a heading</div><p>Content</p></body></html>";
        let result = extract_section(html, "div#not-heading");
        assert!(result.is_err(), "should error on non-heading selector");
    }
}
