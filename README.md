# mailsort

Desktop app (Rust + Dioxus) that sorts your Gmail inbox with a local Ollama model. Review categories and AI remove-suggestions, batch trash or delete forever, manage Trash.

## Build & run

```
cargo build --release
target\release\mailsort.exe
```

On launch it starts Ollama if needed and syncs: new inbox mail gets fetched and sorted. Press **Sync** in the sidebar to run it again.

## Setup (one time)

1. Google Cloud: create a project, enable the **Gmail API**, set up the consent screen (External, add yourself as test user), create a **Desktop app** OAuth client and download its JSON.
2. Start the app, click **Open folder**, save the JSON there as `credentials.json`, click **Continue**. Google login opens in the browser.
3. `ollama pull qwen2.5:3b` (other model: set `MAILSORT_MODEL`).

Data lives in `%APPDATA%\mailsort\data` (`credentials.json`, `token.json`, `mail.db`).

## Code

| | |
|---|---|
| `src/classify.rs` | categories, sender rules, the two Ollama prompts (analyze email, summarize group) |
| `src/groups.rs` | emails grouped by organization + topic, verdicts (Keep / Review / Can go) |
| `src/google.rs` | Google login, Gmail calls |
| `src/db.rs` | SQLite cache |
| `src/sync.rs` | inbox sync, Trash listing |
| `src/queue.rs` | trash / restore / delete-forever queue with undo |
| `src/ui/` | Dioxus components; styles in `assets/style.css` |

`cargo test` runs the checks.
