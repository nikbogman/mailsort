//! Web demo stand-in for the Gmail API: actions succeed after a short delay, bodies are generated.

use std::time::Duration;

use crate::db;

pub type Result<T> = std::result::Result<T, String>;

const LATENCY: Duration = Duration::from_millis(150);

pub fn has_credentials() -> bool {
    true
}

pub async fn get_body(id: &str) -> Result<(String, String)> {
    crate::sleep(LATENCY).await;
    let e = db::by_ids(&[id.to_string()]).pop().ok_or("Email not found")?;
    let html = format!(
        "<div style=\"font-family:system-ui,sans-serif;max-width:560px;margin:24px auto;line-height:1.6;color:#1b1d22\">\
         <h2 style=\"margin:0 0 12px\">{}</h2><p>{}</p>\
         <p style=\"color:#5d6270\">This is a demo of mailsort with made-up mail. Nothing here touches a real inbox.</p></div>",
        e.subject, e.snippet
    );
    Ok(("text/html".into(), html))
}

pub async fn trash(_id: &str) -> Result<()> {
    crate::sleep(LATENCY).await;
    Ok(())
}

pub async fn untrash(_id: &str) -> Result<()> {
    crate::sleep(LATENCY).await;
    Ok(())
}

pub async fn delete_forever(ids: &[String]) -> Result<()> {
    crate::sleep(LATENCY).await;
    db::remove(ids);
    Ok(())
}
