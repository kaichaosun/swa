mod auth;
mod db;
mod handlers;
mod models;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use rust_embed::Embed;
use tower_http::cors::CorsLayer;

#[derive(Embed)]
#[folder = "ui/"]
struct UiAssets;

#[derive(Parser)]
#[command(name = "swa", about = "Self-hosted website analytics")]
struct Args {
    /// Port for the tracker API
    #[arg(short, long, default_value_t = 3330)]
    port: u16,

    /// Port for the dashboard UI
    #[arg(long, default_value_t = 3331)]
    ui_port: u16,

    /// Path to SQLite database file
    #[arg(short, long, default_value = "./ram.db")]
    db: PathBuf,
}

async fn serve_index() -> impl IntoResponse {
    match UiAssets::get("index.html") {
        Some(content) => Html(String::from_utf8_lossy(&content.data).to_string()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn serve_login() -> impl IntoResponse {
    match UiAssets::get("login.html") {
        Some(content) => Html(String::from_utf8_lossy(&content.data).to_string()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn serve_tracker_js() -> Response {
    match UiAssets::get("tracker.js") {
        Some(content) => (
            [(header::CONTENT_TYPE, "application/javascript")],
            content.data.to_vec(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn serve_asset(axum::extract::Path(path): axum::extract::Path<String>) -> Response {
    match UiAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.to_vec(),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let database = db::Database::open(&args.db).expect("Failed to open database");
    let state: handlers::AppState = Arc::new(database);

    let cors = CorsLayer::new()
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE]);

    // Tracker server (collection endpoints + tracker script for 3rd-party websites)
    let api_app = Router::new()
        .route("/track/event", post(handlers::collect_pageview))
        .route("/track/download", post(handlers::collect_download))
        .route("/tracker.js", get(serve_tracker_js))
        .layer(cors)
        .with_state(state.clone());

    // UI server: protected routes (require auth)
    let protected = Router::new()
        .route("/dash/stats/overview", get(handlers::stats_overview))
        .route("/dash/stats/pageviews", get(handlers::stats_pageviews))
        .route("/dash/stats/pages", get(handlers::stats_pages))
        .route("/dash/stats/referrers", get(handlers::stats_referrers))
        .route("/dash/stats/browsers", get(handlers::stats_browsers))
        .route("/dash/stats/os", get(handlers::stats_os))
        .route("/dash/stats/downloads", get(handlers::stats_downloads))
        .route("/dash/stats/realtime", get(handlers::stats_realtime))
        .route("/dash/stats/domains", get(handlers::list_domains))
        .route("/dash/settings", get(handlers::get_settings))
        .route("/dash/settings", post(handlers::update_settings))
        .route("/", get(serve_index))
        .layer(middleware::from_fn_with_state(state.clone(), auth::require_auth));

    // UI server: public routes (login/register/static assets)
    let ui_app = Router::new()
        .merge(protected)
        .route("/login", get(serve_login))
        .route("/auth/register", post(handlers::register))
        .route("/auth/login", post(handlers::login))
        .route("/auth/logout", post(handlers::logout))
        .route("/{*path}", get(serve_asset))
        .with_state(state);

    let api_addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let ui_addr = SocketAddr::from(([127, 0, 0, 1], args.ui_port));

    println!("SWA API listening on http://{}", api_addr);
    println!("SWA UI  listening on http://{}", ui_addr);
    tracing::info!("SWA API listening on http://{}", api_addr);
    tracing::info!("SWA UI  listening on http://{}", ui_addr);

    let api_listener = tokio::net::TcpListener::bind(api_addr)
        .await
        .expect("Failed to bind API port");
    let ui_listener = tokio::net::TcpListener::bind(ui_addr)
        .await
        .expect("Failed to bind UI port");

    tokio::select! {
        r = axum::serve(api_listener, api_app) => r.expect("API server error"),
        r = axum::serve(ui_listener, ui_app) => r.expect("UI server error"),
    }
}
