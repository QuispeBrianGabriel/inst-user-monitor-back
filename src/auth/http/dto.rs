//! Objetos de entrada y salida de la API (JSON).
//!
//! Las peticiones no derivan `Debug` a propósito: llevan la contraseña en
//! claro y no deben acabar en un log.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::domain::{User, ports::AccessToken};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id(),
            email: user.email().to_string(),
            created_at: user.created_at(),
        }
    }
}

/// Respuesta de token según OAuth 2.0 (RFC 6749 §5.1).
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
}

impl From<AccessToken> for TokenResponse {
    fn from(token: AccessToken) -> Self {
        Self {
            access_token: token.token,
            token_type: "Bearer",
            expires_in: token.expires_in,
        }
    }
}
