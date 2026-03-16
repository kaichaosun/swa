use std::sync::Arc;

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum_extra::extract::CookieJar;
use chrono::Utc;
use rand::RngCore;

use crate::db::Database;
use crate::models::*;

pub type AppState = Arc<Database>;

pub async fn collect_pageview(
    State(db): State<AppState>,
    body: Bytes,
) -> StatusCode {
    let event: PageViewEvent = match serde_json::from_slice(&body) {
        Ok(e) => e,
        Err(_) => return StatusCode::BAD_REQUEST,
    };
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
    body: Bytes,
) -> StatusCode {
    let event: DownloadEvent = match serde_json::from_slice(&body) {
        Ok(e) => e,
        Err(_) => return StatusCode::BAD_REQUEST,
    };
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

// --- Auth handlers ---

pub async fn register(
    State(db): State<AppState>,
    body: Bytes,
) -> impl IntoResponse {
    let req: RegisterRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(AuthResponse {
            success: false, message: "Invalid request".into(),
        })),
    };

    if req.email.is_empty() || !req.email.contains('@') {
        return (StatusCode::BAD_REQUEST, Json(AuthResponse {
            success: false, message: "Invalid email".into(),
        }));
    }

    if req.password.len() < 8 {
        return (StatusCode::BAD_REQUEST, Json(AuthResponse {
            success: false, message: "Password must be at least 8 characters".into(),
        }));
    }

    match db.count_users() {
        Ok(count) if count >= 1 => {
            return (StatusCode::FORBIDDEN, Json(AuthResponse {
                success: false, message: "Registration is closed".into(),
            }));
        }
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthResponse {
                success: false, message: "Server error".into(),
            }));
        }
        _ => {}
    }

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = match argon2.hash_password(req.password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthResponse {
            success: false, message: "Server error".into(),
        })),
    };

    match db.create_user(&req.email, &password_hash) {
        Ok(_) => (StatusCode::CREATED, Json(AuthResponse {
            success: true, message: "Account created".into(),
        })),
        Err(_) => (StatusCode::CONFLICT, Json(AuthResponse {
            success: false, message: "Email already registered".into(),
        })),
    }
}

pub async fn login(
    State(db): State<AppState>,
    jar: CookieJar,
    body: Bytes,
) -> Response {
    let req: LoginRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(AuthResponse {
            success: false, message: "Invalid request".into(),
        })).into_response(),
    };

    let (user_id, _, password_hash) = match db.get_user_by_email(&req.email) {
        Ok(Some(u)) => u,
        _ => return (StatusCode::UNAUTHORIZED, Json(AuthResponse {
            success: false, message: "Invalid email or password".into(),
        })).into_response(),
    };

    let parsed_hash = match PasswordHash::new(&password_hash) {
        Ok(h) => h,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthResponse {
            success: false, message: "Server error".into(),
        })).into_response(),
    };

    if Argon2::default().verify_password(req.password.as_bytes(), &parsed_hash).is_err() {
        return (StatusCode::UNAUTHORIZED, Json(AuthResponse {
            success: false, message: "Invalid email or password".into(),
        })).into_response();
    }

    // Generate session token
    let mut token_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut token_bytes);
    let token = hex::encode(token_bytes);

    let expires_at = (Utc::now() + chrono::Duration::days(7))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    if db.create_session(&token, user_id, &expires_at).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthResponse {
            success: false, message: "Server error".into(),
        })).into_response();
    }

    let cookie = format!(
        "swa_session={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=604800",
        token
    );
    let jar = jar.add(axum_extra::extract::cookie::Cookie::parse(cookie).unwrap());

    (jar, Json(AuthResponse {
        success: true, message: "Logged in".into(),
    })).into_response()
}

pub async fn logout(
    State(db): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    if let Some(cookie) = jar.get("swa_session") {
        let _ = db.delete_session(cookie.value());
    }
    let jar = jar.remove(axum_extra::extract::cookie::Cookie::from("swa_session"));
    (jar, Json(AuthResponse {
        success: true, message: "Logged out".into(),
    }))
}
