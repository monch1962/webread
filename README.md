# webread

A single static binary (~2.7 MB) for web content extraction from the CLI.
No browser engine, no JavaScript runtime, no display server.

## Usage

```
webread get <url>                        # Fetch URL, print clean text
webread search <query>                   # Search web, print text results
webread links <url>                      # Enumerate all hrefs on a page (with link text)
webread readable <url>                   # Article extraction (readability mode)
webread html <url> [--selector 'css']    # Raw HTML with optional CSS filter
webread config-check                     # Validate config file

All commands support --json, --timeout, --max-size, --proxy, --user-agent,
--compact, --meta, --outline, --method, and --post-data flags.
```

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

# Get all links as absolute URLs (relative URLs resolved automatically)
webread links https://example.com

# Search results as JSON (pipe to jq for processing)
webread search "rust" --json | jq '.results[] | {title, url}'

# Custom timeout and body size limit for constrained systems
webread get https://en.wikipedia.org/wiki/Rust --timeout 15 --max-size 5000000

# Custom User-Agent
webread get https://api.example.com --user-agent "my-bot/1.0"

# Via HTTP proxy (also respects ALL_PROXY, HTTPS_PROXY, HTTP_PROXY env vars)
webread get https://example.com --proxy http://proxy.corp:8080

# Compact output (token-efficient, -10-30% tokens on large pages)
webread get https://en.wikipedia.org/wiki/Rust --compact

# HEAD request (check status/length without downloading body)
webread get https://example.com --method HEAD

# POST request with body data
webread get https://httpbin.org/post --method POST --post-data "key=value"

# Meta mode (structured page metadata, ~99% token saving)
webread get https://en.wikipedia.org/wiki/Rust --meta

# Meta as JSON (structured data for agentic processing)
webread get https://en.wikipedia.org/wiki/Rust --meta --json

# Outline mode (heading hierarchy only, ~98% token saving)
webread get https://en.wikipedia.org/wiki/Rust --outline

# Outline as JSON
webread get https://en.wikipedia.org/wiki/Rust --outline --json

# Validate config file
webread config-check

# With structured error output for agentic use
WR_JSON_ERROR=1 webread get https://example.com --json
```

## Download

Prebuilt binaries for all platforms are available on the
[Releases page](https://github.com/monch1962/webread/releases).

| Platform | Arch | File |
|----------|------|------|
| Linux | x86_64 | `webread-linux-amd64.tar.gz` |
| Linux | ARM64 | `webread-linux-arm64.tar.gz` |
| macOS | Intel | `webread-macos-amd64.tar.gz` |
| macOS | Apple Silicon | `webread-macos-arm64.tar.gz` |
| Windows | x86_64 | `webread-windows-amd64.zip` |

```bash
# Example: Linux x86_64
curl -L https://github.com/monch1962/webread/releases/latest/download/webread-linux-amd64.tar.gz | tar xz
./webread-linux-amd64/webread --help

# Or install system-wide
sudo cp webread-linux-amd64/webread /usr/local/bin/
```

Each release includes SHA256 checksums for verification.

## Build

```bash
cargo build --release
```

Binary at `target/release/webread` (~2.7 MB stripped, statically linked, no runtime dependencies).

## Architecture

```
src/
├── lib.rs      # Core logic: fetch, extract, score, resolve, search, decode, config
└── main.rs     # CLI entry point with clap subcommands, config file loading
tests/
├── integration.rs  # 31 integration tests (smoke + cross-site + JSON + parallel + URL validation + new features)
└── batch_test.py   # Batch test across 302 websites (parallel, 12 workers)
```

- **HTTP:** `ureq` (synchronous, no async runtime)
- **HTML parsing:** `scraper` (html5ever + CSS selectors)
- **Search:** DuckDuckGo Lite API (no API key needed)
- **Output:** text by default, `--json` for structured output
- **Config:** `~/.config/webread/config` (simple key=value, no extra dependencies)
- **Binary:** 2.7 MB (LTO, opt-level=z, panic=abort, stripped, no `url`/`serde`-derive deps)

## Resource Guardrails

All commands support these safety flags for resource-constrained systems:

| Flag | Default | Purpose |
|------|---------|---------|
| `--timeout <secs>` | 30s | Prevents hanging on slow/dead sites |
| `--max-size <bytes>` | 10 MB | Truncates oversized responses (prevents OOM) |
| `--user-agent <string>` | Safari UA | Override the User-Agent header |
| `--proxy <url>` | env vars | HTTP proxy (e.g. `http://proxy:8080`). Falls back to `ALL_PROXY`/`HTTPS_PROXY`/`HTTP_PROXY` env |
| `--method <method>` | GET | HTTP method: `GET`, `POST`, `HEAD` |
| `--post-data <body>` | — | Body data for POST requests |
| `--compact` | off | Token-efficient output (aggressive whitespace compression) |
| `--meta` | off | Structured metadata mode: title + description + OG tags + canonical + charset + language + link/char count (~99% token saving) |
| `--outline` | off | Heading hierarchy mode: title + h1-h6 tree + link/char count (~98% token saving) |
| `--json` | off | Machine-parseable structured output |

Plus built-in:
- **Content-type check**: skips non-HTML responses (PDFs, images) with a clear error
- **Auto-retry**: one retry on transient errors (502, 503, timeout)
- **URL resolution**: relative links (`/page`, `../other`) resolved to absolute

## Exit Codes (Agentic Use)

Exit codes distinguish error types for programmatic (agent) consumers:

| Code | Meaning | Trigger |
|------|---------|--------|
| 0 | Success | Everything OK |
| 2 | Truncated | Response exceeded `--max-size` |
| 3 | Content-Type not HTML | PDF, image, or other binary response |
| 4 | Network error | DNS failure, connection refused, timeout |
| 5 | Proxy error | Invalid proxy URL or proxy unavailable |
| 6 | HTTP error | 4xx client or 5xx server error |
| 8 | Config/input error | Invalid flags, bad config file, invalid selector |

Set `WR_JSON_ERROR=1` to get machine-parseable error JSON on stdout:

```json
{"error": {"code": "TIMEOUT", "message": "Request timed out", "url": "..."}}
```

Error codes: `TIMEOUT`, `DNS_FAILURE`, `CONNECTION_REFUSED`, `HTTP_4XX`,
`HTTP_5XX`, `CONTENT_TYPE_NOT_HTML`, `TRUNCATED`, `PROXY_ERROR`,
`NETWORK_ERROR`, `INVALID_URL`, `CONFIG_ERROR`, `HTTP_ERROR`

## Configuration File

Create `~/.config/webread/config` with key=value lines:

```
# Default timeout and size limits
timeout=15
max-size=5000000
proxy=http://proxy.corp:8080
user-agent=my-custom-bot/1.0
```

Comments (`#`) and blank lines are ignored. CLI flags override config file values.

## Readability Algorithm

`webread readable` implements a simplified Mozilla Readability algorithm:

1. **Score candidates** by paragraph count and text density
2. **Prefer semantic tags** (`<article>`, `<main>`, `[role=main]`) with 1.3-1.5x bonus
3. **Recognize content classes** like `post-content`, `article-body`, `entry`
4. **Skip non-content classes** like `sidebar`, `comment`, `widget`, `footer`
5. **Strip known non-content elements** (`<nav>`, `<header>`, `<footer>`, `<aside>`, `<script>`, `<style>`)
6. **Fall back** to `<body>` with tag stripping, then to raw text extraction

## Test Suite

```
cargo test        # 104 tests (73 unit + 31 integration, parallel by default)
cargo clippy      # Zero warnings
```

| Suite | Count | Covers |
|-------|-------|--------|
| Unit (lib) | 75 | URL decoding, text extraction, readability scoring, fetch errors, config parsing, URL resolution, content-type checks, retry logic, user-agent override, proxy config, agent building, compact mode, meta mode, outline mode, error codes (13 variants), config validation, HTTP method, link text, search snippets |
| Integration | 39 | CLI smoke tests, JSON output structure, JSON metadata fields, compact mode, meta mode, outline mode, --help agent-discovery (4), HEAD method, links with text, search snippets, cross-site (Wikipedia, GitHub, arXiv, HN, dev.to), parallel stress, URL list validation |

Cross-site integration tests validate against real websites with
`std::thread::spawn` for concurrent execution to verify no shared-state
corruption.

For the full 302-site batch test (parallel, ~2 minutes with 12 workers):
```bash
python3 tests/batch_test.py
```

## Known Limitations

### Sites that require JavaScript
webread fetches raw HTML only — no JavaScript execution:
- **Single-Page Applications** (Reddit, Quora, Medium)
- **Cloudflare JS challenge sites** (Stack Overflow, Server Fault, Ask Ubuntu)

### Sites with paywalls or login walls
- WSJ, Reuters, The Economist, Britannica (HTTP 401/403)
- Some academic publishers (ACM, IEEE)

### Bot-protected sites
webread sends a Safari User-Agent by default (~85% acceptance rate).
The remaining ~15% use bot protection or require authentication.
Use `--user-agent` to try alternate identities. See `tests/batch_test.py`
for the full breakdown of all 302 tested sites.

### Memory
Pages up to `--max-size` (default 10 MB) are loaded into memory.
Typical documentation and article pages use 10-100 KB.

## Comparison with Chrome-based Tools

| Feature | webread | Chrome (browser_navigate) |
|---------|---------|--------------------------|
| Binary size | **2.7 MB** | 200+ MB |
| RAM per call | **5-10 MB** | 200-500 MB |
| Start time | **~5 ms** | 1-5 seconds |
| JS execution | No | **Full** |
| SPA support | Limited | **Full** |
| Readability | **Scoring-based** (simplified Readability) | Mozilla Readability |
| Bot bypass | ~85% | **~99%** |
| JSON output | **Native** (`--json` on every subcommand) | Requires scripting |
| Max body limit | **`--max-size` flag** | None |
| Timeout | **`--timeout` flag** | Configurable |
| Cross-site batch test | **302 sites, 2 min** | Manual |
| Offline | **Yes** | No |

## Hermes Agent Integration

See [HERMES.md](HERMES.md) for detailed instructions on configuring webread
as a lightweight replacement for `browser_navigate`/`browser_snapshot`
in Hermes Agent. Includes tool definitions, prompt patterns, config file
examples, and performance benchmarks for resource-constrained systems.
