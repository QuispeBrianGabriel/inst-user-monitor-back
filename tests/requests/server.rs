//! Comportamiento común a toda la API: monitorización y middlewares
//! (`server.middlewares` en config/*.yaml).

use axum::http::{HeaderValue, Method, StatusCode, header};
use inst_user_monitor_back::app::App;
use loco_rs::testing::prelude::*;
use serde_json::{Value, json};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn readiness_checks_the_database() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/_readiness").await;
        response.assert_status_ok();
        assert_eq!(response.json::<Value>()["ok"], true);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn unknown_route_is_not_found() {
    request::<App, _, _>(|request, _ctx| async move {
        request
            .get("/no-existe")
            .await
            .assert_status(StatusCode::NOT_FOUND);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn rejects_bodies_over_16_kib() {
    request::<App, _, _>(|request, _ctx| async move {
        let body = json!({ "email": "a".repeat(17 * 1024), "password": "x" });
        request
            .post("/auth/register")
            .json(&body)
            .await
            .assert_status(StatusCode::PAYLOAD_TOO_LARGE);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn cors_allows_the_configured_frontend_only() {
    request::<App, _, _>(|request, _ctx| async move {
        let preflight = |origin: &'static str| {
            request
                .method(Method::OPTIONS, "/auth/login")
                .add_header(header::ORIGIN, HeaderValue::from_static(origin))
                .add_header(
                    header::ACCESS_CONTROL_REQUEST_METHOD,
                    HeaderValue::from_static("POST"),
                )
                .add_header(
                    header::ACCESS_CONTROL_REQUEST_HEADERS,
                    HeaderValue::from_static("content-type,authorization"),
                )
        };

        let allowed = preflight("http://localhost:3000").await;
        assert_eq!(
            allowed.header(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            "http://localhost:3000"
        );

        let other = preflight("https://evil.example").await;
        assert!(
            other
                .maybe_header(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
    })
    .await;
}
