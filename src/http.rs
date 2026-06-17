//! HTTP fetch with retry-and-backoff for rate limiting and transient errors.
//!
//! Discourse forums return `429 Too Many Requests` when a scrape is too
//! aggressive, and may briefly fail with 5xx or connection/timeout errors.
//! Without handling, a single such response aborts the whole scrape. This
//! module wraps a blocking GET so those cases retry instead of aborting:
//! it honours a `Retry-After` header when present, otherwise falls back to
//! capped exponential backoff, with a bounded number of attempts.

use anyhow::{bail, Result};
use std::time::Duration;

/// Maximum number of attempts (initial try + retries) before giving up.
const MAX_ATTEMPTS: u32 = 6;
/// Base delay for exponential backoff (doubles each retry: 1, 2, 4, 8, …).
const BASE_BACKOFF: Duration = Duration::from_secs(1);
/// Upper bound on any single backoff wait.
const MAX_BACKOFF: Duration = Duration::from_secs(60);

/// Perform a GET with retry-and-backoff on 429, 5xx, and transport errors.
///
/// `build` is called to construct a fresh `RequestBuilder` for each attempt
/// (a `reqwest::blocking::RequestBuilder` is consumed by `send`, so it can't
/// be reused across retries). `label` is used only for log lines on stderr.
/// `verbose` gates routine retry chatter; rate-limit (429) waits are always
/// surfaced because the user is otherwise staring at a stalled progress bar.
pub fn get_with_retry<F>(
    label: &str,
    verbose: bool,
    build: F,
) -> Result<reqwest::blocking::Response>
where
    F: Fn() -> reqwest::blocking::RequestBuilder,
{
    let mut attempt: u32 = 1;
    loop {
        match build().send() {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return Ok(resp);
                }

                // Retry on rate limiting (429) and transient server errors (5xx).
                let retryable = status.as_u16() == 429 || status.is_server_error();
                if !retryable || attempt >= MAX_ATTEMPTS {
                    bail!("{}: HTTP {}", label, status);
                }

                let wait = retry_after(&resp).unwrap_or_else(|| backoff_for(attempt));
                // A 429 means we are being rate-limited; surface it even
                // without -v so the apparent stall has an explanation.
                if status.as_u16() == 429 {
                    eprintln!(
                        "{}: HTTP 429 (rate limited), retrying in {}s (attempt {}/{})",
                        label,
                        wait.as_secs(),
                        attempt,
                        MAX_ATTEMPTS
                    );
                } else if verbose {
                    eprintln!(
                        "{}: HTTP {}, retrying in {}s (attempt {}/{})",
                        label,
                        status,
                        wait.as_secs(),
                        attempt,
                        MAX_ATTEMPTS
                    );
                }
                std::thread::sleep(wait);
            }
            Err(err) => {
                // Connection/timeout errors: retry conservatively.
                if attempt >= MAX_ATTEMPTS {
                    return Err(
                        anyhow::Error::new(err).context(format!("{}: request failed", label))
                    );
                }
                let wait = backoff_for(attempt);
                if verbose {
                    eprintln!(
                        "{}: request error ({}), retrying in {}s (attempt {}/{})",
                        label,
                        err,
                        wait.as_secs(),
                        attempt,
                        MAX_ATTEMPTS
                    );
                }
                std::thread::sleep(wait);
            }
        }
        attempt += 1;
    }
}

/// Exponential backoff for a given attempt number (1-based), capped.
///
/// attempt 1 → 1s, 2 → 2s, 3 → 4s, 4 → 8s, … saturating at [`MAX_BACKOFF`].
fn backoff_for(attempt: u32) -> Duration {
    // Shift by (attempt - 1); guard against overflow on large exponents.
    let factor = 1u64
        .checked_shl(attempt.saturating_sub(1))
        .unwrap_or(u64::MAX);
    let secs = BASE_BACKOFF.as_secs().saturating_mul(factor);
    Duration::from_secs(secs.min(MAX_BACKOFF.as_secs()))
}

/// Read and parse the `Retry-After` header from a response, if present and valid.
fn retry_after(resp: &reqwest::blocking::Response) -> Option<Duration> {
    let value = resp.headers().get(reqwest::header::RETRY_AFTER)?;
    let text = value.to_str().ok()?;
    parse_retry_after(text)
}

/// Parse a `Retry-After` header value into a wait duration.
///
/// Two forms are defined by RFC 7231 §7.1.3:
/// - delta-seconds: a non-negative integer number of seconds (e.g. `120`).
/// - HTTP-date: an absolute time (e.g. `Wed, 21 Oct 2015 07:28:00 GMT`); the
///   wait is the difference between that time and now (never negative).
///
/// The result is clamped to [`MAX_BACKOFF`] so a hostile/buggy header can't
/// stall the scrape for an unreasonable length of time. Returns `None` when
/// the value can't be parsed as either form.
fn parse_retry_after(value: &str) -> Option<Duration> {
    let value = value.trim();

    // Form 1: delta-seconds.
    if let Ok(secs) = value.parse::<u64>() {
        return Some(Duration::from_secs(secs.min(MAX_BACKOFF.as_secs())));
    }

    // Form 2: HTTP-date.
    if let Ok(when) = chrono::DateTime::parse_from_rfc2822(value) {
        let delta = when.with_timezone(&chrono::Utc) - chrono::Utc::now();
        let secs = delta.num_seconds().max(0) as u64;
        return Some(Duration::from_secs(secs.min(MAX_BACKOFF.as_secs())));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_then_caps() {
        assert_eq!(backoff_for(1), Duration::from_secs(1));
        assert_eq!(backoff_for(2), Duration::from_secs(2));
        assert_eq!(backoff_for(3), Duration::from_secs(4));
        assert_eq!(backoff_for(4), Duration::from_secs(8));
        assert_eq!(backoff_for(5), Duration::from_secs(16));
        assert_eq!(backoff_for(6), Duration::from_secs(32));
        // Caps at MAX_BACKOFF (60s) for large attempt numbers.
        assert_eq!(backoff_for(7), Duration::from_secs(60));
        assert_eq!(backoff_for(100), Duration::from_secs(60));
    }

    #[test]
    fn retry_after_delta_seconds() {
        assert_eq!(parse_retry_after("0"), Some(Duration::from_secs(0)));
        assert_eq!(parse_retry_after("5"), Some(Duration::from_secs(5)));
        assert_eq!(parse_retry_after("  30 "), Some(Duration::from_secs(30)));
    }

    #[test]
    fn retry_after_seconds_clamped_to_max() {
        // A huge delta-seconds value is clamped to MAX_BACKOFF.
        assert_eq!(parse_retry_after("100000"), Some(Duration::from_secs(60)));
    }

    #[test]
    fn retry_after_http_date_in_past_is_zero() {
        // A date in the past yields a non-negative (zero) wait.
        assert_eq!(
            parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(Duration::from_secs(0))
        );
    }

    #[test]
    fn retry_after_http_date_in_future_is_clamped() {
        // Far-future date clamps to MAX_BACKOFF rather than waiting for years.
        let future = chrono::Utc::now() + chrono::Duration::days(365);
        let header = future.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        assert_eq!(parse_retry_after(&header), Some(MAX_BACKOFF));
    }

    #[test]
    fn retry_after_garbage_is_none() {
        assert_eq!(parse_retry_after("soon"), None);
        assert_eq!(parse_retry_after(""), None);
        assert_eq!(parse_retry_after("-5"), None);
    }
}
