mod db;
mod handlers;
mod models;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::http::{header, HeaderValue, Method, StatusCode};
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
#[command(name = "ram", about = "Self-hosted website analytics")]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value_t = 3000)]
    port: u16,

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

    let app = Router::new()
        // Collection endpoints
        .route("/api/event", post(handlers::collect_pageview))
        .route("/api/download", post(handlers::collect_download))
        // Dashboard API
        .route("/api/stats/overview", get(handlers::stats_overview))
        .route("/api/stats/pageviews", get(handlers::stats_pageviews))
        .route("/api/stats/pages", get(handlers::stats_pages))
        .route("/api/stats/referrers", get(handlers::stats_referrers))
        .route("/api/stats/browsers", get(handlers::stats_browsers))
        .route("/api/stats/os", get(handlers::stats_os))
        .route("/api/stats/downloads", get(handlers::stats_downloads))
        .route("/api/stats/realtime", get(handlers::stats_realtime))
        // Embedded UI
        .route("/", get(serve_index))
        .route("/{*path}", get(serve_asset))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    tracing::info!("RWA listening on http://{}", addr);
    println!("RWA listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind");
    axum::serve(listener, app).await.expect("Server error");
}
