//! Handlers HTTP de `/auth`. Solo traducen HTTP ⇄ casos de uso.

use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
};

use super::{
    dto::{LoginRequest, RegisterRequest, TokenResponse, UserResponse},
    extractor::AuthUser,
};
use crate::auth::{AuthState, application::AuthError};

/// `POST /auth/register` → 201 con el usuario creado.
pub async fn register(
    State(auth): State<AuthState>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<UserResponse>), AuthError> {
    let user = auth.register(&body.email, &body.password).await?;
    Ok((StatusCode::CREATED, Json(user.into())))
}

/// `POST /auth/login` → 200 con el token de acceso.
pub async fn login(
    State(auth): State<AuthState>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, AuthError> {
    let token = auth.login(&body.email, &body.password).await?;
    // RFC 6749 §5.1: las respuestas con tokens no se cachean.
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(TokenResponse::from(token)),
    ))
}

/// `GET /auth/me` → 200 con el usuario del token.
pub async fn me(
    State(auth): State<AuthState>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<UserResponse>, AuthError> {
    let user = auth.current_user(user_id).await?;
    Ok(Json(user.into()))
}
