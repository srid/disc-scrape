use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// Parsed topic metadata from Discourse JSON API
#[derive(Debug, Deserialize)]
pub struct Topic {
    pub title: String,
    pub post_stream: PostStream,
}

#[derive(Debug, Deserialize)]
pub struct PostStream {
    /// Full list of post IDs in the topic
    pub stream: Vec<u64>,
    /// Inline posts (first ~20 posts are included in the topic response)
    #[serde(default)]
    pub posts: Vec<PostData>,
}

/// Post data from the Discourse JSON API
#[derive(Debug, Clone, Deserialize)]
pub struct PostData {
    pub id: u64,
    pub post_number: u64,
    pub username: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Parse a Discourse topic URL into (base_url, topic_id).
///
/// Supported URL formats:
/// - `https://discuss.example.com/t/topic-slug/12345`
/// - `https://discuss.example.com/t/topic-slug/12345/42`  (with post number)
/// - `https://discuss.example.com/t/12345`
pub fn parse_topic_url(url_str: &str) -> Result<(String, u64)> {
    let parsed = url::Url::parse(url_str).context("Invalid URL")?;

    let scheme = parsed.scheme();
    let host = parsed.host_str().context("URL has no host")?;
    let port_suffix = parsed.port().map(|p| format!(":{}", p)).unwrap_or_default();
    let base_url = format!("{}://{}{}", scheme, host, port_suffix);

    let segments: Vec<&str> = parsed.path_segments().context("URL has no path")?.collect();

    // Expect /t/slug/id or /t/id pattern
    if segments.is_empty() || segments[0] != "t" {
        bail!("URL does not look like a Discourse topic URL (expected /t/...)");
    }

    // Find the topic ID — it's the first purely numeric segment after /t/
    let topic_id = segments
        .iter()
        .skip(1)
        .find_map(|s| s.parse::<u64>().ok())
        .context("Could not find topic ID in URL")?;

    Ok((base_url, topic_id))
}

/// Fetch topic metadata including the full post stream.
pub fn fetch_topic(base_url: &str, topic_id: u64) -> Result<Topic> {
    let url = format!("{}/t/{}.json", base_url, topic_id);
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .context("HTTP request failed")?;

    if !resp.status().is_success() {
        bail!("Failed to fetch topic {}: HTTP {}", topic_id, resp.status());
    }

    let topic: Topic = resp.json().context("Failed to parse topic JSON")?;
    Ok(topic)
}

/// Batch-fetch post metadata for a set of post IDs.
///
/// Uses `/t/{topic_id}/posts.json?post_ids[]=...` endpoint.
/// Discourse typically allows ~20 IDs per request.
pub fn fetch_posts_by_ids(
    base_url: &str,
    topic_id: u64,
    post_ids: &[u64],
) -> Result<Vec<PostData>> {
    let client = reqwest::blocking::Client::new();

    let mut all_posts = Vec::new();

    // Batch in chunks of 20
    for chunk in post_ids.chunks(20) {
        let mut url = format!("{}/t/{}/posts.json?", base_url, topic_id);
        for (i, id) in chunk.iter().enumerate() {
            if i > 0 {
                url.push('&');
            }
            url.push_str(&format!("post_ids[]={}", id));
        }

        let resp = client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .with_context(|| format!("HTTP request failed for batch post fetch"))?;

        if !resp.status().is_success() {
            bail!("Failed to batch-fetch posts: HTTP {}", resp.status());
        }

        let body: serde_json::Value = resp.json().context("Failed to parse JSON")?;

        let posts_array = body["post_stream"]["posts"]
            .as_array()
            .context("No post_stream.posts in batch response")?;

        for post_value in posts_array {
            let post: PostData =
                serde_json::from_value(post_value.clone()).context("Failed to parse post data")?;
            all_posts.push(post);
        }

        // Small delay between batches
        if post_ids.len() > 20 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    Ok(all_posts)
}

/// Fetch the raw Markdown content for a post via /raw/{topic_id}/{post_number}.
pub fn fetch_raw_post(base_url: &str, topic_id: u64, post_number: u64) -> Result<String> {
    let url = format!("{}/raw/{}/{}", base_url, topic_id, post_number);
    let client = reqwest::blocking::Client::new();
    let resp = client.get(&url).send().context("HTTP request failed")?;

    if !resp.status().is_success() {
        bail!(
            "Failed to fetch raw post #{}: HTTP {}",
            post_number,
            resp.status()
        );
    }

    let text = resp.text().context("Failed to read response body")?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_topic_url_with_slug() {
        let (base, id) = parse_topic_url("https://discuss.example.com/t/my-topic/12345").unwrap();
        assert_eq!(base, "https://discuss.example.com");
        assert_eq!(id, 12345);
    }

    #[test]
    fn test_parse_topic_url_with_post_number() {
        let (base, id) =
            parse_topic_url("https://discuss.example.com/t/my-topic/12345/42").unwrap();
        assert_eq!(base, "https://discuss.example.com");
        assert_eq!(id, 12345);
    }

    #[test]
    fn test_parse_topic_url_no_slug() {
        let (base, id) = parse_topic_url("https://discuss.example.com/t/12345").unwrap();
        assert_eq!(base, "https://discuss.example.com");
        assert_eq!(id, 12345);
    }

    #[test]
    fn test_parse_topic_url_invalid() {
        assert!(parse_topic_url("https://example.com/not-discourse").is_err());
    }
}
