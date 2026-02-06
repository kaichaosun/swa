use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;

use crate::db::Database;
use crate::models::*;

pub type AppState = Arc<Database>;

pub async fn collect_pageview(
    State(db): State<AppState>,
    Json(event): Json<PageViewEvent>,
) -> StatusCode {
    match db.insert_page_view(&event) {
        Ok(_) => StatusCode::ACCEPTED,
        Err(e) => {
            tracing::error!("Failed to insert page view: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn collect_download(
    State(db): State<AppState>,
    Json(event): Json<DownloadEvent>,
) -> StatusCode {
    match db.insert_download(&event) {
        Ok(_) => StatusCode::ACCEPTED,
        Err(e) => {
            tracing::error!("Failed to insert download: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn stats_overview(
    State(db): State<AppState>,
    Query(range): Query<DateRange>,
) -> Result<Json<ApiResponse<OverviewStats>>, StatusCode> {
    db.get_overview_stats(&range.from, &range.to)
        .map(|data| Json(ApiResponse { data }))
        .map_err(|e| {
            tracing::error!("Failed to get overview stats: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn stats_pageviews(
    State(db): State<AppState>,
    Query(range): Query<DateRange>,
) -> Result<Json<ApiResponse<Vec<DailyStat>>>, StatusCode> {
    db.get_pageview_stats(&range.from, &range.to)
        .map(|data| Json(ApiResponse { data }))
        .map_err(|e| {
            tracing::error!("Failed to get pageview stats: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn stats_pages(
    State(db): State<AppState>,
    Query(range): Query<DateRangeWithLimit>,
) -> Result<Json<ApiResponse<Vec<PageStat>>>, StatusCode> {
    db.get_top_pages(&range.from, &range.to, range.limit)
        .map(|data| Json(ApiResponse { data }))
        .map_err(|e| {
            tracing::error!("Failed to get top pages: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn stats_referrers(
    State(db): State<AppState>,
    Query(range): Query<DateRangeWithLimit>,
) -> Result<Json<ApiResponse<Vec<ReferrerStat>>>, StatusCode> {
    db.get_top_referrers(&range.from, &range.to, range.limit)
        .map(|data| Json(ApiResponse { data }))
        .map_err(|e| {
            tracing::error!("Failed to get top referrers: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn stats_browsers(
    State(db): State<AppState>,
    Query(range): Query<DateRange>,
) -> Result<Json<ApiResponse<Vec<BrowserStat>>>, StatusCode> {
    db.get_browser_stats(&range.from, &range.to)
        .map(|data| Json(ApiResponse { data }))
        .map_err(|e| {
            tracing::error!("Failed to get browser stats: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn stats_os(
    State(db): State<AppState>,
    Query(range): Query<DateRange>,
) -> Result<Json<ApiResponse<Vec<OsStat>>>, StatusCode> {
    db.get_os_stats(&range.from, &range.to)
        .map(|data| Json(ApiResponse { data }))
        .map_err(|e| {
            tracing::error!("Failed to get OS stats: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn stats_downloads(
    State(db): State<AppState>,
    Query(range): Query<DateRange>,
) -> Result<Json<ApiResponse<DownloadStats>>, StatusCode> {
    db.get_download_stats(&range.from, &range.to)
        .map(|data| Json(ApiResponse { data }))
        .map_err(|e| {
            tracing::error!("Failed to get download stats: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn stats_realtime(
    State(db): State<AppState>,
) -> Result<Json<ApiResponse<RealtimeStats>>, StatusCode> {
    db.get_realtime_count()
        .map(|count| {
            Json(ApiResponse {
                data: RealtimeStats {
                    active_visitors: count,
                },
            })
        })
        .map_err(|e| {
            tracing::error!("Failed to get realtime stats: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
