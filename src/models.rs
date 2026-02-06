use serde::{Deserialize, Serialize};

// --- Incoming events ---

#[derive(Debug, Deserialize)]
pub struct PageViewEvent {
    pub domain: String,
    pub path: String,
    #[serde(default)]
    pub referrer: String,
    #[serde(default)]
    pub browser: String,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub screen: String,
    #[serde(default)]
    pub visitor_id: String,
}

#[derive(Debug, Deserialize)]
pub struct DownloadEvent {
    pub app_name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub referrer: String,
}

// --- Query parameters ---

#[derive(Debug, Deserialize)]
pub struct DateRange {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Deserialize)]
pub struct DateRangeWithLimit {
    pub from: String,
    pub to: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    10
}

// --- Response structs ---

#[derive(Debug, Serialize)]
pub struct OverviewStats {
    pub total_views: i64,
    pub unique_visitors: i64,
    pub avg_views_per_day: f64,
    pub total_downloads: i64,
}

#[derive(Debug, Serialize)]
pub struct DailyStat {
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct PageStat {
    pub path: String,
    pub views: i64,
    pub unique_visitors: i64,
}

#[derive(Debug, Serialize)]
pub struct ReferrerStat {
    pub referrer: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct BrowserStat {
    pub browser: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct OsStat {
    pub os: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct DownloadDailyStat {
    pub date: String,
    pub app_name: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct DownloadStats {
    pub daily: Vec<DownloadDailyStat>,
    pub by_app: Vec<DownloadAppStat>,
}

#[derive(Debug, Serialize)]
pub struct DownloadAppStat {
    pub app_name: String,
    pub platform: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct RealtimeStats {
    pub active_visitors: i64,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
}
