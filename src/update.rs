//! Self-update from GitHub Releases: the newest release's .exe replaces the running one, then the app restarts.

use serde::Deserialize;

const LATEST: &str = "https://api.github.com/repos/nikbogman/mailsort/releases/latest";

#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct Release {
    pub tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// "v1.2.3" -> (1, 2, 3); anything unparsable counts as 0.
fn version(v: &str) -> (u64, u64, u64) {
    let mut n = v.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0));
    (n.next().unwrap_or(0), n.next().unwrap_or(0), n.next().unwrap_or(0))
}

fn client() -> reqwest::Client {
    // GitHub's API rejects requests without a User-Agent.
    reqwest::Client::builder().user_agent("mailsort-updater").build().expect("http client")
}

/// The latest release if it's newer than this build. Offline or rate-limited: None, try next start.
pub async fn check() -> Option<Release> {
    let r: Release = client().get(LATEST).send().await.ok()?.error_for_status().ok()?.json().await.ok()?;
    (version(&r.tag_name) > version(env!("CARGO_PKG_VERSION"))).then_some(r)
}

/// Download the release's .exe, swap it in for the running binary, and start it. Only returns on error.
pub async fn install(r: &Release) -> Result<(), String> {
    let asset = r.assets.iter().find(|a| a.name.ends_with(".exe")).ok_or("The release has no .exe")?;
    let fail = |e: reqwest::Error| format!("Update download failed: {e}");
    let bytes = client().get(&asset.browser_download_url).send().await.map_err(fail)?.error_for_status().map_err(fail)?.bytes().await.map_err(fail)?;
    let new = std::env::temp_dir().join("mailsort-update.exe");
    std::fs::write(&new, &bytes).map_err(|e| e.to_string())?;
    self_replace::self_replace(&new).map_err(|e| format!("Couldn't replace the app: {e}"))?;
    let _ = std::fs::remove_file(&new);
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe).spawn().map_err(|e| e.to_string())?;
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::version;

    #[test]
    fn versions_compare_numerically() {
        assert!(version("v0.10.0") > version("0.9.9"));
        assert!(version("v1.0") > version("0.99.0"));
        assert_eq!(version("v0.1.0"), version("0.1.0"));
    }
}
