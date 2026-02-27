use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A cached post with metadata and raw content.
#[derive(Debug, Serialize, Deserialize)]
pub struct CachedPost {
    pub post_number: u64,
    pub post_id: u64,
    pub username: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub raw: String,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

/// File-based cache for Discourse posts.
///
/// Cache layout: `~/.cache/disc-scrape/{domain}/{topic_id}/{post_id}.json`
pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    /// Create a new cache for the given domain and topic.
    pub fn new(domain: &str, topic_id: u64) -> Result<Self> {
        let cache_base = directories::ProjectDirs::from("", "", "disc-scrape")
            .context("Could not determine cache directory")?;
        let dir = cache_base
            .cache_dir()
            .join(domain)
            .join(topic_id.to_string());
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create cache directory: {:?}", dir))?;
        Ok(Self { dir })
    }

    /// Load a cached post by post ID, if it exists.
    pub fn load_by_id(&self, post_id: u64) -> Result<Option<CachedPost>> {
        let path = self.post_path(post_id);
        if !path.exists() {
            return Ok(None);
        }
        let data =
            std::fs::read_to_string(&path).with_context(|| format!("Failed to read {:?}", path))?;
        let post: CachedPost =
            serde_json::from_str(&data).with_context(|| format!("Failed to parse {:?}", path))?;
        Ok(Some(post))
    }

    /// Save a post to the cache (keyed by post_id).
    pub fn save(&self, post: &CachedPost) -> Result<()> {
        let path = self.post_path(post.post_id);
        let data = serde_json::to_string_pretty(post).context("Failed to serialize post")?;
        std::fs::write(&path, data).with_context(|| format!("Failed to write {:?}", path))?;
        Ok(())
    }

    fn post_path(&self, post_id: u64) -> PathBuf {
        self.dir.join(format!("{}.json", post_id))
    }
}
