use axum::{
    extract::{MatchedPath, Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{util::header_or_unknown, ApiContext};

pub async fn gate_internal_routes(
    State(ctx): State<ApiContext>,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .cloned()
        .map(|v| v.as_str().to_string())
        .unwrap_or("unknown".to_string());

    let headers = request.headers_mut();
    headers.remove("x-pluralkit-internal");

    if path.starts_with("/internal") {
        let fail_response =
            (StatusCode::FORBIDDEN, r#"{"message":"go away","code":0}"#).into_response();

        if headers.get("X-PluralKit-Client-IP").is_some() {
            return fail_response;
        }

        let authkey = header_or_unknown(headers.get("X-PluralKit-InternalAuth"));
        if authkey != ctx.internal_request_secret {
            return fail_response;
        }

        headers.append("x-pluralkit-internal", HeaderValue::from_static("1"));
    }

    next.run(request).await
}
