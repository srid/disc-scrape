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
disc-scrape https://meta.discourse.org/t/some-topic/12345

# Save to a file for LLM use
disc-scrape -o thread.md https://meta.discourse.org/t/some-topic/12345

# Force re-download of everything (set cache to 0 days)
disc-scrape -c 0 https://meta.discourse.org/t/some-topic/12345

# Verbose mode to see download progress
disc-scrape -v https://meta.discourse.org/t/some-topic/12345
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

## Caching

Posts are cached in `~/.cache/disc-scrape/{domain}/{topic_id}/`. Posts created more than `--cache-days` days ago are served from cache without re-downloading. Recent posts are always re-fetched to capture edits.

## Development (Flakes)

```bash
# Enter dev shell
nix develop

# Run
cargo run -- https://meta.discourse.org/t/some-topic/12345

# Build
nix build
```
