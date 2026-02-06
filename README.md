# Web Analytics and Monitoring in Rust

RWA - Rust Website Analytics is fully implemented as a single-binary web analytics tool.

Files created
┌─────────────────┬───────────────────────────────────────────────────────────────────────────┐
│      File       │                                  Purpose                                  │
├─────────────────┼───────────────────────────────────────────────────────────────────────────┤
│ Cargo.toml      │ Dependencies: axum, tokio, rusqlite (bundled), rust-embed, clap, etc.     │
├─────────────────┼───────────────────────────────────────────────────────────────────────────┤
│ src/main.rs     │ CLI args, server setup, CORS, embedded UI serving, route mounting         │
├─────────────────┼───────────────────────────────────────────────────────────────────────────┤
│ src/db.rs       │ SQLite with WAL mode, schema migration, all query functions               │
├─────────────────┼───────────────────────────────────────────────────────────────────────────┤
│ src/models.rs   │ Request/response structs (PageViewEvent, DownloadEvent, stats types)      │
├─────────────────┼───────────────────────────────────────────────────────────────────────────┤
│ src/handlers.rs │ 10 API endpoints (2 collection + 8 dashboard)                             │
├─────────────────┼───────────────────────────────────────────────────────────────────────────┤
│ ui/tracker.js   │ Lightweight tracking script (~1KB) with DNT support, daily fingerprinting │
├─────────────────┼───────────────────────────────────────────────────────────────────────────┤
│ ui/index.html   │ Dashboard SPA with Chart.js                                               │
├─────────────────┼───────────────────────────────────────────────────────────────────────────┤
│ ui/style.css    │ Dark-themed responsive styles                                             │
├─────────────────┼───────────────────────────────────────────────────────────────────────────┤
│ ui/app.js       │ Dashboard logic: date ranges, charts, tables, auto-refresh                │
└─────────────────┴───────────────────────────────────────────────────────────────────────────┘

Verified,

- cargo build --release compiles cleanly
- Server starts and binds to 127.0.0.1:3000
- POST /api/event and POST /api/download accept events (202 Accepted)
- All stats APIs return correct data
- Dashboard HTML loads at / (200)
- tracker.js served at /tracker.js (200)

## Usage

Start the server,
```bash
cargo build --release
./target/release/rwa --port 3000
```

Embed tracker on your site
```html
<script defer data-api="https://your-host:3000" src="https://your-host:3000/tracker.js"></script>
````

Track downloads

```html
<a href="app.dmg" onclick="navigator.sendBeacon('https://your-host:3000/api/download', JSON.stringify({app_name:'MyApp',version:'1.0',platform:'macos'}))">Download</a>`
```
