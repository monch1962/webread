# webread

A single static binary (~4 MB) for web content extraction from the CLI. No browser engine, no JavaScript runtime, no display server.

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
webread readable https://en.wikipedia.org/wiki/Rust_(programming_language)

# Filter HTML by CSS selector
webread html https://example.com --selector 'h1'

# Get all links as JSON
webread links https://example.com --json

# Search results as JSON
webread search "rust" --json | jq '.results[] | {title, url}'
```

## Build

```bash
cargo build --release
strip target/release/webread   # optional, reduces size
```

Binary lives at `target/release/webread` (~4.2 MB stripped).

## Architecture

```
src/
├── lib.rs      # Core logic: fetch_url, html_to_text, extract_readable_content, decode_search_url
├── main.rs     # CLI entry point with clap subcommands
tests/
└── integration.rs  # Integration tests (11 tests)
```

- **HTTP:** ureq (minimal, no async runtime)
- **HTML parsing:** scraper (CSS selector support)
- **Search:** DuckDuckGo Lite API (no API key needed)
- **Output:** text by default, `--json` for structured output
