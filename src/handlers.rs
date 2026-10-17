use std::net::IpAddr;
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

/// Strip an optional port and IPv6 brackets: "[::1]:8080" -> "::1", "example.com:80" -> "example.com".
fn host_only(domain: &str) -> &str {
    let domain = domain.trim();
    if let Some(rest) = domain.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }
    // A bare IPv6 address has several colons; only strip a port from host:port
    if domain.matches(':').count() == 1 {
        return domain.split(':').next().unwrap_or(domain);
    }
    domain
}

/// True for loopback, private-network (LAN) and link-local hosts, i.e. anything
/// that is a dev machine rather than a public site.
fn is_local_host(domain: &str) -> bool {
    let host = host_only(domain).trim_end_matches('.');
    if host.is_empty() {
        return true;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => {
                v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
            }
            IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6.is_unspecified()
                    || v6.is_unique_local()
                    || v6.is_unicast_link_local()
                    || v6.to_ipv4_mapped().is_some_and(|v4| is_local_host(&v4.to_string()))
            }
        };
    }
    let host = host.to_ascii_lowercase();
    host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local")
}

/// Normalize a user-entered site into a bare lowercase host, e.g.
/// "https://WWW.Example.com:443/path" -> "www.example.com". Keeps a leading "*." wildcard.
fn normalize_site(input: &str) -> String {
    let s = input.trim();
    let s = s.split_once("://").map(|(_, rest)| rest).unwrap_or(s);
    let s = s.split(['/', '?', '#']).next().unwrap_or(s);
    host_only(s).trim_end_matches('.').to_ascii_lowercase()
}

/// An entry matches the exact host, or with a "*." prefix, any subdomain of it.
fn site_matches(entry: &str, host: &str) -> bool {
    match entry.strip_prefix("*.") {
        Some(base) => host.ends_with(&format!(".{}", base)),
        None => entry == host,
    }
}

fn get_allowed_sites(db: &Database) -> Result<Vec<String>, rusqlite::Error> {
    Ok(db
        .get_setting("allowed_sites")?
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default())
}

/// Whether an incoming event for `domain` should be stored. Local hosts are governed
/// by the allow_localhost toggle; public hosts by the allowed-sites list (empty = all).
fn accepts_domain(db: &Database, domain: &str) -> bool {
    if is_local_host(domain) {
        return db.get_setting("allow_localhost").ok().flatten().as_deref() == Some("true");
    }
    let allowed = get_allowed_sites(db).unwrap_or_default();
    if allowed.is_empty() {
        return true;
    }
    let host = normalize_site(domain);
    allowed.iter().any(|entry| site_matches(entry, &host))
}

pub async fn collect_pageview(State(db): State<AppState>, body: Bytes) -> StatusCode {
    let event: PageViewEvent = match serde_json::from_slice(&body) {
        Ok(e) => e,
        Err(_) => return StatusCode::BAD_REQUEST,
    };
    if !accepts_domain(&db, &event.domain) {
        return StatusCode::ACCEPTED;
    }
    match db.insert_page_view(&event) {
        Ok(_) => StatusCode::ACCEPTED,
        Err(e) => {
            tracing::error!("Failed to insert page view: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn collect_action(State(db): State<AppState>, body: Bytes) -> StatusCode {
    let event: ActionEvent = match serde_json::from_slice(&body) {
        Ok(e) => e,
        Err(_) => return StatusCode::BAD_REQUEST,
    };
    if !accepts_domain(&db, &event.domain) {
        return StatusCode::ACCEPTED;
    }
    match db.insert_action(&event) {
        Ok(_) => StatusCode::ACCEPTED,
        Err(e) => {
            tracing::error!("Failed to insert action: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn list_domains(
    State(db): State<AppState>,
) -> Result<Json<ApiResponse<Vec<DomainInfo>>>, StatusCode> {
    db.get_domains()
        .map(|data| Json(ApiResponse { data }))
        .map_err(|e| {
            tracing::error!("Failed to get domains: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn stats_overview(
    State(db): State<AppState>,
    Query(range): Query<DateRange>,
) -> Result<Json<ApiResponse<OverviewStats>>, StatusCode> {
    db.get_overview_stats(&range.from, &range.to, &range.domain)
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
    db.get_pageview_stats(&range.from, &range.to, range.tz_offset, &range.domain)
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
    db.get_top_pages(&range.from, &range.to, range.limit, &range.domain)
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
    db.get_top_referrers(&range.from, &range.to, range.limit, &range.domain)
        .map(|data| Json(ApiResponse { data }))
        .map_err(|e| {
            tracing::error!("Failed to get top referrers: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn stats_campaigns(
    State(db): State<AppState>,
    Query(range): Query<DateRangeWithLimit>,
) -> Result<Json<ApiResponse<Vec<CampaignStat>>>, StatusCode> {
    db.get_top_campaigns(&range.from, &range.to, range.limit, &range.domain)
        .map(|data| Json(ApiResponse { data }))
        .map_err(|e| {
            tracing::error!("Failed to get top campaigns: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn stats_browsers(
    State(db): State<AppState>,
    Query(range): Query<DateRange>,
) -> Result<Json<ApiResponse<Vec<BrowserStat>>>, StatusCode> {
    db.get_browser_stats(&range.from, &range.to, &range.domain)
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
    db.get_os_stats(&range.from, &range.to, &range.domain)
        .map(|data| Json(ApiResponse { data }))
        .map_err(|e| {
            tracing::error!("Failed to get OS stats: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn stats_actions(
    State(db): State<AppState>,
    Query(range): Query<DateRange>,
) -> Result<Json<ApiResponse<ActionStats>>, StatusCode> {
    db.get_action_stats(&range.from, &range.to, range.tz_offset, &range.domain)
        .map(|data| Json(ApiResponse { data }))
        .map_err(|e| {
            tracing::error!("Failed to get action stats: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn stats_realtime(
    State(db): State<AppState>,
    Query(q): Query<RealtimeQuery>,
) -> Result<Json<ApiResponse<RealtimeStats>>, StatusCode> {
    db.get_realtime_count(&q.domain)
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

// --- Settings handlers ---

pub async fn get_settings(
    State(db): State<AppState>,
) -> Result<Json<ApiResponse<SettingsResponse>>, StatusCode> {
    let load = || -> Result<SettingsResponse, rusqlite::Error> {
        Ok(SettingsResponse {
            allow_localhost: db.get_setting("allow_localhost")?.as_deref() == Some("true"),
            allowed_sites: get_allowed_sites(&db)?,
        })
    };
    load().map(|data| Json(ApiResponse { data })).map_err(|e| {
        tracing::error!("Failed to get settings: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

pub async fn update_settings(State(db): State<AppState>, body: Bytes) -> impl IntoResponse {
    let req: SettingsUpdate = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthResponse {
                    success: false,
                    message: "Invalid request".into(),
                }),
            )
        }
    };

    let save = || -> Result<(), rusqlite::Error> {
        if let Some(allow) = req.allow_localhost {
            db.set_setting("allow_localhost", if allow { "true" } else { "false" })?;
        }
        if let Some(sites) = &req.allowed_sites {
            let mut normalized: Vec<String> = sites
                .iter()
                .map(|s| normalize_site(s))
                .filter(|s| !s.is_empty() && s != "*.")
                .collect();
            normalized.sort();
            normalized.dedup();
            let json = serde_json::to_string(&normalized).unwrap_or_else(|_| "[]".into());
            db.set_setting("allowed_sites", &json)?;
        }
        Ok(())
    };
    match save() {
        Ok(_) => (
            StatusCode::OK,
            Json(AuthResponse {
                success: true,
                message: "Settings updated".into(),
            }),
        ),
        Err(e) => {
            tracing::error!("Failed to update settings: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthResponse {
                    success: false,
                    message: "Server error".into(),
                }),
            )
        }
    }
}

pub async fn delete_domain_data(State(db): State<AppState>, body: Bytes) -> impl IntoResponse {
    let req: DeleteDomainRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthResponse {
                    success: false,
                    message: "Invalid request".into(),
                }),
            )
        }
    };

    if req.domain.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthResponse {
                success: false,
                message: "Domain is required".into(),
            }),
        );
    }

    match db.delete_domain_data(&req.domain) {
        Ok(n) => (
            StatusCode::OK,
            Json(AuthResponse {
                success: true,
                message: format!("Deleted {} rows for {}", n, req.domain),
            }),
        ),
        Err(e) => {
            tracing::error!("Failed to delete domain data: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthResponse {
                    success: false,
                    message: "Server error".into(),
                }),
            )
        }
    }
}

// --- Auth handlers ---

pub async fn register(State(db): State<AppState>, body: Bytes) -> impl IntoResponse {
    let req: RegisterRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthResponse {
                    success: false,
                    message: "Invalid request".into(),
                }),
            )
        }
    };

    if req.email.is_empty() || !req.email.contains('@') {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthResponse {
                success: false,
                message: "Invalid email".into(),
            }),
        );
    }

    if req.password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthResponse {
                success: false,
                message: "Password must be at least 8 characters".into(),
            }),
        );
    }

    match db.count_users() {
        Ok(count) if count >= 1 => {
            return (
                StatusCode::FORBIDDEN,
                Json(AuthResponse {
                    success: false,
                    message: "Registration is closed".into(),
                }),
            );
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthResponse {
                    success: false,
                    message: "Server error".into(),
                }),
            );
        }
        _ => {}
    }

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = match argon2.hash_password(req.password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthResponse {
                    success: false,
                    message: "Server error".into(),
                }),
            )
        }
    };

    match db.create_user(&req.email, &password_hash) {
        Ok(_) => (
            StatusCode::CREATED,
            Json(AuthResponse {
                success: true,
                message: "Account created".into(),
            }),
        ),
        Err(_) => (
            StatusCode::CONFLICT,
            Json(AuthResponse {
                success: false,
                message: "Email already registered".into(),
            }),
        ),
    }
}

pub async fn login(State(db): State<AppState>, jar: CookieJar, body: Bytes) -> Response {
    let req: LoginRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthResponse {
                    success: false,
                    message: "Invalid request".into(),
                }),
            )
                .into_response()
        }
    };

    let (user_id, _, password_hash) = match db.get_user_by_email(&req.email) {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(AuthResponse {
                    success: false,
                    message: "Invalid email or password".into(),
                }),
            )
                .into_response()
        }
    };

    let parsed_hash = match PasswordHash::new(&password_hash) {
        Ok(h) => h,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthResponse {
                    success: false,
                    message: "Server error".into(),
                }),
            )
                .into_response()
        }
    };

    if Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthResponse {
                success: false,
                message: "Invalid email or password".into(),
            }),
        )
            .into_response();
    }

    // Generate session token
    let mut token_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut token_bytes);
    let token = hex::encode(token_bytes);

    let expires_at = (Utc::now() + chrono::Duration::days(7))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    if db.create_session(&token, user_id, &expires_at).is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthResponse {
                success: false,
                message: "Server error".into(),
            }),
        )
            .into_response();
    }

    let cookie = format!(
        "swa_session={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=604800",
        token
    );
    let jar = jar.add(axum_extra::extract::cookie::Cookie::parse(cookie).unwrap());

    (
        jar,
        Json(AuthResponse {
            success: true,
            message: "Logged in".into(),
        }),
    )
        .into_response()
}

pub async fn logout(State(db): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(cookie) = jar.get("swa_session") {
        let _ = db.delete_session(cookie.value());
    }
    let jar = jar.remove(axum_extra::extract::cookie::Cookie::from("swa_session"));
    (
        jar,
        Json(AuthResponse {
            success: true,
            message: "Logged out".into(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_hosts() {
        for h in [
            "", "localhost", "LOCALHOST:3000", "app.localhost", "my-mac.local",
            "127.0.0.1", "127.0.0.2:8080", "0.0.0.0", "10.0.0.5", "172.16.0.1",
            "172.31.255.255", "192.168.1.20:5173", "169.254.1.1",
            "[::1]", "[::1]:3000", "::1", "fd12:3456::1", "fe80::1", "[::ffff:192.168.0.1]",
        ] {
            assert!(is_local_host(h), "{h} should be local");
        }
        for h in [
            "example.com", "www.example.com:443", "8.8.8.8", "172.32.0.1",
            "192.169.0.1", "[2001:db8::1]", "localhost.example.com",
        ] {
            assert!(!is_local_host(h), "{h} should not be local");
        }
    }

    #[test]
    fn normalizes_sites() {
        assert_eq!(normalize_site(" https://WWW.Example.com:443/path?q=1 "), "www.example.com");
        assert_eq!(normalize_site("example.com."), "example.com");
        assert_eq!(normalize_site("*.Example.com"), "*.example.com");
    }

    #[test]
    fn matches_sites() {
        assert!(site_matches("example.com", "example.com"));
        assert!(!site_matches("example.com", "www.example.com"));
        assert!(site_matches("*.example.com", "www.example.com"));
        assert!(site_matches("*.example.com", "a.b.example.com"));
        assert!(!site_matches("*.example.com", "example.com"));
        assert!(!site_matches("*.example.com", "badexample.com"));
    }
}
