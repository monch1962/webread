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
    # --- Programming Language Docs (20) ---
    "https://docs.python.org/3/tutorial/index.html",
    "https://doc.rust-lang.org/book/",
    "https://developer.mozilla.org/en-US/docs/Web/HTML",
    "https://go.dev/doc/",
    "https://docs.oracle.com/en/java/",
    "https://www.php.net/docs.php",
    "https://docs.npmjs.com/",
    "https://www.typescriptlang.org/docs/",
    "https://kotlinlang.org/docs/home.html",
    "https://docs.swift.org/swift-book/",
    "https://www.ruby-lang.org/en/documentation/",
    "https://docs.scala-lang.org/",
    "https://www.r-project.org/",
    "https://julialang.org/documentation/",
    "https://elixir-lang.org/docs.html",
    "https://www.haskell.org/documentation/",
    "https://perldoc.perl.org/",
    "https://docs.microsoft.com/en-us/dotnet/csharp/",
    "https://ziglang.org/documentation/",
    "https://docs.racket-lang.org/",

    # --- Web Frameworks / Frontend Docs (15) ---
    "https://react.dev/",
    "https://angular.dev/",
    "https://vuejs.org/guide/introduction.html",
    "https://svelte.dev/docs",
    "https://nextjs.org/docs",
    "https://nuxt.com/docs",
    "https://remix.run/docs",
    "https://gatsbyjs.com/docs/",
    "https://getbootstrap.com/docs/",
    "https://tailwindcss.com/docs/",
    "https://jquery.com/",
    "https://htmx.org/docs/",
    "https://alpinejs.dev/start-here",
    "https://docusaurus.io/docs",
    "https://astro.build/docs",

    # --- Devops / Infrastructure Docs (15) ---
    "https://docs.docker.com/",
    "https://kubernetes.io/docs/home/",
    "https://docs.aws.amazon.com/",
    "https://cloud.google.com/docs",
    "https://learn.microsoft.com/en-us/azure/",
    "https://www.terraform.io/docs",
    "https://www.vagrantup.com/docs",
    "https://docs.ansible.com/",
    "https://www.jenkins.io/doc/",
    "https://grafana.com/docs/",
    "https://prometheus.io/docs/",
    "https://www.nginx.com/resources/wiki/",
    "https://httpd.apache.org/docs/",
    "https://www.mongodb.com/docs/",
    "https://www.postgresql.org/docs/",

    # --- Wikipedia (20) ---
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
    "https://en.wikipedia.org/wiki/Linux",
    "https://en.wikipedia.org/wiki/Internet",
    "https://en.wikipedia.org/wiki/Computer_science",
    "https://en.wikipedia.org/wiki/Cryptography",
    "https://en.wikipedia.org/wiki/Quantum_computing",
    "https://en.wikipedia.org/wiki/Blockchain",
    "https://en.wikipedia.org/wiki/Climate_change",
    "https://en.wikipedia.org/wiki/Economics",
    "https://en.wikipedia.org/wiki/Philosophy",
    "https://en.wikipedia.org/wiki/Medicine",

    # --- News (15) ---
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
    "https://www.bloomberg.com/",
    "https://www.ft.com/",
    "https://www.latimes.com/",
    "https://www.washingtonpost.com/",
    "https://www.chicagotribune.com/",

    # --- Tech News (12) ---
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
    "https://www.thurrott.com/",
    "https://9to5mac.com/",

    # --- AI / ML Specific (15) ---
    "https://huggingface.co/blog",
    "https://openai.com/blog/",
    "https://www.anthropic.com/blog/",
    "https://pytorch.org/docs/stable/",
    "https://www.tensorflow.org/learn",
    "https://scikit-learn.org/stable/",
    "https://keras.io/",
    "https://www.deepmind.com/blog",
    "https://ai.googleblog.com/",
    "https://lilianweng.github.io/",
    "https://www.fast.ai/",
    "https://jalammar.github.io/",
    "https://colah.github.io/",
    "https://distill.pub/",
    "https://arxiv.org/list/cs.AI/recent",

    # --- Q&A (10) ---
    "https://stackoverflow.com/questions/927358/",
    "https://stackoverflow.com/questions/11227809/",
    "https://stackoverflow.com/questions/388242/",
    "https://serverfault.com/",
    "https://superuser.com/",
    "https://askubuntu.com/",
    "https://www.quora.com/",
    "https://stackoverflow.com/questions/1642028/",
    "https://stackoverflow.com/questions/1452721/",
    "https://stackoverflow.com/questions/600795/",

    # --- Developer Blogs (20) ---
    "https://martinfowler.com/",
    "https://blog.codinghorror.com/",
    "https://www.joelonsoftware.com/",
    "https://karpathy.github.io/",
    "https://simonwillison.net/",
    "https://stratechery.com/",
    "https://apenwarr.ca/log/",
    "https://boringtechnology.club/",
    "https://blog.rust-lang.org/",
    "https://github.blog/engineering/",
    "https://engineering.fb.com/",
    "https://netflixtechblog.com/",
    "https://slack.engineering/",
    "https://medium.com/",
    "https://towardsdatascience.com/",
    "https://betterprogramming.pub/",
    "https://dzone.com/",
    "https://css-tricks.com/",
    "https://www.smashingmagazine.com/",
    "https://alistapart.com/",

    # --- GitHub / Code Hosting (10) ---
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

    # --- Academic / Research (15) ---
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
    "https://pubmed.ncbi.nlm.nih.gov/",
    "https://www.semanticscholar.org/",
    "https://www.researchgate.net/",
    "https://www.cambridge.org/core",
    "https://link.springer.com/",

    # --- Reference / Dictionaries (10) ---
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

    # --- Package Registries / Tools (10) ---
    "https://crates.io/",
    "https://pypi.org/",
    "https://rubygems.org/",
    "https://www.nuget.org/",
    "https://mvnrepository.com/",
    "https://hex.pm/",
    "https://www.npmjs.com/",
    "https://brew.sh/",
    "https://snapcraft.io/",
    "https://flatpak.org/",

    # --- Standards / Protocol Docs (8) ---
    "https://www.ietf.org/",
    "https://www.w3.org/TR/",
    "https://www.rfc-editor.org/",
    "https://www.iso.org/",
    "https://www.nist.gov/",
    "https://whatwg.org/",
    "https://spec.graphql.org/",
    "https://json-schema.org/",

    # --- Social / Community (8) ---
    "https://www.reddit.com/r/programming/",
    "https://lobste.rs/",
    "https://meta.stackexchange.com/",
    "https://discourse.org/",
    "https://www.producthunt.com/",
    "https://www.reddit.com/r/MachineLearning/",
    "https://www.reddit.com/r/rust/",
    "https://www.reddit.com/r/Python/",

    # --- Government / Organizations (8) ---
    "https://www.gov.uk/",
    "https://www.whitehouse.gov/",
    "https://www.usa.gov/",
    "https://www.europa.eu/",
    "https://www.un.org/",
    "https://www.oecd.org/",
    "https://www.imf.org/",
    "https://www.worldbank.org/",

    # --- Educational (10) ---
    "https://example.com/",
    "https://lorem-ipsum.in/",
    "https://www.gutenberg.org/",
    "https://www.oreilly.com/",
    "https://stackoverflow.blog/",
    "https://www.tutorialspoint.com/",
    "https://www.geeksforgeeks.org/",
    "https://www.w3schools.com/",
    "https://www.khanacademy.org/",
    "https://www.coursera.org/",

    # --- International News / Content (12) ---
    "https://www.bbc.co.uk/news/technology",
    "https://www.lemonde.fr/",
    "https://www.spiegel.de/",
    "https://www.dw.com/",
    "https://www.scmp.com/",
    "https://www.japantimes.co.jp/",
    "https://timesofindia.indiatimes.com/",
    "https://www.abc.net.au/news/",
    "https://www.theglobeandmail.com/",
    "https://www.smh.com.au/",
    "https://www.nikkei.com/",
    "https://www.ft.com/world/asia-pacific",

    # --- Design / UX (8) ---
    "https://www.nngroup.com/articles/",
    "https://uxdesign.cc/",
    "https://www.fastcompany.com/design",
    "https://dribbble.com/",
    "https://www.behance.net/",
    "https://material.io/design",
    "https://developer.apple.com/design/",
    "https://m3.material.io/",

    # --- Security / Privacy (8) ---
    "https://www.kb.cert.org/",
    "https://nvd.nist.gov/",
    "https://cve.mitre.org/",
    "https://portswigger.net/blog",
    "https://www.owasp.org/",
    "https://www.schneier.com/",
    "https://blog.talosintelligence.com/",
    "https://www.securityweek.com/",

    # --- Science / Health (8) ---
    "https://www.nih.gov/",
    "https://www.cdc.gov/",
    "https://www.ema.europa.eu/en",
    "https://www.fda.gov/",
    "https://www.newscientist.com/",
    "https://www.scientificamerican.com/",
    "https://www.quantamagazine.org/",
    "https://www.space.com/",

    # --- Developer Tools (8) ---
    "https://code.visualstudio.com/docs",
    "https://www.jetbrains.com/help/",
    "https://neovim.io/doc/",
    "https://www.gnu.org/software/emacs/",
    "https://git-scm.com/doc",
    "https://www.mercurial-scm.org/",
    "https://www.vim.org/docs.php",
    "https://www.gnu.org/software/bash/",

    # --- Tech Company Engineering Blogs (10) ---
    "https://engineering.linkedin.com/blog",
    "https://eng.uber.com/",
    "https://engineering.atspotify.com/",
    "https://stripe.com/blog/engineering",
    "https://dropbox.tech/",
    "https://blog.twitter.com/engineering",
    "https://instagram-engineering.com/",
    "https://about.gitlab.com/blog/",
    "https://blogs.oracle.com/",
    "https://cloudblogs.microsoft.com/",

    # --- Databases / Data (8) ---
    "https://redis.io/docs/",
    "https://www.elastic.co/guide/index.html",
    "https://cassandra.apache.org/doc/latest/",
    "https://www.sqlite.org/docs.html",
    "https://learn.microsoft.com/en-us/sql/",
    "https://neo4j.com/docs/",
    "https://clickhouse.com/docs",
    "https://www.prisma.io/docs",

    # --- API / Integration (8) ---
    "https://graphql.org/learn/",
    "https://restfulapi.net/",
    "https://grpc.io/docs/",
    "https://www.openapis.org/",
    "https://swagger.io/docs/",
    "https://kafka.apache.org/documentation/",
    "https://www.rabbitmq.com/documentation.html",
    "https://nats.io/documentation/",

    # --- Testing / QA (6) ---
    "https://jestjs.io/docs/",
    "https://playwright.dev/docs/",
    "https://www.selenium.dev/documentation/",
    "https://docs.cypress.io/",
    "https://mochajs.org/",
    "https://junit.org/junit5/docs/current/",

    # --- Additional Dev Tools (5) ---
    "https://nixos.org/manual/nix/stable/",
    "https://www.gnu.org/software/make/manual/",
    "https://cmake.org/documentation/",
    "https://mesonbuild.com/",
    "https://bazel.build/",
]

# Subset that gets the readable test too
READABLE_DOMAINS = [
    "wikipedia.org", "bbc.com", "bbc.co.uk", "reuters.com", "apnews.com",
    "theguardian.com", "arstechnica.com", "dev.to", "martinfowler.com",
    "karpathy.github.io", "simonwillison.net", "stratechery.com",
    "anthropic.com", "openai.com", "huggingface.co", "deepmind.com",
    "blog.rust-lang.org", "github.blog", "netflixtechblog.com",
    "slack.engineering", "stripe.com", "dropbox.tech",
    "lilianweng.github.io", "jalammar.github.io", "colah.github.io",
    "distill.pub", "fast.ai", "quantamagazine.org",
    "blog.talosintelligence.com", "portswigger.net",
    "eng.uber.com", "engineering.atspotify.com",
    "about.gitlab.com", "nngroup.com",
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
