//! Datos y peticiones comunes a los tests de `/auth`.

use axum::http::{HeaderName, HeaderValue, header};
use loco_rs::TestServer;
use serde_json::{Value, json};

pub const EMAIL: &str = "john@gmail.com";
pub const PASSWORD: &str = "Sup3r-Secret!";

/// Registra un usuario y devuelve la respuesta (201).
pub async fn register(request: &TestServer, email: &str, password: &str) -> Value {
    let response = request
        .post("/auth/register")
        .json(&json!({ "email": email, "password": password }))
        .await;
    response.assert_status(axum::http::StatusCode::CREATED);
    response.json()
}

/// Inicia sesión y devuelve el token de acceso.
pub async fn login(request: &TestServer, email: &str, password: &str) -> String {
    let response = request
        .post("/auth/login")
        .json(&json!({ "email": email, "password": password }))
        .await;
    response.assert_status_ok();
    response.json::<Value>()["access_token"]
        .as_str()
        .expect("access_token es un string")
        .to_owned()
}

pub fn bearer(token: &str) -> (HeaderName, HeaderValue) {
    let value = HeaderValue::from_str(&format!("Bearer {token}")).expect("cabecera válida");
    (header::AUTHORIZATION, value)
}
