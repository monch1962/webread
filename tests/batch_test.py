#!/usr/bin/env python3
"""Batch test webread against 120+ websites, running tests in parallel."""
import json
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "release" / "webread"
TIMEOUT = 20  # seconds per URL
MAX_WORKERS = 12  # parallel HTTP workers

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

    # --- Q&A ---
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

    # --- GitHub / Code ---
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
    "https://lobste.rs/",
    "https://meta.stackexchange.com/",
    "https://discourse.org/",
    "https://www.producthunt.com/",

    # --- Standards / Government ---
    "https://www.ietf.org/",
    "https://www.w3.org/TR/",
    "https://www.rfc-editor.org/",
    "https://www.iso.org/",
    "https://www.nist.gov/",
    "https://www.gov.uk/",
    "https://www.whitehouse.gov/",
    "https://www.usa.gov/",

    # --- Reference / Educational ---
    "https://example.com/",
    "https://lorem-ipsum.in/",
    "https://www.gutenberg.org/",
    "https://www.oreilly.com/",
    "https://stackoverflow.blog/",
    "https://www.tutorialspoint.com/",
    "https://www.geeksforgeeks.org/",
    "https://www.w3schools.com/",

    # --- International ---
    "https://www.bbc.co.uk/news/technology",
    "https://www.lemonde.fr/",
    "https://www.spiegel.de/",
    "https://www.dw.com/",
    "https://www.scmp.com/",
    "https://www.japantimes.co.jp/",
    "https://timesofindia.indiatimes.com/",
    "https://www.abc.net.au/news/",

    # --- Package Registries ---
    "https://crates.io/",
    "https://pypi.org/",
    "https://rubygems.org/",
    "https://www.nuget.org/",
    "https://mvnrepository.com/",
    "https://hex.pm/",
]

# Subset that gets the readable test too
READABLE_DOMAINS = [
    "wikipedia.org", "bbc.com", "bbc.co.uk", "reuters.com", "apnews.com",
    "theguardian.com", "arstechnica.com", "dev.to", "martinfowler.com",
    "karpathy.github.io", "simonwillison.net", "stratechery.com",
]


def run_webread(subcommand: str, url: str) -> dict:
    """Run a single webread command. Returns result dict."""
    args = [str(BINARY), subcommand, url]
    start = time.time()
    try:
        result = subprocess.run(
            args, capture_output=True, text=True, timeout=TIMEOUT
        )
        duration = time.time() - start
        ok = result.returncode == 0 and len(result.stdout.strip()) > 0
        return {
            "subcommand": subcommand,
            "url": url,
            "ok": ok,
            "output": result.stdout.strip()[:500],
            "error": result.stderr.strip()[:200] if not ok else "",
            "duration": round(duration, 2),
        }
    except subprocess.TimeoutExpired:
        return {
            "subcommand": subcommand,
            "url": url,
            "ok": False,
            "output": "",
            "error": "TIMEOUT",
            "duration": TIMEOUT,
        }
    except FileNotFoundError:
        print(f"ERROR: {BINARY} not found. Build with: cargo build --release")
        sys.exit(1)


def main():
    if not BINARY.exists():
        print("Building release binary...")
        subprocess.run(["cargo", "build", "--release"], cwd=REPO, check=True)
    sz_mb = BINARY.stat().st_size / (1024 * 1024)
    print(f"Binary: {BINARY} ({sz_mb:.1f} MB), workers={MAX_WORKERS}")

    # Build task list: get for every URL, readable for subset
    tasks = []
    for url in URLS:
        tasks.append(("get", url))
        if any(domain in url for domain in READABLE_DOMAINS):
            tasks.append(("readable", url))

    total = len(tasks)
    print(f"Running {total} tests across {len(URLS)} URLs ({MAX_WORKERS} workers)...\n")

    passed = 0
    failed = 0
    failures = []
    completed = 0
    start_wall = time.time()

    with ThreadPoolExecutor(max_workers=MAX_WORKERS) as pool:
        futures = {
            pool.submit(run_webread, subcmd, url): (subcmd, url)
            for subcmd, url in tasks
        }
        for future in as_completed(futures):
            subcmd, url = futures[future]
            result = future.result()
            completed += 1
            status = "OK" if result["ok"] else "FAIL"
            if result["ok"]:
                passed += 1
            else:
                failed += 1
                failures.append(result)

            # Print live progress (compact: one line per test)
            elapsed = time.time() - start_wall
            eta = (elapsed / completed) * (total - completed) if completed > 0 else 0
            print(
                f"[{completed:>3}/{total}] {status:<4} {subcmd:<8} "
                f"{result['duration']:>5.1f}s  {url[:60]}  "
                f"(ETA: {eta:.0f}s)",
                flush=True,
            )

    # --- Summary ---
    elapsed = time.time() - start_wall
    print()
    print("=" * 80)
    print(f"RESULTS: {passed} passed, {failed} failed "
          f"({total} tests, {len(URLS)} URLs)")
    print(f"Wall time: {elapsed:.0f}s "
          f"(sequential estimate: ~{elapsed * MAX_WORKERS:.0f}s)")
    print()

    if failures:
        print("FAILURES:")
        for f in failures:
            print(f"  [{f['subcommand']}] {f['url']}")
            print(f"    Error: {f['error'][:150]}")
        print()

    report = REPO / "target" / "batch-test-report.json"
    with open(report, "w") as fp:
        json.dump({
            "passed": passed,
            "failed": failed,
            "total": total,
            "wall_time_s": round(elapsed, 1),
            "workers": MAX_WORKERS,
            "failures": failures,
        }, fp, indent=2)
    print(f"Report: {report}")

    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
