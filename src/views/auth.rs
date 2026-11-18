//! Respuestas JSON de `/auth`. Nunca se serializa la entidad: lleva el hash
//! de la contraseña.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::models::users;

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub created_at: DateTime<Utc>,
}

impl From<&users::Model> for UserResponse {
    fn from(user: &users::Model) -> Self {
        Self {
            id: user.id,
            email: user.email.clone(),
            created_at: user.created_at.with_timezone(&Utc),
        }
    }
}

/// Respuesta de token según OAuth 2.0 (RFC 6749 §5.1).
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    /// Segundos hasta que caduca.
    pub expires_in: u64,
}

impl TokenResponse {
    #[must_use]
    pub fn bearer(access_token: String, expires_in: u64) -> Self {
        Self {
            access_token,
            token_type: "Bearer",
            expires_in,
        }
    }
}
