use crate::cache::CachedPost;

/// Render the document header (everything before the first post).
///
/// `post_count` is the total number of posts the document will contain.
/// When streaming output, this is known up front from the topic's post stream.
pub fn render_header(title: &str, source_url: &str, post_count: usize) -> String {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC");
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", title));
    out.push_str(&format!("- **Source**: {}\n", source_url));
    out.push_str(&format!("- **Fetched**: {}\n", now));
    out.push_str(&format!("- **Posts**: {}\n", post_count));
    out.push_str("\n---\n\n");

    out
}

/// Render a single post (heading, raw body, and trailing separator).
pub fn render_post(post: &CachedPost) -> String {
    let date = post.created_at.format("%Y-%m-%d %H:%M UTC");
    let mut out = String::new();
    out.push_str(&format!(
        "## Post #{} by @{} ({})\n\n",
        post.post_number, post.username, date
    ));
    out.push_str(&post.raw);
    if !post.raw.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n---\n\n");
    out
}
