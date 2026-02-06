# SWA - Self-hosted Website Analytics

A single-binary web analytics tool with SQLite.

## Usage

Start the server:

```bash
cargo run -- --port 3002

# Or build release binary
cargo build --release
./target/release/swa --port 3002
```

Embed tracker on your site:

```html
<script defer data-api="http://127.0.0.1:3002" src="http://127.0.0.1:3002/tracker.js"></script>
```

Track downloads:

```html
<a href="app.dmg" onclick="navigator.sendBeacon('http://127.0.0.1:3002/api/download', JSON.stringify({app_name:'MyApp',version:'1.0',platform:'macos'}))">Download</a>
```


## Files

| File | Purpose |
|------|---------|
| `Cargo.toml` | Dependencies: axum, tokio, rusqlite (bundled), rust-embed, clap, etc. |
| `src/main.rs` | CLI args, server setup, CORS, embedded UI serving, route mounting |
| `src/db.rs` | SQLite with WAL mode, schema migration, all query functions |
| `src/models.rs` | Request/response structs (PageViewEvent, DownloadEvent, stats types) |
| `src/handlers.rs` | 10 API endpoints (2 collection + 8 dashboard) |
| `ui/tracker.js` | Lightweight tracking script (~1KB) with DNT support, daily fingerprinting |
| `ui/index.html` | Dashboard SPA with Chart.js |
| `ui/style.css` | Dark-themed responsive styles |
| `ui/app.js` | Dashboard logic: date ranges, charts, tables, auto-refresh |

## Manual Tracking

```bash
curl -s "http://127.0.0.1:3002/api/stats/overview?from=2026-02-06&to=2026-02-07"

curl -s -X POST http://127.0.0.1:3002/api/event \
  -H 'Content-Type: application/json' \
  -d '{"domain":"example.com","path":"/test","referrer":"https://google.com","browser":"Chrome","os":"macOS","screen":"1920x1080","visitor_id":"abc123"}'
  
curl -s -X POST http://127.0.0.1:3002/api/download \
  -H 'Content-Type: application/json' \
  -d '{"app_name":"MyApp","version":"1.0","platform":"macos"}'
```
