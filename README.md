# SWA — Self-hosted Website Analytics

A lightweight, single-binary web analytics tool built with Rust and SQLite. No external dependencies, no cloud services — just deploy and go.

## Features

- **Single binary** — UI, API, and tracker bundled into one executable via `rust-embed`
- **SQLite storage** — WAL mode, zero-config, file-based database
- **Two-port architecture** — separate ports for the public tracker API and the authenticated dashboard
- **Pageview + download tracking** — out of the box
- **Privacy-friendly** — respects Do-Not-Track; daily-rotating fingerprints (no cookies for visitors)
- **Auth-protected dashboard** — cookie-based sessions with argon2 password hashing
- **Dark-themed dashboard** — real-time stats, date ranges, Chart.js graphs, auto-refresh

## Quick Start

```bash
# Build release binary
cargo build --release

# Start (defaults: API on :3330, UI on :3331, DB at ./ram.db)
./target/release/swa

# Or customize
./target/release/swa --port 3330 --ui-port 3331 --db /path/to/analytics.db
```

Open `http://127.0.0.1:3331` to register an account and access the dashboard.

## Integrate the Tracker

Add one script tag to any website you want to track:

```html
<script defer data-api="https://your-server:3330" src="https://your-server:3330/tracker.js"></script>
```

The tracker (~1 KB) automatically collects page path, referrer, browser, OS, and screen size.

### Download Tracking

```html
<a href="app.dmg"
   onclick="navigator.sendBeacon('https://your-server:3330/track/download',
     JSON.stringify({app_name:'MyApp',version:'1.0',platform:'macos'}))">
  Download for macOS
</a>
```

## API Reference

SWA runs two servers:

| Port (default) | Purpose |
|---|---|
| `3330` | **Tracker API** — public, receives events from tracked sites |
| `3331` | **Dashboard UI** — auth-protected, serves the analytics dashboard |

### Collection Endpoints (Tracker API — port 3330)

| Method | Path | Description |
|---|---|---|
| `POST` | `/track/event` | Record a pageview |
| `POST` | `/track/download` | Record a download |
| `GET` | `/tracker.js` | Serve the tracking script |

### Dashboard Endpoints (UI — port 3331, auth required)

| Method | Path | Description |
|---|---|---|
| `GET` | `/dash/stats/overview` | Total views, visitors, bounce rate |
| `GET` | `/dash/stats/pageviews` | Daily pageview time series |
| `GET` | `/dash/stats/pages` | Top pages |
| `GET` | `/dash/stats/referrers` | Top referrers |
| `GET` | `/dash/stats/browsers` | Browser breakdown |
| `GET` | `/dash/stats/os` | OS breakdown |
| `GET` | `/dash/stats/downloads` | Download stats |
| `GET` | `/dash/stats/realtime` | Active visitors in last 5 min |

### Auth Endpoints (UI — port 3331, public)

| Method | Path | Description |
|---|---|---|
| `POST` | `/auth/register` | Create an account |
| `POST` | `/auth/login` | Log in (sets `swa_session` cookie) |
| `POST` | `/auth/logout` | Log out |

## Project Structure

```
src/
  main.rs        # CLI, two-server setup, CORS, embedded asset serving
  db.rs          # SQLite (WAL), schema migrations, query functions
  models.rs      # Request/response types
  handlers.rs    # All API endpoint handlers
  auth.rs        # Cookie-based session middleware
ui/
  tracker.js     # Lightweight tracking script (DNT-aware)
  index.html     # Dashboard SPA
  login.html     # Login / register page
  app.js         # Dashboard logic (charts, tables, date ranges)
  style.css      # Dark-themed responsive styles
```

## Deploy

### Systemd Service

Create `/etc/systemd/system/swa.service`:

```ini
[Unit]
Description=SWA Analytics
After=network.target

[Service]
ExecStart=/opt/swa/swa --port 3330 --ui-port 3331 --db /opt/swa/data/analytics.db
WorkingDirectory=/opt/swa
Restart=always
User=swa

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now swa
```

### Git Push-to-Deploy (optional)

On the remote server, set up a bare repo with a `post-receive` hook:

```bash
mkdir -p ~/repos/swa.git && cd ~/repos/swa.git && git init --bare

cat > hooks/post-receive << 'EOF'
#!/bin/bash
set -e
WORK_DIR=/opt/swa
export PATH="$HOME/.cargo/bin:$PATH"
mkdir -p $WORK_DIR
git --work-tree=$WORK_DIR --git-dir=$HOME/repos/swa.git checkout -f
cd $WORK_DIR && cargo build --release
sudo systemctl restart swa
EOF

chmod +x hooks/post-receive
```

Then push from your local machine:

```bash
git remote add deploy ssh://user@your-server/~/repos/swa.git
git push deploy main
```

> **Note:** Rust must be installed on the server. For resource-constrained servers, cross-compile locally and `scp` the binary instead.

## License

MIT
