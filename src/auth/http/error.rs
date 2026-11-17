//! Traducción de `AuthError` a respuestas HTTP.
//!
//! Formato: `{"error": "<código>", "message": "<texto>"}`. El código es
//! estable (para el frontend); el mensaje es legible y puede cambiar.

use axum::{
    Json,
    http::{HeaderValue, StatusCode, header::WWW_AUTHENTICATE},
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::auth::application::AuthError;

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::InvalidEmail(_) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_email"),
            Self::WeakPassword(_) => (StatusCode::UNPROCESSABLE_ENTITY, "weak_password"),
            Self::EmailTaken => (StatusCode::CONFLICT, "email_taken"),
            Self::InvalidCredentials => (StatusCode::UNAUTHORIZED, "invalid_credentials"),
            Self::InvalidToken => (StatusCode::UNAUTHORIZED, "invalid_token"),
            Self::Unexpected(error) => {
                // El detalle va al log, nunca al cliente.
                tracing::error!(%error, "error inesperado en auth");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
            }
        };
        let message = match &self {
            Self::Unexpected(_) => "error interno".to_owned(),
            other => other.to_string(),
        };

        let mut response =
            (status, Json(json!({ "error": code, "message": message }))).into_response();
        if matches!(self, Self::InvalidToken) {
            // RFC 6750 §3: indica al cliente qué esquema de autenticación usar.
            response.headers_mut().insert(
                WWW_AUTHENTICATE,
                HeaderValue::from_static(r#"Bearer error="invalid_token""#),
            );
        }
        response
    }
}
