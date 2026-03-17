use std::sync::Arc;

use argon2::{PasswordHash, PasswordVerifier};
use axum::{
    extract::{Request, State},
    http::{
        HeaderMap, StatusCode,
        header::{AUTHORIZATION, WWW_AUTHENTICATE},
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::prelude::*;

use crate::config::WebexConfig;

pub async fn auth_layer(
    State(config): State<Arc<WebexConfig>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let auth_cfg = match config.serve.auth.as_ref() {
        Some(a) => a,
        None => {
            return next.run(request).await;
        }
    };
    if let Some(auth_value) = headers.get(AUTHORIZATION) {
        let stored_pass_hash = match PasswordHash::new(&auth_cfg.pass_hash) {
            Ok(sph) => sph,
            Err(_) => {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };
        let body = match auth_value.to_str() {
            Ok(o) => o,
            _ => {
                return StatusCode::BAD_REQUEST.into_response();
            }
        };
        if !body.starts_with("Basic ") || body.len() < 6 {
            return StatusCode::BAD_REQUEST.into_response();
        }
        let auth_str = match BASE64_STANDARD
            .decode(&body[6..])
            .map(|x| String::from_utf8(x))
        {
            Ok(Ok(v)) => v,
            _ => {
                return StatusCode::BAD_REQUEST.into_response();
            }
        };
        let mut iter = auth_str.split(":");
        let (user, pass) = match (iter.next(), iter.next()) {
            (Some(u), Some(p)) => (u, p),
            _ => {
                return StatusCode::BAD_REQUEST.into_response();
            }
        };
        if &auth_cfg.user != user {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        if argon2::Argon2::default()
            .verify_password(pass.as_bytes(), &stored_pass_hash)
            .is_err()
        {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [(WWW_AUTHENTICATE, "Basic realm=\"ssh webex\"")],
        )
            .into_response()
    }
}
