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

## Deploy

Here's a git-push-to-deploy setup for a Rust binary:

On the remote server

1. Create a bare repo:

```sh
mkdir -p ~/repos/swa.git
cd ~/repos/swa.git
git init --bare
```

2. Create the post-receive hook (~/repos/swa.git/hooks/post-receive):

```sh
#!/bin/bash
set -e

WORK_DIR=/opt/swa
export PATH="$HOME/.cargo/bin:$PATH"

# Checkout latest code
mkdir -p $WORK_DIR
git --work-tree=$WORK_DIR --git-dir=$HOME/repos/swa.git checkout -f

# Build release binary
cd $WORK_DIR
cargo build --release

# Restart the service
sudo systemctl restart swa
```

chmod +x ~/repos/swa.git/hooks/post-receive


3. Create a systemd service (/etc/systemd/system/swa.service):

```sh
[Unit]
Description=SWA Analytics
After=network.target

[Service]
ExecStart=/opt/swa/target/release/swa --port 3330 --ui-port 3331 --db /opt/swa/data/ram.db
WorkingDirectory=/opt/swa
Restart=always
User=youruser

[Install]
WantedBy=multi-user.target

```

sudo systemctl daemon-reload
sudo systemctl enable swa

On your local machine

Add the remote and push:
git remote add deploy ssh://user@your-server/~/repos/swa.git
git push deploy main

Each git push deploy main will trigger the hook to build and restart the service.

Note: Rust must be installed on the server (curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh). If the server is resource-constrained, consider
cross-compiling locally and using scp instead.
