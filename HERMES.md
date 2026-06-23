# Using webread with Hermes Agent

webread replaces the heavy `browser_navigate`/`browser_snapshot` tool in Hermes
with a lightweight CLI for web content extraction. No browser engine, no JS
runtime, no display server — just raw HTTP and HTML parsing in a single 2.7 MB
static binary.

## Advantages over browser_navigate

| Factor | browser_navigate + browser_snapshot | webread |
|--------|--------------------------------------|---------|
| Binary size | 200+ MB (Chromium) | **2.7 MB** |
| RAM per call | 200-500 MB | **5-10 MB** |
| Startup time | 1-5 seconds | **~5 ms** |
| Dependencies | Chromium, display server, GPU libs | **None (static binary)** |
| JSON output | Post-processing needed | **Native `--json` flag + structured errors** |
| Body size control | None | **`--max-size` flag** |
| Timeout control | Configurable | **`--timeout` flag** |
| Config file | Browser preferences | **`~/.config/webread/config`** |
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

### Option 1: Download prebuilt binary (fastest)

Grab the right binary from the
[Releases page](https://github.com/monch1962/webread/releases),
no compilation needed:

```bash
# Linux x86_64
curl -L https://github.com/monch1962/webread/releases/latest/download/webread-linux-amd64.tar.gz | tar xz
sudo cp webread-linux-amd64/webread /usr/local/bin/

# macOS Apple Silicon
curl -L https://github.com/monch1962/webread/releases/latest/download/webread-macos-arm64.tar.gz | tar xz
sudo cp webread-macos-arm64/webread /usr/local/bin/
```

### Option 2: Build from source

```bash
# Build from source
cd ~/Projects/webread
cargo build --release
sudo cp target/release/webread /usr/local/bin/webread

# Verify
webread --help
webread get https://example.com --json

# Optional: create config file
mkdir -p ~/.config/webread
cat > ~/.config/webread/config << 'EOF'
# Resource limits for constrained systems
timeout=15
max-size=5000000
proxy=http://proxy.corp:8080
EOF
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
    description: Extract article content from a URL (scoring-based readability).
  webread_search:
    command: webread search {{query}} --json
    description: Search the web and return JSON results.
  webread_links:
    command: webread links {{url}}
    description: Enumerate all links on a page (resolved to absolute URLs).
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
Returns: {"url": "...", "text": "...", "char_count": N,
  "final_url": "...", "status": 200, "truncated": false,
  "max_size": 10485760}
```

### Search
```
To search the web: webread search "query" --json
Returns: {"query": "...", "results": [{"title":"...", "url":"...", "snippet":"..."}]}
```

### Safe fetching on constrained systems
```
To fetch with resource limits: webread get <url> --timeout 15 --max-size 1000000
Prevents hanging on slow sites and OOM on giant pages.
```

### Enterprise proxy environments
```
For environments requiring an HTTP proxy:
  webread get <url> --proxy http://proxy.corp:8080
webread also respects ALL_PROXY, HTTPS_PROXY, and HTTP_PROXY environment
variables, as well as NO_PROXY for bypass rules.
```

### Agentic error handling (structured JSON errors)
```
Set WR_JSON_ERROR=1 to get machine-parseable error JSON on failure:
  WR_JSON_ERROR=1 webread get <url> --json
On error, prints: {"error": {"code": "TIMEOUT", "message": "...", "url": "..."}}

Error codes: TIMEOUT, DNS_FAILURE, CONNECTION_REFUSED, HTTP_4XX, HTTP_5XX,
CONTENT_TYPE_NOT_HTML, TRUNCATED, PROXY_ERROR, NETWORK_ERROR, INVALID_URL

Exit codes: 0=ok, 2=truncated, 3=content-type, 4=network, 5=proxy,
6=timeout, 7=http, 8=config/input error
```

### Token-efficient mode
```
For large pages, use --compact to compress whitespace aggressively:
  webread get <url> --compact
Reduces token count by ~10-30% on typical documentation pages.
```

### Meta mode (agentic pre-scanning)
```
For structured page metadata (no body text):
  webread get <url> --meta
Output:
  title: <page title>
  description: <meta description>
  canonical: <canonical URL>
  og:title: <OG title>
  og:description: <OG description>
  og:image: <OG image>
  og:type: <OG type>
  twitter:card: <Twitter card type>
  charset: <character encoding>
  language: <page language>
  json_ld: <JSON-LD structured data>
  links: N  chars: N

JSON: webread get <url> --meta --json
Returns meta_data: {title, description, canonical, og_title, ...}
Saves ~99% tokens vs full extraction.
```

### Outline mode (agentic pre-scanning)
```
For page heading hierarchy (no body text):
  webread get <url> --outline
Output:
  <page title>
  h1: <heading text>
    h2: <subheading>
      h3: <sub-subheading>
  ...

JSON: webread get <url> --outline --json
Returns outline_data: {title, headings[{level, text}], ...}
Saves ~98% tokens vs full extraction.
```

### Extract specific elements
```
To get all links: webread links <url>
To filter HTML: webread html <url> --selector 'h1'
To get links as JSON: webread links <url> --json
```

## Performance Benchmarks

Timings on a single page fetch:

| Tool | Memory | Time | Binary size | Dependencies |
|------|--------|------|-------------|--------------|
| `curl` | ~2 MB | ~0.5s | 0.1 MB | None |
| `webread get` | ~5 MB | ~0.8s | **2.7 MB** | **None (static)** |
| `webread readable` | ~8 MB | ~0.9s | 2.7 MB | None (static) |
| `browser_navigate` | ~250 MB | ~3s | 200+ MB | Chromium, display server |

webread sits between curl and a full browser — it adds HTML parsing,
CSS selector filtering, content scoring, and JSON output but still starts
in milliseconds and uses minimal RAM.

## Testing webread in Hermes

```bash
# Smoke test
webread get https://example.com

# Full test suite (106+ tests)
cd ~/Projects/webread && cargo test

# Batch test across 302 websites (parallel, ~2 minutes)
python3 tests/batch_test.py
```

## Known Limitations

- **No JavaScript execution** — SPAs and JS-rendered sites won't work
- **Bot protection** — ~15% of sites block non-browser HTTP clients (Cloudflare, etc.)
- **Paywalls** — WSJ, Reuters, Economist, etc. return 401/403
- **Memory limit** — `--max-size` (default 10 MB) bounds RAM per request; giant pages are truncated
- **Timeout** — `--timeout` (default 30s) prevents hanging; slow sites return early
