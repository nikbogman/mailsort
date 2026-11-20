use std::path::PathBuf;
use std::sync::LazyLock;

/// Where credentials.json, token.json and mail.db live: %APPDATA%\mailsort\data on Windows.
pub static DATA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = directories::ProjectDirs::from("", "", "mailsort")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    dir
});

pub fn db_path() -> PathBuf {
    DATA_DIR.join("mail.db")
}

pub fn credentials_path() -> PathBuf {
    DATA_DIR.join("credentials.json")
}

pub fn token_path() -> PathBuf {
    DATA_DIR.join("token.json")
}

pub fn model() -> String {
    std::env::var("MAILSORT_MODEL").unwrap_or_else(|_| "qwen2.5:3b".into())
}

pub fn ollama_url() -> String {
    std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://127.0.0.1:11434".into())
}

/// Max emails fetched from a Gmail label per sync.
pub const LIMIT: usize = 1000;
