use axum::http::{HeaderValue, StatusCode, header};
use inst_user_monitor_back::{app::App, models::users};
use loco_rs::testing::prelude::*;
use sea_orm::ModelTrait;
use serde_json::{Value, json};
use serial_test::serial;

use super::prepare_data::{EMAIL, PASSWORD, bearer, login, register};

/// Error con el formato de Loco: `{"error": código, "description": texto}`.
fn assert_error(response: &axum_test::TestResponse, status: StatusCode, code: &str) {
    response.assert_status(status);
    let body: Value = response.json();
    assert_eq!(body["error"], code, "cuerpo: {body}");
    assert!(body["description"].is_string(), "cuerpo: {body}");
}

#[tokio::test]
#[serial]
async fn register_creates_user_with_normalized_email() {
    request::<App, _, _>(|request, ctx| async move {
        let body = register(&request, " John@GMAIL.com", PASSWORD).await;

        assert_eq!(body["email"], EMAIL);
        let object = body.as_object().expect("objeto JSON");
        let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["created_at", "email", "id"], "solo datos públicos");
        assert!(body["created_at"].as_str().unwrap().ends_with('Z'));

        let email = users::Email::parse(EMAIL).unwrap();
        let user = users::Model::find_by_email(&ctx.db, &email).await.unwrap();
        assert_eq!(body["id"], user.id.to_string());
        assert_eq!(user.id.get_version_num(), 7, "id UUID v7");
        assert!(user.password_hash.starts_with("$argon2id$"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn register_rejects_invalid_input() {
    request::<App, _, _>(|request, _ctx| async move {
        let cases = [
            ("not-an-email", PASSWORD, "invalid_email"),
            (EMAIL, "password", "weak_password"),
        ];
        for (email, password, code) in cases {
            let response = request
                .post("/auth/register")
                .json(&json!({ "email": email, "password": password }))
                .await;
            assert_error(&response, StatusCode::UNPROCESSABLE_ENTITY, code);
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn register_rejects_unknown_fields() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request
            .post("/auth/register")
            .json(&json!({ "email": EMAIL, "password": PASSWORD, "role": "admin" }))
            .await;
        response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn register_rejects_duplicate_email_ignoring_case() {
    request::<App, _, _>(|request, _ctx| async move {
        register(&request, EMAIL, PASSWORD).await;
        let response = request
            .post("/auth/register")
            .json(&json!({ "email": "JOHN@gmail.com", "password": PASSWORD }))
            .await;
        assert_error(&response, StatusCode::CONFLICT, "email_taken");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn login_issues_a_bearer_token_that_is_not_cached() {
    request::<App, _, _>(|request, _ctx| async move {
        register(&request, EMAIL, PASSWORD).await;

        let response = request
            .post("/auth/login")
            .json(&json!({ "email": "JOHN@gmail.com", "password": PASSWORD }))
            .await;

        response.assert_status_ok();
        assert_eq!(response.header(header::CACHE_CONTROL), "no-store");
        let body: Value = response.json();
        assert_eq!(body["token_type"], "Bearer");
        assert_eq!(body["expires_in"], 900);
        assert!(body["access_token"].as_str().is_some_and(|t| !t.is_empty()));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn login_fails_the_same_way_for_any_wrong_credential() {
    request::<App, _, _>(|request, _ctx| async move {
        register(&request, EMAIL, PASSWORD).await;

        // 4 intentos + el registro: dentro del límite de 5 del rate limit.
        let too_long = "A".repeat(users::Password::MAX_LEN + 1);
        for (email, password) in [
            (EMAIL, "Wrong-Pass123"),
            ("nobody@gmail.com", PASSWORD),
            ("not-an-email", PASSWORD),
            (EMAIL, too_long.as_str()),
        ] {
            let response = request
                .post("/auth/login")
                .json(&json!({ "email": email, "password": password }))
                .await;
            assert_error(&response, StatusCode::UNAUTHORIZED, "invalid_credentials");
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn me_returns_the_token_owner() {
    request::<App, _, _>(|request, _ctx| async move {
        let registered = register(&request, EMAIL, PASSWORD).await;
        let token = login(&request, EMAIL, PASSWORD).await;

        let (name, value) = bearer(&token);
        let response = request.get("/auth/me").add_header(name, value).await;

        response.assert_status_ok();
        assert_eq!(response.json::<Value>(), registered);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn me_requires_a_valid_bearer_token() {
    request::<App, _, _>(|request, _ctx| async move {
        for authorization in [None, Some("Bearer garbage"), Some("Basic dXNlcjpwYXNz")] {
            let mut call = request.get("/auth/me");
            if let Some(value) = authorization {
                call = call.add_header(header::AUTHORIZATION, HeaderValue::from_static(value));
            }
            assert_error(&call.await, StatusCode::UNAUTHORIZED, "unauthorized");
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn me_rejects_the_token_of_a_deleted_user() {
    request::<App, _, _>(|request, ctx| async move {
        register(&request, EMAIL, PASSWORD).await;
        let token = login(&request, EMAIL, PASSWORD).await;

        let email = users::Email::parse(EMAIL).unwrap();
        let user = users::Model::find_by_email(&ctx.db, &email).await.unwrap();
        user.delete(&ctx.db).await.unwrap();

        let (name, value) = bearer(&token);
        let response = request.get("/auth/me").add_header(name, value).await;
        assert_error(&response, StatusCode::UNAUTHORIZED, "unauthorized");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn register_and_login_are_rate_limited_per_ip() {
    request::<App, _, _>(|request, _ctx| async move {
        let body = json!({ "email": "nope", "password": "whatever" });
        for _ in 0..5 {
            let response = request.post("/auth/login").json(&body).await;
            response.assert_status(StatusCode::UNAUTHORIZED);
        }

        // El límite es compartido: también alcanza a register.
        let response = request.post("/auth/register").json(&body).await;
        response.assert_status(StatusCode::TOO_MANY_REQUESTS);

        // `me` no tiene límite.
        let response = request.get("/auth/me").await;
        response.assert_status(StatusCode::UNAUTHORIZED);
    })
    .await;
}

/// Detrás de un proxy todas las conexiones llegan desde su IP: el límite va
/// por la IP del cliente que indica el proxy (X-Real-IP en config/test.yaml).
#[tokio::test]
#[serial]
async fn rate_limit_uses_the_client_ip_set_by_the_proxy() {
    request::<App, _, _>(|request, _ctx| async move {
        let body = json!({ "email": "nope", "password": "whatever" });
        let from = |ip: &'static str| {
            (
                header::HeaderName::from_static("x-real-ip"),
                HeaderValue::from_static(ip),
            )
        };

        for _ in 0..5 {
            let (name, value) = from("203.0.113.1");
            let response = request
                .post("/auth/login")
                .add_header(name, value)
                .json(&body)
                .await;
            response.assert_status(StatusCode::UNAUTHORIZED);
        }
        let (name, value) = from("203.0.113.1");
        let response = request
            .post("/auth/login")
            .add_header(name, value)
            .json(&body)
            .await;
        response.assert_status(StatusCode::TOO_MANY_REQUESTS);

        // Otro cliente, misma conexión (la del proxy): tiene su propio límite.
        let (name, value) = from("203.0.113.2");
        let response = request
            .post("/auth/login")
            .add_header(name, value)
            .json(&body)
            .await;
        response.assert_status(StatusCode::UNAUTHORIZED);
    })
    .await;
}
