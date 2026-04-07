use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::models::*;

pub struct Database {
    conn: Mutex<Connection>,
}

/// Returns a SQL fragment and parameter value for optional domain filtering.
/// When domain is Some, returns (" AND domain = ?N", domain_value).
/// When None, returns ("", "%") — the param is unused but keeps parameter indices stable.
fn domain_clause(domain: Option<&str>) -> (&'static str, String) {
    match domain {
        Some(d) => (" AND domain = ?3", d.to_string()),
        None => ("", "%".to_string()),
    }
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        let db = Database {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS page_views (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                domain TEXT NOT NULL,
                path TEXT NOT NULL,
                referrer TEXT NOT NULL DEFAULT '',
                browser TEXT NOT NULL DEFAULT '',
                os TEXT NOT NULL DEFAULT '',
                screen TEXT NOT NULL DEFAULT '',
                visitor_id TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_page_views_created_at ON page_views(created_at);
            CREATE INDEX IF NOT EXISTS idx_page_views_domain ON page_views(domain);
            CREATE INDEX IF NOT EXISTS idx_page_views_visitor_id ON page_views(visitor_id);

            CREATE TABLE IF NOT EXISTS download_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                app_name TEXT NOT NULL,
                version TEXT NOT NULL DEFAULT '',
                platform TEXT NOT NULL DEFAULT '',
                referrer TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_download_events_created_at ON download_events(created_at);
            CREATE INDEX IF NOT EXISTS idx_download_events_app_name ON download_events(app_name);

            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE TABLE IF NOT EXISTS sessions (
                token TEXT PRIMARY KEY,
                user_id INTEGER NOT NULL REFERENCES users(id),
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                expires_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    pub fn insert_page_view(&self, event: &PageViewEvent) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO page_views (domain, path, referrer, browser, os, screen, visitor_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.domain,
                event.path,
                event.referrer,
                event.browser,
                event.os,
                event.screen,
                event.visitor_id,
            ],
        )?;
        Ok(())
    }

    pub fn insert_download(&self, event: &DownloadEvent) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO download_events (app_name, version, platform, referrer)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                event.app_name,
                event.version,
                event.platform,
                event.referrer,
            ],
        )?;
        Ok(())
    }

    pub fn get_domains(&self) -> Result<Vec<DomainInfo>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT domain, COUNT(*) as total_views
             FROM page_views
             GROUP BY domain
             ORDER BY total_views DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(DomainInfo {
                domain: row.get(0)?,
                total_views: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_overview_stats(&self, from: &str, to: &str, domain: Option<&str>) -> Result<OverviewStats, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let (domain_filter, domain_param) = domain_clause(domain);

        let total_views: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM page_views WHERE created_at >= ?1 AND created_at < ?2{}", domain_filter),
            params![from, to, domain_param],
            |row| row.get(0),
        )?;

        let unique_visitors: i64 = conn.query_row(
            &format!("SELECT COUNT(DISTINCT visitor_id) FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2 AND visitor_id != ''{}", domain_filter),
            params![from, to, domain_param],
            |row| row.get(0),
        )?;

        let days: f64 = conn.query_row(
            "SELECT MAX(1, CAST(julianday(?2) - julianday(?1) AS REAL)) as days",
            params![from, to],
            |row| row.get(0),
        )?;

        let avg_views_per_day = total_views as f64 / days;

        let total_downloads: i64 = conn.query_row(
            "SELECT COUNT(*) FROM download_events WHERE created_at >= ?1 AND created_at < ?2",
            params![from, to],
            |row| row.get(0),
        )?;

        Ok(OverviewStats {
            total_views,
            unique_visitors,
            avg_views_per_day,
            total_downloads,
        })
    }

    pub fn get_pageview_stats(&self, from: &str, to: &str, tz_offset: i32, domain: Option<&str>) -> Result<Vec<DailyStat>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let tz_modifier = format!("{:+} minutes", tz_offset);
        let domain_filter = if domain.is_some() { " AND domain = ?4" } else { "" };
        let domain_val = domain.unwrap_or("");
        let mut stmt = conn.prepare(
            &format!("SELECT date(created_at, ?3) as day, COUNT(*) as count
             FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2{}
             GROUP BY day
             ORDER BY day", domain_filter),
        )?;
        let rows = stmt.query_map(params![from, to, tz_modifier, domain_val], |row| {
            Ok(DailyStat {
                date: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_top_pages(&self, from: &str, to: &str, limit: i64, domain: Option<&str>) -> Result<Vec<PageStat>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let domain_filter = if domain.is_some() { " AND domain = ?4" } else { "" };
        let domain_val = domain.unwrap_or("");
        let mut stmt = conn.prepare(
            &format!("SELECT path, COUNT(*) as views, COUNT(DISTINCT visitor_id) as unique_visitors
             FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2{}
             GROUP BY path
             ORDER BY views DESC
             LIMIT ?3", domain_filter),
        )?;
        let rows = stmt.query_map(params![from, to, limit, domain_val], |row| {
            Ok(PageStat {
                path: row.get(0)?,
                views: row.get(1)?,
                unique_visitors: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_top_referrers(&self, from: &str, to: &str, limit: i64, domain: Option<&str>) -> Result<Vec<ReferrerStat>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let domain_filter = if domain.is_some() { " AND domain = ?4" } else { "" };
        let domain_val = domain.unwrap_or("");
        let mut stmt = conn.prepare(
            &format!("SELECT referrer, COUNT(*) as count
             FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2 AND referrer != ''{}
             GROUP BY referrer
             ORDER BY count DESC
             LIMIT ?3", domain_filter),
        )?;
        let rows = stmt.query_map(params![from, to, limit, domain_val], |row| {
            Ok(ReferrerStat {
                referrer: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_browser_stats(&self, from: &str, to: &str, domain: Option<&str>) -> Result<Vec<BrowserStat>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let (domain_filter, domain_param) = domain_clause(domain);
        let mut stmt = conn.prepare(
            &format!("SELECT browser, COUNT(*) as count
             FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2 AND browser != ''{}
             GROUP BY browser
             ORDER BY count DESC", domain_filter),
        )?;
        let rows = stmt.query_map(params![from, to, domain_param], |row| {
            Ok(BrowserStat {
                browser: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_os_stats(&self, from: &str, to: &str, domain: Option<&str>) -> Result<Vec<OsStat>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let (domain_filter, domain_param) = domain_clause(domain);
        let mut stmt = conn.prepare(
            &format!("SELECT os, COUNT(*) as count
             FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2 AND os != ''{}
             GROUP BY os
             ORDER BY count DESC", domain_filter),
        )?;
        let rows = stmt.query_map(params![from, to, domain_param], |row| {
            Ok(OsStat {
                os: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_download_stats(&self, from: &str, to: &str, tz_offset: i32) -> Result<DownloadStats, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let tz_modifier = format!("{:+} minutes", tz_offset);

        let mut stmt = conn.prepare(
            "SELECT date(created_at, ?3) as day, app_name, COUNT(*) as count
             FROM download_events
             WHERE created_at >= ?1 AND created_at < ?2
             GROUP BY day, app_name
             ORDER BY day",
        )?;
        let daily: Vec<DownloadDailyStat> = stmt
            .query_map(params![from, to, tz_modifier], |row| {
                Ok(DownloadDailyStat {
                    date: row.get(0)?,
                    app_name: row.get(1)?,
                    count: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut stmt2 = conn.prepare(
            "SELECT app_name, platform, COUNT(*) as count
             FROM download_events
             WHERE created_at >= ?1 AND created_at < ?2
             GROUP BY app_name, platform
             ORDER BY count DESC",
        )?;
        let by_app: Vec<DownloadAppStat> = stmt2
            .query_map(params![from, to], |row| {
                Ok(DownloadAppStat {
                    app_name: row.get(0)?,
                    platform: row.get(1)?,
                    count: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(DownloadStats { daily, by_app })
    }

    pub fn get_unique_visitors(&self, from: &str, to: &str) -> Result<Vec<DailyStat>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT date(created_at) as day, COUNT(DISTINCT visitor_id) as count
             FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2 AND visitor_id != ''
             GROUP BY day
             ORDER BY day",
        )?;
        let rows = stmt.query_map(params![from, to], |row| {
            Ok(DailyStat {
                date: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    // --- Auth methods ---

    pub fn count_users(&self) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
    }

    pub fn create_user(&self, email: &str, password_hash: &str) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO users (email, password_hash) VALUES (?1, ?2)",
            params![email, password_hash],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_user_by_email(&self, email: &str) -> Result<Option<(i64, String, String)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, email, password_hash FROM users WHERE email = ?1")?;
        let mut rows = stmt.query(params![email])?;
        match rows.next()? {
            Some(row) => Ok(Some((row.get(0)?, row.get(1)?, row.get(2)?))),
            None => Ok(None),
        }
    }

    pub fn create_session(&self, token: &str, user_id: i64, expires_at: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions (token, user_id, expires_at) VALUES (?1, ?2, ?3)",
            params![token, user_id, expires_at],
        )?;
        Ok(())
    }

    pub fn validate_session(&self, token: &str) -> Result<Option<i64>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT user_id FROM sessions WHERE token = ?1 AND expires_at > strftime('%Y-%m-%dT%H:%M:%SZ', 'now')"
        )?;
        let mut rows = stmt.query(params![token])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    pub fn delete_session(&self, token: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sessions WHERE token = ?1", params![token])?;
        Ok(())
    }

    // --- Settings methods ---

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_realtime_count(&self, domain: Option<&str>) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        match domain {
            Some(d) => conn.query_row(
                "SELECT COUNT(DISTINCT visitor_id) FROM page_views
                 WHERE created_at >= strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-5 minutes')
                 AND visitor_id != '' AND domain = ?1",
                params![d],
                |row| row.get(0),
            ),
            None => conn.query_row(
                "SELECT COUNT(DISTINCT visitor_id) FROM page_views
                 WHERE created_at >= strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-5 minutes')
                 AND visitor_id != ''",
                [],
                |row| row.get(0),
            ),
        }
    }
}
