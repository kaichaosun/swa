use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::models::*;

pub struct Database {
    conn: Mutex<Connection>,
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

    pub fn get_overview_stats(&self, from: &str, to: &str) -> Result<OverviewStats, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let total_views: i64 = conn.query_row(
            "SELECT COUNT(*) FROM page_views WHERE created_at >= ?1 AND created_at < ?2",
            params![from, to],
            |row| row.get(0),
        )?;

        let unique_visitors: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT visitor_id) FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2 AND visitor_id != ''",
            params![from, to],
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

    pub fn get_pageview_stats(&self, from: &str, to: &str) -> Result<Vec<DailyStat>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT date(created_at) as day, COUNT(*) as count
             FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2
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

    pub fn get_top_pages(&self, from: &str, to: &str, limit: i64) -> Result<Vec<PageStat>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT path, COUNT(*) as views, COUNT(DISTINCT visitor_id) as unique_visitors
             FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2
             GROUP BY path
             ORDER BY views DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![from, to, limit], |row| {
            Ok(PageStat {
                path: row.get(0)?,
                views: row.get(1)?,
                unique_visitors: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_top_referrers(&self, from: &str, to: &str, limit: i64) -> Result<Vec<ReferrerStat>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT referrer, COUNT(*) as count
             FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2 AND referrer != ''
             GROUP BY referrer
             ORDER BY count DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![from, to, limit], |row| {
            Ok(ReferrerStat {
                referrer: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_browser_stats(&self, from: &str, to: &str) -> Result<Vec<BrowserStat>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT browser, COUNT(*) as count
             FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2 AND browser != ''
             GROUP BY browser
             ORDER BY count DESC",
        )?;
        let rows = stmt.query_map(params![from, to], |row| {
            Ok(BrowserStat {
                browser: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_os_stats(&self, from: &str, to: &str) -> Result<Vec<OsStat>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT os, COUNT(*) as count
             FROM page_views
             WHERE created_at >= ?1 AND created_at < ?2 AND os != ''
             GROUP BY os
             ORDER BY count DESC",
        )?;
        let rows = stmt.query_map(params![from, to], |row| {
            Ok(OsStat {
                os: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_download_stats(&self, from: &str, to: &str) -> Result<DownloadStats, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT date(created_at) as day, app_name, COUNT(*) as count
             FROM download_events
             WHERE created_at >= ?1 AND created_at < ?2
             GROUP BY day, app_name
             ORDER BY day",
        )?;
        let daily: Vec<DownloadDailyStat> = stmt
            .query_map(params![from, to], |row| {
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

    pub fn get_realtime_count(&self) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(DISTINCT visitor_id) FROM page_views
             WHERE created_at >= strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-5 minutes')
             AND visitor_id != ''",
            [],
            |row| row.get(0),
        )
    }
}
