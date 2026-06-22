use clap::{Parser, Subcommand};
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
    /// Enumerate all hrefs on a page
    Links { url: String },
    /// Article extraction (readability mode)
    Readable { url: String },
    /// Search the web
    Search { query: String },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let json = cli.json;
    let opts = FetchOptions {
        timeout_secs: cli.timeout,
        max_body_bytes: cli.max_size,
        ..FetchOptions::default()
    };
    match cli.command {
        Command::Get { url } => cmd_get(&url, json, &opts),
        Command::Html { url, selector } => cmd_html(&url, selector.as_deref(), json, &opts),
        Command::Links { url } => cmd_links(&url, json, &opts),
        Command::Readable { url } => cmd_readable(&url, json, &opts),
        Command::Search { query } => cmd_search(&query, json),
    }
}

fn fetch_with_opts(url: &str, opts: &FetchOptions) -> anyhow::Result<String> {
    let result = fetch_url_with(url, opts)?;
    Ok(result.body)
}

fn cmd_get(url: &str, json: bool, opts: &FetchOptions) -> anyhow::Result<()> {
    let html = fetch_with_opts(url, opts)?;
    let text = html_to_text(&html);
    if json {
        let output = serde_json::json!({
            "url": url,
            "text": text,
            "char_count": text.len(),
        });
        println!("{output}");
    } else {
        println!("{text}");
    }
    Ok(())
}

fn cmd_html(
    url: &str,
    selector: Option<&str>,
    json: bool,
    opts: &FetchOptions,
) -> anyhow::Result<()> {
    let html = fetch_with_opts(url, opts)?;
    let doc = scraper::Html::parse_document(&html);

    if json {
        let selected = if let Some(sel_str) = selector {
            if let Ok(sel) = scraper::Selector::parse(sel_str) {
                let fragments: Vec<String> = doc.select(&sel).map(|e| e.html()).collect();
                fragments
            } else {
                Vec::new()
            }
        } else {
            vec![html.to_string()]
        };
        let output = serde_json::json!({
            "url": url,
            "selector": selector,
            "html": selected.join("\n"),
            "match_count": selected.len(),
        });
        println!("{output}");
    } else {
        if let Some(sel_str) = selector {
            let sel = scraper::Selector::parse(sel_str)
                .map_err(|e| anyhow::anyhow!("Invalid CSS selector '{sel_str}': {e}"))?;
            for element in doc.select(&sel) {
                println!("{}", element.html());
            }
        } else {
            println!("{html}");
        }
    }
    Ok(())
}

fn cmd_links(url: &str, json: bool, opts: &FetchOptions) -> anyhow::Result<()> {
    let html = fetch_with_opts(url, opts)?;
    let doc = scraper::Html::parse_document(&html);
    let sel = scraper::Selector::parse("a").unwrap();
    let links: Vec<String> = doc
        .select(&sel)
        .filter_map(|e| e.value().attr("href"))
        .map(|h| resolve_url(url, h))
        .collect();

    if json {
        let output = serde_json::json!({
            "url": url,
            "links": links,
            "total": links.len(),
        });
        println!("{output}");
    } else {
        for href in &links {
            println!("{href}");
        }
    }
    Ok(())
}

fn cmd_readable(url: &str, json: bool, opts: &FetchOptions) -> anyhow::Result<()> {
    let html = fetch_with_opts(url, opts)?;
    let text = extract_readable_content(&html)?;
    if json {
        let output = serde_json::json!({
            "url": url,
            "text": text,
            "char_count": text.len(),
        });
        println!("{output}");
    } else {
        println!("{text}");
    }
    Ok(())
}

fn cmd_search(query: &str, json: bool) -> anyhow::Result<()> {
    let url = "https://lite.duckduckgo.com/lite/";
    let response = ureq::get(url)
        .query("q", query)
        .header("User-Agent", &webread::user_agent())
        .call()?;
    let html = response.into_body().read_to_string()?;
    let doc = scraper::Html::parse_document(&html);

    let link_sel = scraper::Selector::parse("a.result-link").unwrap();
    let snippet_sel = scraper::Selector::parse(".result-snippet").unwrap();

    let results: Vec<serde_json::Value> = doc
        .select(&link_sel)
        .enumerate()
        .filter_map(|(i, link)| {
            let href = link.value().attr("href")?;
            let clean_url = decode_search_url(href).unwrap_or_else(|_| href.to_string());
            let title: String = link.text().collect::<Vec<_>>().join(" ").trim().to_string();
            let snippet = doc
                .select(&snippet_sel)
                .nth(i)
                .map(|s| s.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .unwrap_or_default();
            Some(serde_json::json!({
                "title": title,
                "url": clean_url,
                "snippet": snippet,
            }))
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
            let snippet = result["snippet"].as_str().unwrap_or("");
            println!("{}. {title}", i + 1);
            println!("   {url}");
            if !snippet.is_empty() {
                println!("   {snippet}");
            }
            println!();
        }
    }
    Ok(())
}
