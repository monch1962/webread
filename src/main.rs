use clap::{Parser, Subcommand};
use std::process;
use webread::*;

#[derive(Parser)]
#[command(
    name = "webread",
    about = "Fetch, extract, and search web content from the CLI"
)]
struct Cli {
    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Request timeout in seconds (default: 30)
    #[arg(long, global = true, default_value = "30")]
    timeout: u64,

    /// Maximum response body in bytes (default: 10MB, 0 = unlimited)
    #[arg(long, global = true, default_value = "10485760")]
    max_size: usize,

    /// User-Agent header value (default: Safari on macOS)
    #[arg(long, global = true)]
    user_agent: Option<String>,

    /// Proxy URL (e.g. "http://proxy:8080"). Falls back to ALL_PROXY/HTTPS_PROXY/HTTP_PROXY env.
    #[arg(long, global = true)]
    proxy: Option<String>,

    /// Token-efficient output: aggressive whitespace compression, skip low-value content
    #[arg(long, global = true)]
    compact: bool,

    /// Page outline mode: emit heading hierarchy (h1-h6) only. Cheapest after --meta. Saves ~98% tokens.
    #[arg(long, global = true)]
    outline: bool,

    /// Cheapest mode: emit structured meta tags only (title, description, OG, canonical, language, etc). Saves ~99% tokens. Try this first.
    #[arg(long, global = true)]
    meta: bool,

    /// HTTP method: GET (default), POST, HEAD
    #[arg(long, global = true, default_value = "GET")]
    method: String,

    /// POST body data (required if --method POST)
    #[arg(long, global = true)]
    post_data: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fetch a URL and print clean text
    Get { url: String },
    /// Fetch raw HTML, optionally filtered by CSS selector
    Html {
        url: String,
        #[arg(long)]
        selector: Option<String>,
    },
    /// Enumerate all hrefs on a page (with link text)
    Links { url: String },
    /// Article extraction (readability mode)
    Readable { url: String },
    /// Search the web
    Search { query: String },
    /// Validate the config file and report errors
    ConfigCheck,
}

fn main() -> ! {
    let cli = Cli::parse();

    let cfg = load_config();
    let timeout = cli.timeout;
    let max_size = cli.max_size;
    let user_agent = cli.user_agent.or_else(|| cfg.get("user-agent").cloned());
    let proxy = cli.proxy.or_else(|| cfg.get("proxy").cloned());

    let method = match cli.method.to_uppercase().as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "HEAD" => HttpMethod::Head,
        other => {
            let err = ErrorCode::ConfigError(format!(
                "Invalid HTTP method '{other}'. Use GET, POST, or HEAD."
            ));
            print_error_json(&err, None);
            process::exit(err.exit_code());
        }
    };

    if method == HttpMethod::Post && cli.post_data.is_none() {
        let err = ErrorCode::ConfigError("--post-data is required when --method POST".into());
        print_error_json(&err, None);
        process::exit(err.exit_code());
    }

    let json = cli.json;
    let compact = cli.compact;
    let meta = cli.meta;
    let outline = cli.outline;
    let opts = FetchOptions {
        timeout_secs: timeout,
        max_body_bytes: max_size,
        proxy_url: proxy,
        user_agent,
        method,
        compact,
        meta,
        outline,
        post_body: cli.post_data,
        ..FetchOptions::default()
    };

    if matches!(cli.command, Command::ConfigCheck) {
        let result = cmd_config_check(&cfg);
        process::exit(result);
    }

    let result = match cli.command {
        Command::Get { url } => cmd_get(&url, json, &opts),
        Command::Html { url, selector } => cmd_html(&url, selector.as_deref(), json, &opts),
        Command::Links { url } => cmd_links(&url, json, &opts),
        Command::Readable { url } => cmd_readable(&url, json, &opts),
        Command::Search { query } => cmd_search(&query, json, &opts),
        Command::ConfigCheck => unreachable!(),
    };

    match result {
        Ok(exit) => process::exit(exit),
        Err((exit, error_opt)) => {
            if let Some(err) = error_opt {
                print_error_json(&err, None);
            }
            process::exit(exit);
        }
    }
}

pub fn print_error_json(err: &ErrorCode, url: Option<&str>) {
    if std::env::var("WR_JSON_ERROR").is_ok() || url.is_some() {
        println!(
            "{}",
            serde_json::to_string(&err.to_json(url)).unwrap_or_default()
        );
    }
}

fn fetch_with_opts(
    url: &str,
    opts: &FetchOptions,
) -> Result<(String, FetchResult), (i32, Option<ErrorCode>)> {
    match fetch_url_with(url, opts) {
        Ok(result) => Ok((result.body.clone(), result)),
        Err(e) => {
            let msg = format!("{e:#}");
            let err_code = if msg.contains("timed out") || msg.contains("timeout") {
                Some(ErrorCode::Timeout)
            } else if msg.contains("Content-Type") {
                Some(ErrorCode::ContentTypeNotHtml(msg))
            } else if msg.contains("proxy") || msg.contains("Proxy") {
                Some(ErrorCode::ProxyError(msg))
            } else if msg.contains("503") || msg.contains("502") {
                Some(ErrorCode::Http5xx(0))
            } else {
                Some(ErrorCode::NetworkError(msg))
            };
            let exit = err_code.as_ref().map(|e| e.exit_code()).unwrap_or(1);
            Err((exit, err_code))
        }
    }
}

fn handle_truncated(result: &FetchResult, opts: &FetchOptions) -> i32 {
    if result.truncated {
        eprintln!(
            "[truncated] Response exceeded --max-size ({} bytes). Use --max-size 0 for unlimited.",
            opts.max_body_bytes
        );
        ErrorCode::Truncated.exit_code()
    } else {
        0
    }
}

/// Generic output helper for structured modes (meta, outline).
/// Each type implements both Display and to_json().
fn output_mode<T: std::fmt::Display + ToJson>(
    val: T,
    json: bool,
    result: &FetchResult,
    opts: &FetchOptions,
    url: &str,
    mode: &str,
) -> i32 {
    if json {
        let mut extra = serde_json::json!({});
        add_metadata(&mut extra, result, opts, url);
        let obj = extra.as_object_mut().unwrap();
        obj.insert(format!("{mode}_data"), val.to_json());
        obj.insert(mode.into(), serde_json::Value::Bool(true));
        println!("{extra}");
    } else {
        println!("{val}");
    }
    handle_truncated(result, opts)
}

/// Trait for types that can be serialized to JSON.
pub trait ToJson {
    fn to_json(&self) -> serde_json::Value;
}

// Blanket impl: MetaResult and OutlineResult already implement to_json()
impl ToJson for MetaResult {
    fn to_json(&self) -> serde_json::Value {
        self.to_json()
    }
}
impl ToJson for OutlineResult {
    fn to_json(&self) -> serde_json::Value {
        self.to_json()
    }
}

fn cmd_config_check(cfg: &std::collections::HashMap<String, String>) -> i32 {
    let path = dirs_config_path().join("webread").join("config");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Config file not found at: {}", path.display());
            eprintln!("Create with: mkdir -p ~/.config/webread && echo 'timeout=15' > ~/.config/webread/config");
            return 0;
        }
    };

    let errors = validate_config(&content);
    if errors.is_empty() {
        println!("Config file at {} is valid", path.display());
        println!();
        println!("Current settings:");
        for (key, value) in cfg {
            println!("  {key} = {value}");
        }
        0
    } else {
        eprintln!(
            "Config file at {} has {} issue(s):",
            path.display(),
            errors.len()
        );
        for (key, msg) in &errors {
            eprintln!("  [{key}] {msg}");
        }
        1
    }
}

fn add_metadata(
    extra: &mut serde_json::Value,
    result: &FetchResult,
    opts: &FetchOptions,
    url: &str,
) {
    let obj = extra.as_object_mut().unwrap();
    obj.insert("url".into(), serde_json::Value::String(url.to_string()));
    obj.insert(
        "final_url".into(),
        serde_json::Value::String(result.final_url.clone()),
    );
    obj.insert(
        "status".into(),
        serde_json::Value::Number(serde_json::Number::from(result.status)),
    );
    obj.insert(
        "max_size".into(),
        serde_json::Value::Number(serde_json::Number::from(opts.max_body_bytes as u64)),
    );
    obj.insert(
        "truncated".into(),
        serde_json::Value::Bool(result.truncated),
    );
    if let Some(ref ct) = result.content_type {
        obj.insert("content_type".into(), serde_json::Value::String(ct.clone()));
    }
    if let Some(ra) = result.retry_after {
        obj.insert(
            "retry_after".into(),
            serde_json::Value::Number(serde_json::Number::from(ra)),
        );
    }
    obj.insert("timed_out".into(), serde_json::Value::Bool(false));
    obj.insert("error".into(), serde_json::Value::Null);
}

fn cmd_get(url: &str, json: bool, opts: &FetchOptions) -> Result<i32, (i32, Option<ErrorCode>)> {
    let (html, result) = fetch_with_opts(url, opts)?;

    if opts.meta {
        let m = extract_metadata(&html);
        return Ok(output_mode(m, json, &result, opts, url, "meta"));
    }

    if opts.outline {
        let o = generate_outline(&html);
        return Ok(output_mode(o, json, &result, opts, url, "outline"));
    }

    let text = if opts.compact {
        html_to_text_with_options(&html, true)
    } else {
        html_to_text(&html)
    };

    if json {
        let mut extra = serde_json::json!({});
        add_metadata(&mut extra, &result, opts, url);
        let obj = extra.as_object_mut().unwrap();
        let char_count = text.len();
        obj.insert("text".into(), serde_json::Value::String(text));
        obj.insert(
            "char_count".into(),
            serde_json::Value::Number(serde_json::Number::from(char_count as u64)),
        );
        println!("{extra}");
    } else {
        println!("{text}");
    }

    Ok(handle_truncated(&result, opts))
}

fn cmd_html(
    url: &str,
    selector: Option<&str>,
    json: bool,
    opts: &FetchOptions,
) -> Result<i32, (i32, Option<ErrorCode>)> {
    let (html, result) = fetch_with_opts(url, opts)?;
    let doc = scraper::Html::parse_document(&html);

    if json {
        let selected = if let Some(sel_str) = selector {
            if let Ok(sel) = scraper::Selector::parse(sel_str) {
                doc.select(&sel).map(|e| e.html()).collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        } else {
            vec![html.to_string()]
        };
        let mut extra = serde_json::json!({
            "url": url,
            "final_url": result.final_url,
            "status": result.status,
            "selector": selector,
            "html": selected.join("\n"),
            "match_count": selected.len(),
            "max_size": opts.max_body_bytes,
            "truncated": result.truncated,
        });
        if let Some(ref ct) = result.content_type {
            extra["content_type"] = serde_json::Value::String(ct.clone());
        }
        println!("{extra}");
    } else {
        if let Some(sel_str) = selector {
            let sel = scraper::Selector::parse(sel_str).map_err(|e| {
                let err =
                    ErrorCode::InvalidSelector(format!("Invalid CSS selector '{sel_str}': {e}"));
                (err.exit_code(), Some(err))
            })?;
            for element in doc.select(&sel) {
                println!("{}", element.html());
            }
        } else {
            println!("{html}");
        }
    }

    Ok(handle_truncated(&result, opts))
}

fn cmd_links(url: &str, json: bool, opts: &FetchOptions) -> Result<i32, (i32, Option<ErrorCode>)> {
    let (html, result) = fetch_with_opts(url, opts)?;
    let doc = scraper::Html::parse_document(&html);
    let sel = scraper::Selector::parse("a").unwrap();
    let links: Vec<serde_json::Value> = doc
        .select(&sel)
        .filter_map(|e| {
            let href = e.value().attr("href")?;
            let text: String = e.text().collect::<Vec<_>>().join(" ").trim().to_string();
            Some(serde_json::json!({
                "url": resolve_url(url, href),
                "text": if text.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(text) },
            }))
        })
        .collect();

    if json {
        let mut extra = serde_json::json!({
            "url": url,
            "final_url": result.final_url,
            "status": result.status,
            "links": links,
            "total": links.len(),
            "max_size": opts.max_body_bytes,
            "truncated": result.truncated,
        });
        if let Some(ref ct) = result.content_type {
            extra["content_type"] = serde_json::Value::String(ct.clone());
        }
        println!("{extra}");
    } else {
        for link in &links {
            let url_str = link["url"].as_str().unwrap_or("");
            let text_str = link["text"].as_str().unwrap_or("");
            if text_str.is_empty() {
                println!("{url_str}");
            } else {
                println!("{text_str} -> {url_str}");
            }
        }
    }

    Ok(handle_truncated(&result, opts))
}

fn cmd_readable(
    url: &str,
    json: bool,
    opts: &FetchOptions,
) -> Result<i32, (i32, Option<ErrorCode>)> {
    let (html, result) = fetch_with_opts(url, opts)?;

    if opts.meta {
        let m = extract_metadata(&html);
        return Ok(output_mode(m, json, &result, opts, url, "meta"));
    }

    if opts.outline {
        let o = generate_outline(&html);
        return Ok(output_mode(o, json, &result, opts, url, "outline"));
    }

    let text = extract_readable_content(&html).map_err(|e| {
        let err = ErrorCode::NetworkError(format!("Failed to extract readable content: {e}"));
        (err.exit_code(), Some(err))
    })?;

    if json {
        let mut extra = serde_json::json!({});
        add_metadata(&mut extra, &result, opts, url);
        let obj = extra.as_object_mut().unwrap();
        let char_count = text.len();
        obj.insert("text".into(), serde_json::Value::String(text));
        obj.insert(
            "char_count".into(),
            serde_json::Value::Number(serde_json::Number::from(char_count as u64)),
        );
        println!("{extra}");
    } else {
        println!("{text}");
    }

    Ok(handle_truncated(&result, opts))
}

fn cmd_search(
    query: &str,
    json: bool,
    opts: &FetchOptions,
) -> Result<i32, (i32, Option<ErrorCode>)> {
    let url = "https://lite.duckduckgo.com/lite/";
    let agent = build_agent(opts).map_err(|e| {
        let err = ErrorCode::ProxyError(format!("{e}"));
        (err.exit_code(), Some(err))
    })?;
    let ua = opts.user_agent.as_deref().unwrap_or(DEFAULT_UA);
    let response = match agent
        .get(url)
        .query("q", query)
        .header("User-Agent", ua)
        .call()
    {
        Ok(r) => r,
        Err(e) => {
            let err_code = classify_error(&e);
            return Err((err_code.exit_code(), Some(err_code)));
        }
    };
    let html = match response.into_body().read_to_string() {
        Ok(s) => s,
        Err(e) => {
            let err = ErrorCode::NetworkError(format!("Failed to read search response: {e}"));
            return Err((err.exit_code(), Some(err)));
        }
    };
    let doc = scraper::Html::parse_document(&html);

    let link_sel = scraper::Selector::parse("a.result-link").unwrap();
    let snippet_sel = scraper::Selector::parse(".result-snippet").unwrap();
    let snippets: Vec<String> = doc
        .select(&snippet_sel)
        .map(|s| s.text().collect::<Vec<_>>().join(" ").trim().to_string())
        .collect();

    let results: Vec<serde_json::Value> = doc
        .select(&link_sel)
        .enumerate()
        .filter_map(|(i, link)| {
            let href = link.value().attr("href")?;
            let clean_url = decode_search_url(href).unwrap_or_else(|_| href.to_string());
            let title: String = link.text().collect::<Vec<_>>().join(" ").trim().to_string();
            if title.is_empty() {
                return None;
            }
            let snippet = snippets.get(i).cloned().unwrap_or_default();
            let mut obj = serde_json::json!({
                "title": title,
                "url": clean_url,
            });
            if !snippet.is_empty() {
                obj["snippet"] = serde_json::Value::String(snippet);
            }
            Some(obj)
        })
        .collect();

    if json {
        let output = serde_json::json!({
            "query": query,
            "results": results,
            "total": results.len(),
        });
        println!("{output}");
    } else {
        println!("=== Search results for: {query} ===");
        for (i, result) in results.iter().enumerate() {
            let title = result["title"].as_str().unwrap_or("");
            let url = result["url"].as_str().unwrap_or("");
            println!("{}. {title}", i + 1);
            println!("   {url}");
            if let Some(snippet) = result.get("snippet").and_then(|s| s.as_str()) {
                if !snippet.is_empty() {
                    println!("   {snippet}");
                }
            }
            println!();
        }
    }
    Ok(0)
}
