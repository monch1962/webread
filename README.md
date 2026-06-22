# webread

A single static binary (~2.7 MB) for web content extraction from the CLI.
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

# Extract article content (scoring-based readability algorithm)
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

Binary at `target/release/webread` (~2.6 MB stripped, no dependencies).

## Architecture

```
src/
├── lib.rs      # Core logic: fetch, extract, score, search, decode
└── main.rs     # CLI entry point with clap subcommands
tests/
├── integration.rs  # 22 integration tests (smoke + cross-site + JSON)
└── batch_test.py   # Batch test across 130+ websites
```

- **HTTP:** `ureq` (synchronous, no async runtime)
- **HTML parsing:** `scraper` (html5ever + CSS selectors)
- **Search:** DuckDuckGo Lite API (no API key needed)
- **Output:** text by default, `--json` for structured output
- **Binary:** 2.7 MB (LTO, opt-level=z, panic=abort, stripped)

## Readability Algorithm

`webread readable` implements a simplified Mozilla Readability algorithm:

1. **Score candidates** by paragraph count and text density
2. **Prefer semantic tags** (`<article>`, `<main>`, `[role=main]`) with 1.3-1.5x bonus
3. **Recognize content classes** like `post-content`, `article-body`, `entry`
4. **Skip non-content classes** like `sidebar`, `comment`, `widget`, `footer`
5. **Strip known non-content elements** (`<nav>`, `<header>`, `<footer>`, `<aside>`, `<script>`, `<style>`)
6. **Fall back** to `<body>` with tag stripping, then to raw text

This handles sites without semantic HTML tags (e.g., `<div class="post-content">`),
selects the most paragraph-rich region, and excludes sidebars and navigation.

## Test Suite

```
cargo test        # 45 tests (21 unit + 24 integration, parallel by default)
cargo clippy      # Zero warnings
```

Cross-site integration tests validate against real websites:
Wikipedia, GitHub, arXiv, Hacker News, dev.to, example.com.
Integration tests use `std::thread::spawn` for concurrent subcommand
execution to verify no shared-state corruption.

For the full 130+ site batch test (parallel, ~40 seconds with 12 workers):
```bash
python3 tests/batch_test.py
```

## Known Limitations

### Sites that require JavaScript
webread fetches raw HTML only — no JavaScript execution:
- **Single-Page Applications** (Reddit, Quora, Medium)
- **Cloudflare JS challenge sites** (Stack Overflow, Server Fault)

### Sites with paywalls or login walls
- WSJ, Reuters, The Economist, Britannica (HTTP 401/403)
- Some academic publishers (ACM, IEEE)

### Bot-protected sites
webread sends a Safari User-Agent. ~85% of sites accept this.
The remaining ~15% use bot protection (Cloudflare, DataDome) or
require authentication. See `tests/batch_test.py` for the full breakdown.

### Memory
Very large pages (100KB+) are loaded entirely into memory.
Acceptable for documentation, articles, and typical web pages.

## Comparison with Chrome-based Tools

| Feature | webread | Chrome (browser_navigate) |
|---------|---------|--------------------------|
| Binary size | 2.6 MB | 200+ MB |
| RAM usage | ~5-10 MB | 200-500 MB |
| Start time | ~5 ms | 1-5 seconds |
| JS execution | No | Yes |
| SPA support | Limited | Full |
| Readability | Scoring-based | Mozilla Readability |
| Bot detection bypass | ~85% | ~99% |
| JSON output | Native | Requires scripting |
| Offline | Yes | No |

## Hermes Agent Integration

See [HERMES.md](HERMES.md) for detailed instructions on configuring webread
as a lightweight replacement for `browser_navigate`/`browser_snapshot`
in Hermes Agent. Includes tool definitions, prompt patterns, and
performance benchmarks.
