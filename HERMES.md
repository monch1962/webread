# Using webread with Hermes Agent

webread replaces the heavy `browser_navigate`/`browser_snapshot` tool in Hermes
with a lightweight CLI for web content extraction. No browser engine, no JS
runtime, no display server — just raw HTTP and HTML parsing in a single 2.7 MB
static binary.

## Advantages over browser_navigate

| Factor | browser_navigate + browser_snapshot | webread |
|--------|--------------------------------------|---------|
| Binary size | 200+ MB (Chromium) | 2.6 MB |
| RAM per call | 200-500 MB | 5-10 MB |
| Startup time | 1-5 seconds | ~5 ms |
| Dependencies | Chromium, display server | None (static binary) |
| JSON output | Post-processing needed | Native `--json` flag |
| JS execution | Full | None (intentional) |

## When to use webread vs browser

**Use webread for:**
- Documentation pages (MDN, docs.python.org, readthedocs)
- Wikipedia articles and knowledge bases
- News articles and blog posts
- GitHub READMEs and source listings
- arXiv papers and academic abstracts
- Package registries (crates.io, PyPI, npm)
- HTTP APIs that return HTML
- Any static HTML page

**Use browser for:**
- Single-page apps rendered by JavaScript (Reddit, Twitter/X)
- Sites behind Cloudflare JS challenges
- Sites requiring interactive login flows
- Pages that dynamically load content after initial HTML

## Installing webread in Hermes

### Option 1: Installed system-wide

```bash
# Build from source
cd ~/Projects/webread
cargo build --release
strip target/release/webread
sudo cp target/release/webread /usr/local/bin/webread

# Verify
webread --help
webread get https://example.com --json
```

### Option 2: Use directly from the build directory

```bash
export PATH="$HOME/Projects/webread/target/release:$PATH"
```

## Configuring as a Hermes Tool

Add a custom tool definition in Hermes' configuration to make webread
available to agents. In `~/.hermes/config.yaml`:

```yaml
tools:
  webread:
    command: webread
    description: >
      Fetch and extract web content without a browser.
      Supports: get (clean text), readable (article mode),
      html (raw + CSS selector), links (href enumeration),
      search (DuckDuckGo). All support --json flag.
```

Or as individual tools for finer-grained control:

```yaml
tools:
  webread_get:
    command: webread get {{url}}
    description: Fetch a URL and return clean text.
  webread_readable:
    command: webread readable {{url}}
    description: Extract article content from a URL.
  webread_search:
    command: webread search {{query}} --json
    description: Search the web and return JSON results.
  webread_links:
    command: webread links {{url}}
    description: Enumerate all links on a page.
  webread_html:
    command: webread html {{url}} --selector {{selector}}
    description: Fetch HTML filtered by CSS selector.
```

## Usage Examples for Prompts

When instructing an AI agent to use webread, use these patterns:

### Fetch clean text
```
To read a page, use: webread get <url>
Example output: clean text with all HTML stripped
```

### Article extraction
```
To extract an article without navigation/ads: webread readable <url>
Strips: nav, header, footer, aside, script, style
Uses content scoring to find the main article body
```

### Structured data via JSON
```
For machine-parseable output: webread get <url> --json
Returns: {"url": "...", "text": "...", "char_count": N}
```

### Search
```
To search the web: webread search "query" --json
Returns: {"query": "...", "results": [{"title":"...", "url":"...", "snippet":"..."}]}
```

### Extract specific elements
```
To get all links: webread links <url>
To filter HTML: webread html <url> --selector 'h1'
To get links as JSON: webread links <url> --json
```

## Performance Benchmarks

Timings on a single page fetch:

| Tool | Memory | Time | Binary size |
|------|--------|------|-------------|
| `curl` | ~2 MB | ~0.5s | 0.1 MB |
| `webread get` | ~5 MB | ~0.8s | 2.6 MB |
| `webread readable` | ~8 MB | ~0.9s | 2.6 MB |
| `browser_navigate` | ~250 MB | ~3s | 200+ MB |

webread sits between curl and a full browser — it adds HTML parsing and
content extraction but still starts in milliseconds.

## Testing webread in Hermes

```bash
# Smoke test: fetch a known URL
webread get https://example.com

# Full test suite
cd ~/Projects/webread && cargo test

# Batch test across 302 websites
python3 tests/batch_test.py
```

## Known Limitations

- No JavaScript execution — SPAs and JS-rendered sites won't work
- Some sites block non-browser HTTP clients (Cloudflare, etc.) — ~15% failure rate
- Paywalled sites return 401/403
- Very large pages (>1 MB) loaded entirely into memory
