//! Adaptador HTTP del módulo auth: rutas, DTOs y extractor del token.

mod controller;
mod dto;
mod error;
pub mod extractor;

use std::time::Duration;

use axum::{
    Router,
    routing::{get, post},
};
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};

use super::AuthState;

/// Peticiones seguidas permitidas por IP en register/login.
const RATE_LIMIT_BURST: u32 = 5;
/// Cada cuántos segundos se recupera una petición (≈ 10 por minuto).
const RATE_LIMIT_REFILL_SECS: u64 = 6;

/// Rutas de `/auth`:
/// - `POST /auth/register`
/// - `POST /auth/login`
/// - `GET  /auth/me` (requiere token)
///
/// Register y login tienen límite de peticiones por IP contra fuerza bruta.
/// Se identifica la IP por la conexión: el servidor debe arrancar con
/// `into_make_service_with_connect_info::<SocketAddr>()`.
pub fn router(state: AuthState) -> Router {
    let rate_limit = GovernorConfigBuilder::default()
        .per_second(RATE_LIMIT_REFILL_SECS)
        .burst_size(RATE_LIMIT_BURST)
        .finish()
        .expect("configuración de rate limit válida");

    // Libera de memoria las IPs que ya no están limitadas.
    let limiter = rate_limit.limiter().clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            limiter.retain_recent();
        }
    });

    let limited = Router::new()
        .route("/auth/register", post(controller::register))
        .route("/auth/login", post(controller::login))
        .route_layer(GovernorLayer::new(rate_limit));

    Router::new()
        .merge(limited)
        .route("/auth/me", get(controller::me))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, time::Duration};

    use axum::{
        body::{Body, to_bytes},
        extract::ConnectInfo,
        http::{Request, StatusCode, header},
    };
    use serde_json::{Value, json};
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;
    use crate::auth::{self, AuthConfig};

    /// Router de auth sin base de datos (los casos probados no llegan a ella).
    fn app() -> Router {
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(500))
            .connect_lazy("postgres://user:pass@127.0.0.1:1/none")
            .unwrap();
        let config = AuthConfig::new(
            "test-secret-with-at-least-32-bytes!!",
            Duration::from_secs(900),
        )
        .unwrap();
        router(auth::state(pool, &config).unwrap())
    }

    /// IP del cliente, como la añade `into_make_service_with_connect_info`.
    fn from_client(mut request: Request<Body>) -> Request<Body> {
        let addr = SocketAddr::from(([127, 0, 0, 1], 4000));
        request.extensions_mut().insert(ConnectInfo(addr));
        request
    }

    fn post_json(uri: &str, body: Value) -> Request<Body> {
        from_client(
            Request::post(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
    }

    async fn send(app: Router, request: Request<Body>) -> (StatusCode, Value) {
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    #[tokio::test]
    async fn register_rejects_invalid_email() {
        let body = json!({ "email": "not-an-email", "password": "Sup3r-Secret!" });
        let (status, json) = send(app(), post_json("/auth/register", body)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json["error"], "invalid_email");
    }

    #[tokio::test]
    async fn register_rejects_weak_password() {
        let body = json!({ "email": "john@gmail.com", "password": "password" });
        let (status, json) = send(app(), post_json("/auth/register", body)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json["error"], "weak_password");
    }

    #[tokio::test]
    async fn register_rejects_unknown_fields() {
        let body =
            json!({ "email": "john@gmail.com", "password": "Sup3r-Secret!", "role": "admin" });
        let (status, _) = send(app(), post_json("/auth/register", body)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn login_with_malformed_email_is_invalid_credentials() {
        let body = json!({ "email": "nope", "password": "whatever" });
        let (status, json) = send(app(), post_json("/auth/login", body)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"], "invalid_credentials");
    }

    #[tokio::test]
    async fn me_requires_a_valid_bearer_token() {
        for authorization in [None, Some("Bearer garbage"), Some("Basic dXNlcjpwYXNz")] {
            let mut request = Request::get("/auth/me");
            if let Some(value) = authorization {
                request = request.header(header::AUTHORIZATION, value);
            }
            let request = from_client(request.body(Body::empty()).unwrap());
            let response = app().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert!(response.headers().contains_key(header::WWW_AUTHENTICATE));
        }
    }

    #[tokio::test]
    async fn login_is_rate_limited_per_ip() {
        let app = app();
        let body = json!({ "email": "nope", "password": "whatever" });
        for _ in 0..RATE_LIMIT_BURST {
            let (status, _) = send(app.clone(), post_json("/auth/login", body.clone())).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        }
        let (status, _) = send(app, post_json("/auth/login", body)).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    }
}
