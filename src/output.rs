use crate::cache::CachedPost;

/// Render all posts into an LLM-friendly Markdown document.
pub fn render(title: &str, source_url: &str, posts: &[CachedPost]) -> String {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC");
    let mut out = String::new();

    // Header
    out.push_str(&format!("# {}\n\n", title));
    out.push_str(&format!("- **Source**: {}\n", source_url));
    out.push_str(&format!("- **Fetched**: {}\n", now));
    out.push_str(&format!("- **Posts**: {}\n", posts.len()));
    out.push_str("\n---\n\n");

    // Posts
    for post in posts {
        let date = post.created_at.format("%Y-%m-%d %H:%M UTC");
        out.push_str(&format!(
            "## Post #{} by @{} ({})\n\n",
            post.post_number, post.username, date
        ));
        out.push_str(&post.raw);
        if !post.raw.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("\n---\n\n");
    }

    out
}
