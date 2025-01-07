use axum::{
    extract::{Request, MatchedPath, State},
    http::{Method, HeaderValue},
    middleware::Next,
    response::Response,
};
use pluralkit_models::{PKApiKey, ApiKeyType};
use sqlx::Postgres;
use tracing::error;

use crate::ApiContext;

use super::logger::DID_AUTHENTICATE_HEADER;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum ApiKeyAccess {
    None = 0,
    PublicRead,
    PrivateRead,
    Full,
}

impl ApiKeyAccess {
    pub fn privacy_level(&self) -> String {
        match self {
            Self::None | Self::PublicRead => "public".into(),
            Self::PrivateRead | Self::Full => "private".into(),
        }
    }
}

pub fn is_part_path<'a, 'b>(part: &'a str, endpoint: &'b str) -> bool {
    if !endpoint.starts_with("/v2/") {
        return false;
    }

    let path_frags = endpoint[4..].split("/").collect::<Vec<&str>>();
    match part {
        "system" => match &path_frags[..] {
            ["systems", _] => true,
            ["systems", _, "settings"] => true,
            ["systems", _, "autoproxy"] => true,
            ["systems", _, "guilds", ..] => true,
            _ => false,
        },
        "members" => match &path_frags[..] {
            ["systems", _, "members"] => true,
            ["members"] => true,
            ["members", _, "groups"] => false,
            ["members", _, "groups", ..] => false,
            ["members", ..] => true,
            _ => false,
        },
        "groups" => match &path_frags[..] {
            ["systems", _, "groups"] => true,
            ["groups"] => true,
            ["groups", ..] => true,
            ["members", _, "groups"] => true,
            ["members", _, "groups", ..] => true,
            _ => false,
        },
        "switches" => match &path_frags[..] {
            ["systems", _, "fronters"] => true,
            ["systems", _, "switches"] => true,
            ["systems", _, "switches", ..] => true,
            _ => false,
        },
        _ => false,
    }
}

pub fn apikey_can_access(token: &PKApiKey, method: Method, endpoint: String) -> ApiKeyAccess {
    if token.kind == ApiKeyType::Dashboard {
        return ApiKeyAccess::Full;
    }

    let mut access = ApiKeyAccess::None;
    for rscope in token.scopes.iter() {
        let scope = rscope.split(":").collect::<Vec<&str>>();
        let na = match (&method, &scope[..]) {
            (&Method::GET, ["identify"]) =>
                if &endpoint == "/v2/systems/:system_id" {
                    ApiKeyAccess::PublicRead
                } else {
                    ApiKeyAccess::None
                },

            (&Method::GET, ["publicread", part]) =>
                if *part == "all" || is_part_path(part.as_ref(), endpoint.as_ref()) {
                    ApiKeyAccess::PublicRead
                } else {
                    ApiKeyAccess::None
                },

            (&Method::GET, ["read", part]) =>
                if *part == "all" || is_part_path(part.as_ref(), endpoint.as_ref()) {
                    ApiKeyAccess::PrivateRead
                } else {
                    ApiKeyAccess::None
                },

            (_, ["write", part]) =>
                if *part == "all" || is_part_path(part.as_ref(), endpoint.as_ref()) {
                    ApiKeyAccess::Full
                } else {
                    ApiKeyAccess::None
                },

            _ => ApiKeyAccess::None,
        };

        if na > access {
            access = na;
        }
    }

    access
}

pub async fn authnz(State(ctx): State<ApiContext>, mut request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let endpoint = request
        .extensions()
        .get::<MatchedPath>()
        .cloned()
        .map(|v| v.as_str().to_string())
        .unwrap_or("unknown".to_string());

    let mut authenticated: Option<String> = None;
    let headers = request.headers_mut();
    headers.remove("x-pluralkit-systemid");
    headers.remove("x-pluralkit-tid");
    headers.remove("x-pluralkit-privacylevel");

    let auth_header = headers
        .get("authorization")
        .map(|h| h.to_str().ok())
        .flatten();

    if let Some(auth_header) = auth_header {
        if auth_header.starts_with("Bearer ")
            && let Some(tid) =
                PKApiKey::parse_header_str(auth_header[7..].to_string(), &ctx.token_publickey)
            && let Some(token) =
                sqlx::query_as::<Postgres, PKApiKey>("select * from api_keys where id = $1")
                    .bind(&tid)
                    .fetch_optional(&ctx.db)
                    .await
                    .expect("failed to query apitoken in postgres")
        {
            authenticated = Some(format!("{tid}"));
            headers.append(
                "x-pluralkit-tid",
                HeaderValue::from_str(format!("{tid}").as_str()).unwrap(),
            );

            let access = apikey_can_access(&token, method.clone(), endpoint.clone());
            if access != ApiKeyAccess::None {
                headers.append(
                    "x-pluralkit-systemid",
                    HeaderValue::from_str(format!("{}", token.system).as_str()).unwrap(),
                );
                headers.append(
                    "x-pluralkit-privacylevel",
                    HeaderValue::from_str(access.privacy_level().as_ref()).unwrap(),
                );
            }
        } else if let Some(system_id) =
            match libpk::db::repository::legacy_token_auth(&ctx.db, auth_header).await {
                Ok(val) => val,
                Err(err) => {
                    error!(?err, "failed to query authorization token in postgres");
                    None
                }
            }
        {
            authenticated = Some("legacytoken".into());
            headers.append(
                "x-pluralkit-systemid",
                HeaderValue::from_str(format!("{system_id}").as_str()).unwrap(),
            );
            headers.append(
                "x-pluralkit-privacylevel",
                HeaderValue::from_static("private"),
            );
        }
    }

    let mut response = next.run(request).await;
    if authenticated.is_some() {
        let respheaders = response.headers_mut();
        respheaders.insert(DID_AUTHENTICATE_HEADER, HeaderValue::from_static("1"));
        respheaders.insert(
            "X-PluralKit-Authentication",
            HeaderValue::from_str(format!("{}", authenticated.unwrap()).as_str()).unwrap(),
        );
    }

    response
}
