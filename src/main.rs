use clap::{Parser, Subcommand};
use scraper::{Html, Selector};

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

fn fetch_url(url: &str) -> anyhow::Result<String> {
    let response = ureq::get(url).call()?;
    let body = response.into_body().read_to_string()?;
    Ok(body)
}

fn cmd_get(url: &str) -> anyhow::Result<()> {
    let html = fetch_url(url)?;
    let doc = Html::parse_document(&html);
    let text = html_to_text(&doc);
    println!("{text}");
    Ok(())
}

fn cmd_html(url: &str, selector: Option<&str>) -> anyhow::Result<()> {
    let html = fetch_url(url)?;
    let doc = Html::parse_document(&html);

    if let Some(sel_str) = selector {
        let sel = Selector::parse(sel_str)
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
    let doc = Html::parse_document(&html);
    let sel = Selector::parse("a").unwrap();

    for element in doc.select(&sel) {
        if let Some(href) = element.value().attr("href") {
            println!("{href}");
        }
    }
    Ok(())
}

fn cmd_readable(url: &str) -> anyhow::Result<()> {
    let html = fetch_url(url)?;
    let doc = Html::parse_document(&html);

    // Try article, main, or body in order of preference
    let content = Selector::parse("article")
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
        });

    if let Some(article) = content {
        let text: Vec<&str> = article.text().collect();
        println!("{}", text.join(" "));
    } else {
        // Fallback: just get all text
        let text = html_to_text(&doc);
        println!("{text}");
    }
    Ok(())
}

fn cmd_search(query: &str) -> anyhow::Result<()> {
    let url = format!("https://lite.duckduckgo.com/lite/");
    let response = ureq::get(&url).query("q", query).call()?;
    let html = response.into_body().read_to_string()?;
    let doc = Html::parse_document(&html);

    let link_sel = Selector::parse("a.result-link").unwrap();
    let snippet_sel = Selector::parse(".result-snippet").unwrap();

    println!("=== Search results for: {query} ===");
    for (i, link) in doc.select(&link_sel).enumerate() {
        if let Some(href) = link.value().attr("href") {
            let title: Vec<&str> = link.text().collect();
            let title = title.join(" ").trim().to_string();
            println!("{}. {title}", i + 1);
            println!("   {href}");

            // Try to find a matching snippet (same result row)
            if let Some(snippet) = doc.select(&snippet_sel).nth(i) {
                let text: Vec<&str> = snippet.text().collect();
                let text = text.join(" ").trim().to_string();
                if !text.is_empty() {
                    println!("   {text}");
                }
            }
            println!();
        }
    }
    Ok(())
}

/// Extract clean text from HTML by walking document tree
fn html_to_text(doc: &Html) -> String {
    use scraper::ElementRef;

    fn collect_text(element: ElementRef) -> String {
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
                    if let Some(child_elem) = ElementRef::wrap(child) {
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

    // Try to find body or just use document root
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
