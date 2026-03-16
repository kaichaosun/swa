use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;

use crate::handlers::AppState;

pub async fn require_auth(
    State(db): State<AppState>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Response {
    let token = jar.get("swa_session").map(|c| c.value().to_string());

    let authenticated = match token {
        Some(t) => db.validate_session(&t).ok().flatten().is_some(),
        None => false,
    };

    if authenticated {
        next.run(request).await
    } else {
        // JSON API calls get 401, browser navigation gets redirected
        let accept = request
            .headers()
            .get("accept")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if accept.contains("application/json") {
            StatusCode::UNAUTHORIZED.into_response()
        } else {
            Redirect::to("/login").into_response()
        }
    }
}
