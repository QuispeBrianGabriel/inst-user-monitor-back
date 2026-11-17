//! Extractor `AuthUser`: protege un endpoint exigiendo
//! `Authorization: Bearer <token>`. Si falta o no es válido, responde 401.
//!
//! Uso: `async fn handler(AuthUser(user_id): AuthUser) { … }`

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use uuid::Uuid;

use crate::auth::{AuthState, application::AuthError};

#[derive(Debug, Clone, Copy)]
pub struct AuthUser(pub Uuid);

impl FromRequestParts<AuthState> for AuthUser {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        auth: &AuthState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(bearer_token)
            .ok_or(AuthError::InvalidToken)?;
        auth.authenticate(token).map(Self)
    }
}

/// Token de un `Authorization: Bearer <token>` (el esquema no distingue mayúsculas).
fn bearer_token(header: &str) -> Option<&str> {
    let (scheme, token) = header.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bearer_header() {
        assert_eq!(bearer_token("Bearer abc.def"), Some("abc.def"));
        assert_eq!(bearer_token("bearer abc"), Some("abc"));
        assert_eq!(bearer_token("Basic abc"), None);
        assert_eq!(bearer_token("Bearer "), None);
        assert_eq!(bearer_token("Bearer"), None);
    }
}
