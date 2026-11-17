//! Rutas HTTP de la API.

use std::time::Duration;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::get,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};

use crate::auth::{self, AuthState};

/// Tamaño máximo del cuerpo de una petición. La API solo recibe JSON pequeño.
const BODY_LIMIT: usize = 16 * 1024;
/// Tiempo máximo por petición antes de responder 408.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Construye el router con todas las rutas y las capas comunes.
pub fn router(pool: PgPool, auth: AuthState) -> Router {
    Router::new()
        .route("/health", get(health))
        .with_state(pool)
        .merge(auth::router(auth))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(TraceLayer::new_for_http())
}

/// 200 si la API y la base responden, 503 si la base no responde.
async fn health(State(pool): State<PgPool>) -> (StatusCode, Json<Value>) {
    match sqlx::query("SELECT 1").execute(&pool).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "up" }))),
        Err(error) => {
            tracing::warn!(%error, "la base de datos no responde");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "down" })),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;

    /// Router con una base que no existe: los tests no necesitan PostgreSQL.
    fn app_without_db() -> Router {
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(500))
            .connect_lazy("postgres://user:pass@127.0.0.1:1/none")
            .unwrap();
        let config = auth::AuthConfig::new(
            "test-secret-with-at-least-32-bytes!!",
            Duration::from_secs(900),
        )
        .unwrap();
        let auth = auth::state(pool.clone(), &config).unwrap();
        router(pool, auth)
    }

    async fn get_status(uri: &str) -> StatusCode {
        let request = Request::get(uri).body(Body::empty()).unwrap();
        app_without_db().oneshot(request).await.unwrap().status()
    }

    #[tokio::test]
    async fn health_is_unavailable_without_db() {
        assert_eq!(get_status("/health").await, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn unknown_route_is_not_found() {
        assert_eq!(get_status("/no-existe").await, StatusCode::NOT_FOUND);
    }
}
