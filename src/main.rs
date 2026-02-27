mod cache;
mod discourse;
mod output;

use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[clap(
    author = "Sridhar Ratnakumar",
    version,
    about = "Download Discourse thread posts as raw Markdown for LLM consumption"
)]
struct Args {
    /// Discourse thread URL (e.g. https://discuss.example.com/t/topic-slug/12345)
    #[arg()]
    url: String,

    /// Output file (default: stdout)
    #[arg(short, long)]
    output: Option<String>,

    /// Cache threshold in days — posts older than this are not re-downloaded
    #[arg(short, long, default_value_t = 4)]
    cache_days: u64,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let (base_url, topic_id) =
        discourse::parse_topic_url(&args.url).context("Failed to parse Discourse thread URL")?;

    if args.verbose {
        eprintln!("Base URL: {}", base_url);
        eprintln!("Topic ID: {}", topic_id);
    }

    // Fetch topic metadata and post stream
    if args.verbose {
        eprintln!("Fetching topic metadata...");
    }
    let topic = discourse::fetch_topic(&base_url, topic_id).context("Failed to fetch topic")?;

    if args.verbose {
        eprintln!("Topic: {}", topic.title);
        eprintln!("Total posts: {}", topic.post_stream.stream.len());
    }

    // Set up cache
    let domain = url::Url::parse(&base_url)
        .map(|u| u.host_str().unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let cache = cache::Cache::new(&domain, topic_id)?;

    let cache_threshold = chrono::Utc::now() - chrono::Duration::days(args.cache_days as i64);

    // Build a map of post_id -> PostData from inline posts in the topic response
    let mut post_data_by_id: HashMap<u64, discourse::PostData> = HashMap::new();
    for post in &topic.post_stream.posts {
        post_data_by_id.insert(post.id, post.clone());
    }

    // Figure out which post IDs we still need to fetch
    // (not in inline posts AND not cached or cache is stale)
    let all_post_ids = &topic.post_stream.stream;

    // First pass: check cache for all posts, collect IDs that need fetching
    let mut ids_to_fetch: Vec<u64> = Vec::new();
    for &post_id in all_post_ids {
        if post_data_by_id.contains_key(&post_id) {
            // We have inline data — still need to check cache for raw content
            continue;
        }
        // Check cache by post_id
        if let Some(cached) = cache.load_by_id(post_id)? {
            if cached.created_at < cache_threshold {
                // Old enough, trust cache — no need to fetch
                continue;
            }
        }
        ids_to_fetch.push(post_id);
    }

    // Batch-fetch metadata for posts we don't have inline
    if !ids_to_fetch.is_empty() {
        if args.verbose {
            eprintln!(
                "Batch-fetching metadata for {} posts...",
                ids_to_fetch.len()
            );
        }
        let fetched = discourse::fetch_posts_by_ids(&base_url, topic_id, &ids_to_fetch)
            .context("Failed to batch-fetch posts")?;
        for post in fetched {
            post_data_by_id.insert(post.id, post);
        }
    }

    // Now iterate through all posts in order, fetching raw content as needed
    let mut posts: Vec<cache::CachedPost> = Vec::new();
    let total = all_post_ids.len();

    for (i, &post_id) in all_post_ids.iter().enumerate() {
        // Check cache first (keyed by post_id)
        if let Some(cached) = cache.load_by_id(post_id)? {
            if cached.created_at < cache_threshold {
                if args.verbose {
                    eprintln!(
                        "[{}/{}] Post #{} (id={}) cached, skipping",
                        i + 1,
                        total,
                        cached.post_number,
                        post_id
                    );
                }
                posts.push(cached);
                continue;
            }
        }

        // Get post metadata
        let post_data = post_data_by_id
            .get(&post_id)
            .with_context(|| format!("No metadata for post id={}", post_id))?;

        // Fetch raw markdown via /raw/{topic_id}/{post_number}
        if args.verbose {
            eprintln!(
                "[{}/{}] Fetching raw post #{} (id={})...",
                i + 1,
                total,
                post_data.post_number,
                post_id
            );
        }
        let raw = discourse::fetch_raw_post(&base_url, topic_id, post_data.post_number)
            .with_context(|| {
                format!(
                    "Failed to fetch raw content for post #{}",
                    post_data.post_number
                )
            })?;

        let cached_post = cache::CachedPost {
            post_number: post_data.post_number,
            post_id: post_data.id,
            username: post_data.username.clone(),
            created_at: post_data.created_at,
            raw,
            fetched_at: chrono::Utc::now(),
        };

        cache.save(&cached_post)?;
        posts.push(cached_post);

        // Small delay to be respectful to the server
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // Generate output
    let rendered = output::render(&topic.title, &args.url, &posts);

    if let Some(path) = &args.output {
        std::fs::write(path, &rendered)
            .with_context(|| format!("Failed to write output to {}", path))?;
        eprintln!("Output written to {}", path);
    } else {
        print!("{}", rendered);
    }

    Ok(())
}
