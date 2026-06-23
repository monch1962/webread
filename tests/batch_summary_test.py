#!/usr/bin/env python3
"""Batch test webread --summary mode against 300+ websites."""
import json
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "release" / "webread"
TIMEOUT = 20
MAX_WORKERS = 12

# Load URLs from batch_urls.txt or batch_test.py
url_file = REPO / "tests" / "batch_urls.txt"
if url_file.exists():
    URLS = [l.strip() for l in open(url_file) if l.strip() and not l.strip().startswith("#")]
else:
    import re
    with open(REPO / "tests" / "batch_test.py") as f:
        URLS = re.findall(r'"https?://[^"]+"', f.read())
extra = ["https://example.com", "https://httpstat.us/200"]
all_urls = list(dict.fromkeys(URLS + extra))


def test_summary(url):
    start = time.time()
    try:
        r = subprocess.run(
            [str(BINARY), "get", url, "--summary", "--json"],
            capture_output=True, text=True, timeout=TIMEOUT,
        )
        elapsed = time.time() - start
        if r.returncode != 0:
            return {"url": url, "status": "fail", "reason": f"exit={r.returncode}", "elapsed": elapsed}
        data = json.loads(r.stdout)
        sd = data.get("summary_data", {})
        if not sd:
            return {"url": url, "status": "fail", "reason": "no summary_data", "elapsed": elapsed}
        preview = sd.get("preview", "")
        issues = []
        if not preview:
            issues.append("empty preview")
        return {"url": url, "status": "pass" if not issues else "warn", "issues": issues,
                "has_title": bool(sd.get("title")), "sections": len(sd.get("sections", [])),
                "links": sd.get("link_count", -1), "total_chars": sd.get("total_chars", -1),
                "preview_len": len(preview), "elapsed": elapsed}
    except subprocess.TimeoutExpired:
        return {"url": url, "status": "fail", "reason": "timeout", "elapsed": TIMEOUT}
    except Exception as e:
        return {"url": url, "status": "fail", "reason": str(e), "elapsed": 0}


def main():
    if not BINARY.exists():
        print(f"ERROR: Build release binary first: cargo build --release")
        sys.exit(1)
    results = {"pass": 0, "warn": 0, "fail": 0, "errors": [], "stats": {"title_ok": 0, "sections": 0, "links": 0}}
    print(f"Testing {len(all_urls)} URLs with --summary (parallel={MAX_WORKERS})")
    start_total = time.time()
    with ThreadPoolExecutor(max_workers=MAX_WORKERS) as pool:
        fut_map = {pool.submit(test_summary, url): url for url in all_urls}
        done = 0
        for fut in as_completed(fut_map):
            done += 1
            r = fut.result()
            results[r["status"]] += 1
            if r["status"] == "pass":
                results["stats"]["title_ok"] += 1 if r.get("has_title") else 0
                results["stats"]["sections"] += 1 if r.get("sections", 0) > 0 else 0
                results["stats"]["links"] += 1 if r.get("links", 0) > 0 else 0
            if r["status"] != "pass":
                results["errors"].append(r)
            issues = f" [{','.join(r.get('issues', []))}]" if r.get("issues") else ""
            print(f"  [{done}/{len(all_urls)}] {r['status'].upper()} {r['url'][:65]:65s} "
                  f"t={'Y' if r.get('has_title') else 'N'} "
                  f"s={r.get('sections',0)} l={r.get('links',0)} "
                  f"p={r.get('preview_len',0)}c c={r.get('total_chars',0)}c "
                  f"{r.get('elapsed',0):.1f}s{issues}")
    elapsed = time.time() - start_total
    pct = results["pass"] / len(all_urls) * 100
    print(f"\n{'='*60}")
    print(f"RESULTS: {results['pass']}/{len(all_urls)} passed ({pct:.0f}%)")
    print(f"  Pass: {results['pass']}, Warn: {results['warn']}, Fail: {results['fail']}")
    print(f"  Title: {results['stats']['title_ok']}, Sections: {results['stats']['sections']}, Links: {results['stats']['links']}")
    print(f"  Time: {elapsed:.0f}s")
    if results["errors"]:
        print(f"\nFAILURES:")
        for e in results["errors"]:
            reason = e.get("reason") or "; ".join(e.get("issues", []))
            print(f"  {e['url']}: {reason}")
    return 0 if results["fail"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
