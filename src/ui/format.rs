//! Display helpers: dates, avatars, and the sandboxed email document.

use dioxus::prelude::*;

pub use crate::classify::{sender_address, sender_name};

/// "09:05" today, "18 Nov" this year, "18 Nov 2025" before.
pub fn fmt_date(d: &str) -> String {
    let now = chrono::Local::now();
    if d.starts_with(&now.format("%Y-%m-%d").to_string()) {
        return d.get(11..).unwrap_or(d).to_string();
    }
    match chrono::NaiveDate::parse_from_str(d.get(..10).unwrap_or(""), "%Y-%m-%d") {
        Ok(date) if d.starts_with(&now.format("%Y").to_string()) => date.format("%-d %b").to_string(),
        Ok(date) => date.format("%-d %b %Y").to_string(),
        Err(_) => d.to_string(),
    }
}

/// "18 Nov" for one day, "3 Mar – 18 Nov" for a span.
pub fn fmt_range(first: &str, last: &str) -> String {
    if first.get(..10) == last.get(..10) { fmt_date(last) } else { format!("{} – {}", fmt_date(first), fmt_date(last)) }
}

/// Initial on a soft color picked from the name or address, so a sender always gets the same color.
pub fn avatar(sender: &str, class: &str) -> Element {
    let initial = sender_name(sender).chars().find(|c| c.is_alphanumeric()).map_or('?', |c| c.to_ascii_uppercase());
    let hue = sender_address(sender).to_lowercase().bytes().fold(0u32, |h, b| (h * 31 + b as u32) % 360);
    rsx! {
        span { class: "avatar {class}", style: "--h:{hue}", "{initial}" }
    }
}

/// Checkbox drawn from state, not a native input that toggles itself and can drift out of sync.
/// `partial` shows a dash (some selected). Put it inside a clickable `.check-hit`.
pub fn check(checked: bool, partial: bool) -> Element {
    let state = if checked { "checked" } else if partial { "partial" } else { "" };
    let aria = if checked { "true" } else if partial { "mixed" } else { "false" };
    rsx! {
        span { class: "check {state}", role: "checkbox", "aria-checked": aria }
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Email bodies are untrusted HTML. Rendered in a sandboxed iframe (no scripts) with a policy that
/// blocks remote images unless the user asks for them: loading them tells the sender you opened it.
/// Links open in the default browser.
pub fn email_document(mime: &str, body: &str, show_images: bool) -> String {
    let images = if show_images { "data: https: http:" } else { "data:" };
    let policy = format!(
        r#"<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src {images}">"#
    );
    if mime == "text/html" {
        format!(r#"{policy}<base target="_top">{body}"#)
    } else {
        format!(r#"{policy}<pre style="white-space:pre-wrap;font:14px/1.5 system-ui,sans-serif;margin:16px">{}</pre>"#, html_escape(body))
    }
}

/// Whether an HTML body loads images from the web (and so has something to show).
pub fn has_remote_images(mime: &str, body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    mime == "text/html" && ["src=\"http", "src='http", "src=http", "url(http", "url('http", "url(\"http"].iter().any(|p| lower.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_bodies_are_escaped() {
        assert!(email_document("text/plain", "<script>", false).contains("&lt;script&gt;"));
    }

    #[test]
    fn remote_images_only_when_asked() {
        assert!(email_document("text/html", "x", false).contains("img-src data:\""));
        assert!(email_document("text/html", "x", true).contains("img-src data: https: http:\""));
        assert!(has_remote_images("text/html", r#"<IMG SRC="https://t.co/p.gif">"#));
        assert!(!has_remote_images("text/html", r#"<img src="data:image/png;base64,AA">"#));
        assert!(!has_remote_images("text/plain", "src=\"http://x\""));
    }
}
