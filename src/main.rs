use clap::{Parser, Subcommand};
use webread::*;

#[derive(Parser)]
#[command(
    name = "webread",
    about = "Fetch, extract, and search web content from the CLI"
)]
struct Cli {
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
    match cli.command {
        Command::Get { url } => cmd_get(&url),
        Command::Html { url, selector } => cmd_html(&url, selector.as_deref()),
        Command::Links { url } => cmd_links(&url),
        Command::Readable { url } => cmd_readable(&url),
        Command::Search { query } => cmd_search(&query),
    }
}

fn cmd_get(url: &str) -> anyhow::Result<()> {
    let html = fetch_url(url)?;
    let text = html_to_text(&html);
    println!("{text}");
    Ok(())
}

fn cmd_html(url: &str, selector: Option<&str>) -> anyhow::Result<()> {
    let html = fetch_url(url)?;
    let doc = scraper::Html::parse_document(&html);

    if let Some(sel_str) = selector {
        let sel = scraper::Selector::parse(sel_str)
            .map_err(|e| anyhow::anyhow!("Invalid CSS selector '{sel_str}': {e}"))?;
        for element in doc.select(&sel) {
            println!("{}", element.html());
        }
    } else {
        println!("{html}");
    }
    Ok(())
}

fn cmd_links(url: &str) -> anyhow::Result<()> {
    let html = fetch_url(url)?;
    let doc = scraper::Html::parse_document(&html);
    let sel = scraper::Selector::parse("a").unwrap();

    for element in doc.select(&sel) {
        if let Some(href) = element.value().attr("href") {
            println!("{href}");
        }
    }
    Ok(())
}

fn cmd_readable(url: &str) -> anyhow::Result<()> {
    let html = fetch_url(url)?;
    let text = extract_readable_content(&html)?;
    println!("{text}");
    Ok(())
}

fn cmd_search(query: &str) -> anyhow::Result<()> {
    let url = "https://lite.duckduckgo.com/lite/";
    let response = ureq::get(url).query("q", query).call()?;
    let html = response.into_body().read_to_string()?;
    let doc = scraper::Html::parse_document(&html);

    let link_sel = scraper::Selector::parse("a.result-link").unwrap();

    println!("=== Search results for: {query} ===");
    for (i, link) in doc.select(&link_sel).enumerate() {
        if let Some(href) = link.value().attr("href") {
            // Decode DuckDuckGo redirect URLs
            let clean_url = decode_search_url(href).unwrap_or_else(|_| href.to_string());
            let title: Vec<&str> = link.text().collect();
            let title = title.join(" ").trim().to_string();
            println!("{}. {title}", i + 1);
            println!("   {clean_url}");
            println!();
        }
    }
    Ok(())
}
