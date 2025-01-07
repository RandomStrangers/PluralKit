use axum::{
    extract::{MatchedPath, Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{ApiContext, util::header_or_unknown};

pub async fn gate_internal_routes(State(ctx): State<ApiContext>, mut request: Request, next: Next) -> Response {
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .cloned()
        .map(|v| v.as_str().to_string())
        .unwrap_or("unknown".to_string());

    let headers = request.headers_mut();
    headers.remove("x-pluralkit-internal");
    let source_ip = header_or_unknown(headers.get("X-PluralKit-Client-IP"));
    let authkey = header_or_unknown(headers.get("X-PluralKit-InternalAuth"));

    if path.starts_with("/internal") {
        let internal_ok = match source_ip {
            "127.0.0.1" => true,
            _ => false,
        };

        if internal_ok && authkey == ctx.internal_request_secret {
            headers.append("x-pluralkit-internal", HeaderValue::from_static("1"));
            return next.run(request).await;
        }

        return
        (
            StatusCode::FORBIDDEN,
            r#"{"message":"go away","code":0}"#,
        )
        .into_response();
    }

    next.run(request).await
}
