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
pub struct ActionEvent {
    pub domain: String,
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub referrer: String,
}

// --- Query parameters ---

#[derive(Debug, Deserialize)]
pub struct DateRange {
    pub from: String,
    pub to: String,
    pub domain: String,
    /// Minutes ahead of UTC (e.g. 480 for UTC+8). Used to group dates by local time.
    #[serde(default)]
    pub tz_offset: i32,
}

#[derive(Debug, Deserialize)]
pub struct DateRangeWithLimit {
    pub from: String,
    pub to: String,
    pub domain: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub tz_offset: i32,
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
    pub total_actions: i64,
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
pub struct ActionDailyStat {
    pub date: String,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct ActionStats {
    pub daily: Vec<ActionDailyStat>,
    pub by_name: Vec<ActionNameStat>,
}

#[derive(Debug, Serialize)]
pub struct ActionNameStat {
    pub name: String,
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct RealtimeStats {
    pub active_visitors: i64,
}

#[derive(Debug, Serialize)]
pub struct DomainInfo {
    pub domain: String,
    pub total_views: i64,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
}

// --- Realtime query ---

#[derive(Debug, Deserialize)]
pub struct RealtimeQuery {
    pub domain: String,
}

// --- Settings ---

#[derive(Debug, Serialize)]
pub struct SettingsResponse {
    pub allow_localhost: bool,
}

#[derive(Debug, Deserialize)]
pub struct SettingsUpdate {
    pub allow_localhost: bool,
}

// --- Auth ---

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub success: bool,
    pub message: String,
}
