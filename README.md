# disc-scrape

Download Discourse thread posts as raw Markdown for LLM consumption, with smart caching.

## Usage

```
disc-scrape [OPTIONS] <URL>
```

### Arguments

- `<URL>` — Discourse thread URL (e.g. `https://discuss.example.com/t/topic-slug/12345`)

### Options

- `-o, --output <FILE>` — Write output to a file (default: stdout)
- `-c, --cache-days <N>` — Cache threshold in days (default: 4). Posts older than N days are served from cache.
- `-v, --verbose` — Show progress and debug information on stderr
- `-h, --help` — Show help
- `-V, --version` — Show version

### Examples

```bash
# Print a thread's posts to stdout
nix run github:srid/disc-scrape -- https://meta.discourse.org/t/some-topic/12345

# Save to a file for LLM use
nix run github:srid/disc-scrape -- -o thread.md https://meta.discourse.org/t/some-topic/12345

# Force re-download of everything (set cache to 0 days)
nix run github:srid/disc-scrape -- -c 0 https://meta.discourse.org/t/some-topic/12345

# Verbose mode to see download progress
nix run github:srid/disc-scrape -- -v https://meta.discourse.org/t/some-topic/12345
```

## Output Format

The output is a clean Markdown document designed for LLM context windows:

```markdown
# Topic Title

- **Source**: https://discuss.example.com/t/topic-slug/12345
- **Fetched**: 2026-02-27 16:00 UTC
- **Posts**: 42

---

## Post #1 by @username (2026-02-20 10:00 UTC)

Original markdown content of the post...

---

## Post #2 by @another_user (2026-02-21 14:30 UTC)

Another post's content...

---
```

## How It Works

1. **Parse the URL** — Extracts the base domain and topic ID from the Discourse thread URL
2. **Fetch topic metadata** — Calls `/t/{topic_id}.json` to get the topic title and full list of post IDs
3. **Resolve post metadata** — The first ~20 posts come inline; remaining post IDs are batch-fetched via `/t/{topic_id}/posts.json?post_ids[]=...`
4. **Download raw Markdown** — For each post, fetches `/raw/{topic_id}/{post_number}` to get the original Markdown source (not rendered HTML)
5. **Cache** — Each post is cached as a JSON file keyed by post ID. On subsequent runs, posts older than `--cache-days` are served from cache; recent posts are always re-fetched to capture edits
6. **Render** — All posts are assembled into a single Markdown document with metadata headers, suitable for pasting into an LLM context window

## Caching

Posts are cached in `{cache_dir}/disc-scrape/{domain}/{topic_id}/` (`~/Library/Caches/` on macOS, `~/.cache/` on Linux). Posts created more than `--cache-days` days ago are served from cache without re-downloading. Recent posts are always re-fetched to capture edits.

## Development (Flakes)

```bash
# Enter dev shell
nix develop

# Run
cargo run -- https://meta.discourse.org/t/some-topic/12345

# Build
nix build
```
