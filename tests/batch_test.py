#!/usr/bin/env python3
"""Batch test webread against 100+ websites that AI agents commonly read."""
import json
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "release" / "webread"
TIMEOUT = 15  # seconds per URL

# 120 URLs across categories AI agents commonly read
URLS = [
    # --- Programming Language Docs ---
    "https://docs.python.org/3/tutorial/index.html",
    "https://doc.rust-lang.org/book/",
    "https://developer.mozilla.org/en-US/docs/Web/HTML",
    "https://go.dev/doc/",
    "https://docs.oracle.com/en/java/",
    "https://www.php.net/docs.php",
    "https://docs.npmjs.com/",
    "https://docs.docker.com/",
    "https://kubernetes.io/docs/home/",
    "https://docs.aws.amazon.com/",

    # --- Wikipedia ---
    "https://en.wikipedia.org/wiki/Rust_(programming_language)",
    "https://en.wikipedia.org/wiki/Python_(programming_language)",
    "https://en.wikipedia.org/wiki/Artificial_intelligence",
    "https://en.wikipedia.org/wiki/Large_language_model",
    "https://en.wikipedia.org/wiki/Transformer_(deep_learning_architecture)",
    "https://en.wikipedia.org/wiki/Neural_network",
    "https://en.wikipedia.org/wiki/Machine_learning",
    "https://en.wikipedia.org/wiki/Algorithm",
    "https://en.wikipedia.org/wiki/Data_structure",
    "https://en.wikipedia.org/wiki/World_War_II",

    # --- News ---
    "https://www.bbc.com/news",
    "https://www.reuters.com/",
    "https://apnews.com/",
    "https://www.theguardian.com/international",
    "https://www.nytimes.com/",
    "https://www.wsj.com/",
    "https://www.economist.com/",
    "https://www.aljazeera.com/",
    "https://www.npr.org/",
    "https://www.cnbc.com/",

    # --- Tech News ---
    "https://news.ycombinator.com/",
    "https://arstechnica.com/",
    "https://www.theverge.com/",
    "https://www.wired.com/",
    "https://techcrunch.com/",
    "https://www.zdnet.com/",
    "https://www.theregister.com/",
    "https://www.infoworld.com/",
    "https://dev.to/",
    "https://www.infoq.com/",

    # --- Stack Overflow / Q&A ---
    "https://stackoverflow.com/questions/1/",
    "https://stackoverflow.com/questions/927358/",
    "https://stackoverflow.com/questions/11227809/",
    "https://stackoverflow.com/questions/388242/",
    "https://serverfault.com/",
    "https://superuser.com/",
    "https://askubuntu.com/",
    "https://www.quora.com/",

    # --- Blogs ---
    "https://medium.com/",
    "https://towardsdatascience.com/",
    "https://martinfowler.com/",
    "https://blog.codinghorror.com/",
    "https://www.joelonsoftware.com/",
    "https://karpathy.github.io/",
    "https://simonwillison.net/",
    "https://stratechery.com/",
    "https://apenwarr.ca/log/",
    "https://boringtechnology.club/",

    # --- GitHub / Code Hosting ---
    "https://github.com/rust-lang/rust",
    "https://github.com/python/cpython",
    "https://github.com/torvalds/linux",
    "https://github.com/golang/go",
    "https://github.com/facebook/react",
    "https://github.com/nodejs/node",
    "https://github.com/microsoft/vscode",
    "https://gitlab.com/gitlab-org/gitlab",
    "https://bitbucket.org/",
    "https://sourceforge.net/",

    # --- Academic ---
    "https://arxiv.org/abs/1706.03762",
    "https://arxiv.org/abs/2005.14165",
    "https://arxiv.org/abs/2301.00234",
    "https://scholar.google.com/",
    "https://dl.acm.org/",
    "https://ieeexplore.ieee.org/",
    "https://www.nature.com/",
    "https://www.science.org/",
    "https://www.pnas.org/",
    "https://openreview.net/",

    # --- Reference ---
    "https://www.merriam-webster.com/",
    "https://www.oxfordlearnersdictionaries.com/",
    "https://dictionary.cambridge.org/",
    "https://www.britannica.com/",
    "https://www.encyclopedia.com/",
    "https://mathworld.wolfram.com/",
    "https://plato.stanford.edu/",
    "https://www.cia.gov/the-world-factbook/",
    "https://www.who.int/",
    "https://www.nasa.gov/",

    # --- Social / Forums ---
    "https://www.reddit.com/r/programming/",
    "https://www.reddit.com/r/MachineLearning/",
    "https://lobste.rs/",
    "https://meta.stackexchange.com/",
    "https://discourse.org/",
    "https://www.producthunt.com/",
    "https://news.ycombinator.com/item?id=1",
    "https://www.reddit.com/r/rust/",

    # --- Government / Standards ---
    "https://www.ietf.org/",
    "https://www.w3.org/TR/",
    "https://www.rfc-editor.org/",
    "https://www.iso.org/",
    "https://www.nist.gov/",
    "https://www.gov.uk/",
    "https://www.whitehouse.gov/",
    "https://www.usa.gov/",

    # --- Miscellaneous ---
    "https://example.com/",
    "https://httpbin.org/html",
    "https://httpbin.org/links/10",
    "https://lorem-ipsum.in/",
    "https://www.gutenberg.org/",
    "https://www.oreilly.com/",
    "https://stackoverflow.blog/",
    "https://www.tutorialspoint.com/",
    "https://www.geeksforgeeks.org/",
    "https://www.w3schools.com/",

    # --- Regional / International ---
    "https://www.bbc.co.uk/news/technology",
    "https://www.lemonde.fr/",
    "https://www.spiegel.de/",
    "https://www.dw.com/",
    "https://www.scmp.com/",
    "https://www.japantimes.co.jp/",
    "https://timesofindia.indiatimes.com/",
    "https://www.abc.net.au/news/",

    # --- Additional Programming ---
    "https://crates.io/",
    "https://pypi.org/",
    "https://rubygems.org/",
    "https://www.nuget.org/",
    "https://mvnrepository.com/",
    "https://hex.pm/",
    "https://www.doxygen.nl/",
    "https://www.sphinx-doc.org/",
]

results = {"passed": 0, "failed": 0, "skipped": 0, "failures": []}

def run_test(url: str, subcommand: str, extra_args=None):
    """Run a single webread command and return (success, output, duration)."""
    args = [str(BINARY), subcommand, url]
    if extra_args:
        args.extend(extra_args)
    start = time.time()
    try:
        result = subprocess.run(args, capture_output=True, text=True, timeout=TIMEOUT)
        duration = time.time() - start
        ok = result.returncode == 0 and len(result.stdout.strip()) > 0
        return ok, result.stdout.strip()[:200], duration, result.stderr.strip()[:200]
    except subprocess.TimeoutExpired:
        return False, "", TIMEOUT, "TIMEOUT"
    except FileNotFoundError:
        print(f"ERROR: {BINARY} not found. Build with: cargo build --release")
        sys.exit(1)

def main():
    if not BINARY.exists():
        print("Building release binary...")
        subprocess.run(["cargo", "build", "--release"], cwd=REPO, check=True)
    sz_mb = BINARY.stat().st_size / (1024 * 1024)
    print(f"Binary at {BINARY} ({sz_mb:.1f} MB)")

    print(f"Testing {len(URLS)} URLs against webread...\n")
    print(f"{'#':>4}  {'Subcommand':<12}  {'Status':<8}  {'Time':<6}  {'URL/Output'}")
    print("-" * 80)

    for i, url in enumerate(URLS, 1):
        # Test 'get' for all URLs
        ok, output, duration, err = run_test(url, "get")
        status = "OK" if ok else "FAIL"
        chars = len(output)
        print(f"{i:>4}  {'get':<12}  {status:<8}  {duration:<6.1f}s  {url[:50]:<50}")
        if not ok:
            results["failed"] += 1
            results["failures"].append({
                "url": url, "subcommand": "get",
                "error": err or "empty output",
                "duration": duration
            })
        else:
            results["passed"] += 1

        # Test 'readable' for a subset (news, wikipedia, blogs)
        if any(domain in url for domain in ["wikipedia.org", "bbc.com", "bbc.co.uk",
                "reuters.com", "apnews.com", "theguardian.com", "arstechnica.com",
                "dev.to", "martinfowler.com", "karpathy.github.io",
                "simonwillison.net", "stratechery.com"]):
            ok2, out2, dur2, err2 = run_test(url, "readable")
            status2 = "OK" if ok2 else "FAIL"
            print(f"{'':>4}  {'readable':<12}  {status2:<8}  {dur2:<6.1f}s  (subset test)")
            if not ok2:
                results["failed"] += 1
                results["failures"].append({
                    "url": url, "subcommand": "readable",
                    "error": err2 or "empty output",
                    "duration": dur2
                })
            else:
                results["passed"] += 1

    # --- Summary ---
    print()
    print("=" * 80)
    print(f"RESULTS: {results['passed']} passed, {results['failed']} failed, "
          f"{results['skipped']} skipped "
          f"({len(URLS)} URLs, ~{results['passed'] + results['failed']} tests)")
    print()

    if results["failures"]:
        print("FAILURES:")
        for f in results["failures"]:
            print(f"  [{f['subcommand']}] {f['url']}")
            print(f"    Error: {f['error'][:150]}")
            print(f"    Duration: {f['duration']:.1f}s")
        print()

    # Write results to file
    report = REPO / "target" / "batch-test-report.json"
    with open(report, "w") as fp:
        json.dump(results, fp, indent=2)
    print(f"Report written to {report}")

    return 0 if results["failed"] == 0 else 1

if __name__ == "__main__":
    sys.exit(main())
