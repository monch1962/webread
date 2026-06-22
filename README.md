# webread

A single static binary (~2.6 MB) for web content extraction from the CLI.
No browser engine, no JavaScript runtime, no display server.

## Usage

```
webread get <url>                        # Fetch URL, print clean text
webread search <query>                   # Search web, print text results
webread links <url>                      # Enumerate all hrefs on a page
webread readable <url>                   # Article extraction (readability mode)
webread html <url> [--selector 'css']    # Raw HTML with optional CSS filter
```

All commands support `--json` for structured output.

## Examples

```bash
# Fetch a page as clean text
webread get https://example.com

# Search the web
webread search "rust programming language"

# Extract article content (strips nav, headers, footers, sidebars)
webread readable 'https://en.wikipedia.org/wiki/Rust_(programming_language)'

# Filter HTML by CSS selector
webread html https://example.com --selector 'h1'

# Get all links as JSON
webread links https://example.com --json

# Search results as JSON (pipe to jq for processing)
webread search "rust" --json | jq '.results[] | {title, url}'
```

## Build

```bash
cargo build --release
```

Binary at `target/release/webread` (~2.6 MB stripped, no additional tools needed).

## Architecture

```
src/
├── lib.rs      # Core logic: fetch_url, html_to_text, extract_readable_content, decode_search_url
└── main.rs     # CLI entry point with clap subcommands
tests/
├── integration.rs  # 22 integration tests (smoke + cross-site + JSON structure)
└── batch_test.py   # Batch test across 130+ websites
```

- **HTTP:** `ureq` (synchronous, no async runtime)
- **HTML parsing:** `scraper` (html5ever + CSS selectors)
- **Search:** DuckDuckGo Lite API (no API key needed)
- **Output:** text by default, `--json` for structured output
- **Binary:** 2.6 MB (LTO, opt-level=z, stripped, no unwind tables)

## Test Suite

```
cargo test        # 30 tests (18 unit + 22 integration)
cargo clippy      # Zero warnings
```

Cross-site tests validate webread against real websites: Wikipedia, GitHub,
arXiv, Hacker News, dev.to, and example.com.

For the full 130+ site batch test:
```bash
python3 tests/batch_test.py
```

## Known Limitations

### Sites that require JavaScript
webread fetches raw HTML only — it does not execute JavaScript. The following
types of sites will produce empty or minimal output:

- **Single-Page Applications** (Reddit, Quora, Medium)
- **Sites behind Cloudflare JS challenges** (Stack Overflow, Server Fault, Super User)

### Sites with paywalls or login walls
Sites that require authentication or subscription return HTTP 401/403:

- WSJ, Reuters, The Economist, Britannica (paywalls)
- Some academic publishers (ACM, IEEE — some content accessible)

### Bot-protected sites
Some sites aggressively block non-browser HTTP clients regardless of
User-Agent. webread sends a Safari User-Agent by default, which is accepted
by the majority of sites.

### Large pages
Very large pages (100KB+) are fetched entirely into memory before processing.
This is acceptable for most documentation and article pages.

## Comparison with Chrome-based tools

| Feature | webread | Chrome (browser_navigate) |
|---------|---------|--------------------------|
| Binary size | 2.6 MB | 200+ MB (browser engine) |
| RAM usage | ~5-10 MB | 200-500 MB+ |
| Start time | ~5 ms | 1-5 seconds |
| JS execution | No | Yes |
| SPA support | Limited | Full |
| Bot detection bypass | ~85% of sites | ~99% of sites |
| Offline | Yes | No |
| JSON output | Native | Requires scripting |
