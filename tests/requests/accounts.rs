use axum::http::{HeaderName, HeaderValue, Method, StatusCode, header};
use axum_test::TestResponse;
use inst_user_monitor_back::app::App;
use loco_rs::{TestServer, testing::prelude::*};
use serde_json::{Value, json};
use serial_test::serial;

use super::prepare_data::{EMAIL, PASSWORD, bearer, login, register};

const FOLLOWERS: &str = include_str!("../fixtures/followers.txt");

/// Registra un usuario e inicia sesión: devuelve su cabecera `Authorization`.
async fn sign_in(request: &TestServer, email: &str) -> (HeaderName, HeaderValue) {
    register(request, email, PASSWORD).await;
    bearer(&login(request, email, PASSWORD).await)
}

async fn create_account(
    request: &TestServer,
    auth: &(HeaderName, HeaderValue),
    body: Value,
) -> TestResponse {
    request
        .post("/accounts")
        .add_header(auth.0.clone(), auth.1.clone())
        .json(&body)
        .await
}

async fn paste(
    request: &TestServer,
    auth: &(HeaderName, HeaderValue),
    account_id: &str,
    text: &str,
) -> TestResponse {
    request
        .post(&format!("/accounts/{account_id}/snapshots"))
        .add_header(auth.0.clone(), auth.1.clone())
        .json(&json!({ "text": text }))
        .await
}

async fn get_json(request: &TestServer, auth: &(HeaderName, HeaderValue), path: &str) -> Value {
    let response = request
        .get(path)
        .add_header(auth.0.clone(), auth.1.clone())
        .await;
    response.assert_status_ok();
    response.json()
}

fn assert_error(response: &TestResponse, status: StatusCode, code: &str) {
    response.assert_status(status);
    let body: Value = response.json();
    assert_eq!(body["error"], code, "cuerpo: {body}");
    assert!(body["description"].is_string(), "cuerpo: {body}");
}

#[tokio::test]
#[serial]
async fn accounts_require_a_token() {
    request::<App, _, _>(|request, _ctx| async move {
        request
            .get("/accounts")
            .await
            .assert_status(StatusCode::UNAUTHORIZED);
        request
            .post("/accounts")
            .json(&json!({ "username": "duki" }))
            .await
            .assert_status(StatusCode::UNAUTHORIZED);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn create_and_list_accounts() {
    request::<App, _, _>(|request, _ctx| async move {
        let auth = sign_in(&request, EMAIL).await;

        let response = create_account(
            &request,
            &auth,
            json!({ "username": " @Duki ", "full_name": "Duki", "notes": "  " }),
        )
        .await;
        response.assert_status(StatusCode::CREATED);
        let account: Value = response.json();
        assert_eq!(account["username"], "duki");
        assert_eq!(account["full_name"], "Duki");
        assert_eq!(account["notes"], Value::Null);

        create_account(&request, &auth, json!({ "username": "angelmeeza" }))
            .await
            .assert_status(StatusCode::CREATED);

        let list = get_json(&request, &auth, "/accounts").await;
        let usernames: Vec<_> = list
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["account"]["username"].as_str().unwrap())
            .collect();
        assert_eq!(usernames, ["angelmeeza", "duki"]);
        assert_eq!(list[0]["followers"], Value::Null);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn create_rejects_invalid_or_repeated_accounts() {
    request::<App, _, _>(|request, _ctx| async move {
        let auth = sign_in(&request, EMAIL).await;
        create_account(&request, &auth, json!({ "username": "duki" }))
            .await
            .assert_status(StatusCode::CREATED);

        let cases = [
            (
                json!({ "username": "DUKI" }),
                StatusCode::CONFLICT,
                "username_taken",
            ),
            (
                json!({ "username": "du ki" }),
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_username",
            ),
            (
                json!({ "username": "" }),
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_username",
            ),
            (
                json!({ "username": "otra", "full_name": "x".repeat(151) }),
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_full_name",
            ),
        ];
        for (body, status, code) in cases {
            assert_error(&create_account(&request, &auth, body).await, status, code);
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn accounts_of_other_users_do_not_exist() {
    request::<App, _, _>(|request, _ctx| async move {
        let owner = sign_in(&request, EMAIL).await;
        let account: Value = create_account(&request, &owner, json!({ "username": "duki" }))
            .await
            .json();
        let id = account["id"].as_str().unwrap();

        let other = sign_in(&request, "other@gmail.com").await;
        let path = format!("/accounts/{id}");
        assert_eq!(get_json(&request, &other, "/accounts").await, json!([]));
        request
            .get(&path)
            .add_header(other.0.clone(), other.1.clone())
            .await
            .assert_status(StatusCode::NOT_FOUND);
        request
            .delete(&path)
            .add_header(other.0.clone(), other.1.clone())
            .await
            .assert_status(StatusCode::NOT_FOUND);
        paste(&request, &other, id, "duki\n5202")
            .await
            .assert_status(StatusCode::NOT_FOUND);

        // Otro usuario puede monitorear la misma cuenta por su cuenta.
        create_account(&request, &other, json!({ "username": "duki" }))
            .await
            .assert_status(StatusCode::CREATED);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn update_and_delete_an_account() {
    request::<App, _, _>(|request, _ctx| async move {
        let auth = sign_in(&request, EMAIL).await;
        let account: Value = create_account(&request, &auth, json!({ "username": "duki" }))
            .await
            .json();
        let path = format!("/accounts/{}", account["id"].as_str().unwrap());

        let response = request
            .put(&path)
            .add_header(auth.0.clone(), auth.1.clone())
            .json(&json!({ "username": "duki", "full_name": "Duki", "notes": "Trap" }))
            .await;
        response.assert_status_ok();
        assert_eq!(response.json::<Value>()["notes"], "Trap");

        request
            .delete(&path)
            .add_header(auth.0.clone(), auth.1.clone())
            .await
            .assert_status(StatusCode::NO_CONTENT);
        assert_eq!(get_json(&request, &auth, "/accounts").await, json!([]));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn pasting_lists_tracks_changes() {
    request::<App, _, _>(|request, _ctx| async move {
        let auth = sign_in(&request, EMAIL).await;
        let account: Value = create_account(&request, &auth, json!({ "username": "rocioteran__" }))
            .await
            .json();
        let id = account["id"].as_str().unwrap();
        let path = format!("/accounts/{id}");

        let response = paste(&request, &auth, id, FOLLOWERS).await;
        response.assert_status(StatusCode::CREATED);
        let snapshot: Value = response.json();
        assert_eq!(snapshot["total"], 222);

        let detail = get_json(&request, &auth, &path).await;
        assert_eq!(detail["followers"]["latest"]["total"], 222);
        assert_eq!(detail["followers"]["entries"][0]["username"], "izqmatty");
        assert_eq!(detail["followers"]["changes"], Value::Null);

        // Segunda captura: se va `izqmatty`, llega `nuevo.seguidor` y
        // `duki` cambia de nombre.
        let second = FOLLOWERS
            .replacen("izqmatty\nizqmatty\n", "nuevo.seguidor\nNuevo\n", 1)
            .replacen("duki\n5202\n", "duki\nDuki\n", 1);
        paste(&request, &auth, id, &second)
            .await
            .assert_status(StatusCode::CREATED);

        let detail = get_json(&request, &auth, &path).await;
        let followers = &detail["followers"];
        assert_eq!(followers["history"].as_array().unwrap().len(), 2);
        assert_eq!(followers["previous"]["total"], 222);
        let changes = &followers["changes"];
        assert_eq!(changes["total_delta"], 0);
        assert_eq!(
            changes["added"],
            json!([{ "username": "nuevo.seguidor", "full_name": "Nuevo" }])
        );
        assert_eq!(
            changes["removed"],
            json!([{ "username": "izqmatty", "full_name": "izqmatty" }])
        );
        assert_eq!(
            changes["renamed"],
            json!([{ "username": "duki", "before": "5202", "after": "Duki" }])
        );

        let list = get_json(&request, &auth, "/accounts").await;
        let summary = &list[0]["followers"];
        assert_eq!(summary["latest"]["total"], 222);
        assert_eq!(summary["total_delta"], 0);
        assert_eq!(summary["added"], 1);
        assert_eq!(summary["removed"], 1);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn invalid_pastes_are_rejected_and_not_saved() {
    request::<App, _, _>(|request, _ctx| async move {
        let auth = sign_in(&request, EMAIL).await;
        let account: Value = create_account(&request, &auth, json!({ "username": "duki" }))
            .await
            .json();
        let id = account["id"].as_str().unwrap();

        let response = paste(&request, &auth, id, "Seguidores\nduki\n5202").await;
        assert_error(
            &response,
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_format",
        );
        let description = response.json::<Value>()["description"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(description.contains("línea 1"), "{description}");

        assert_error(
            &paste(&request, &auth, id, "  \n").await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_format",
        );
        // Ya no hay listas distintas: un `kind` es un campo desconocido.
        request
            .post(&format!("/accounts/{id}/snapshots"))
            .add_header(auth.0.clone(), auth.1.clone())
            .json(&json!({ "kind": "following", "text": "duki\n5202" }))
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);

        let detail = get_json(&request, &auth, &format!("/accounts/{id}")).await;
        assert_eq!(detail["followers"], Value::Null, "no se guardó nada");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn pasted_lists_may_exceed_the_general_body_limit() {
    request::<App, _, _>(|request, _ctx| async move {
        let auth = sign_in(&request, EMAIL).await;
        let account: Value = create_account(&request, &auth, json!({ "username": "duki" }))
            .await
            .json();
        let id = account["id"].as_str().unwrap();

        // ≈ 300 KB, muy por encima de los 16 KB del resto de la API.
        let text: String = (0..10_000)
            .map(|i| format!("cuenta.{i}\nCuenta número {i}\n"))
            .collect();
        let response = paste(&request, &auth, id, &text).await;
        response.assert_status(StatusCode::CREATED);
        assert_eq!(response.json::<Value>()["total"], 10_000);

        // El resto de rutas mantiene el límite general.
        create_account(
            &request,
            &auth,
            json!({ "username": "x", "notes": "x".repeat(17 * 1024) }),
        )
        .await
        .assert_status(StatusCode::PAYLOAD_TOO_LARGE);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn delete_a_snapshot() {
    request::<App, _, _>(|request, _ctx| async move {
        let auth = sign_in(&request, EMAIL).await;
        let account: Value = create_account(&request, &auth, json!({ "username": "duki" }))
            .await
            .json();
        let id = account["id"].as_str().unwrap();
        paste(&request, &auth, id, "duki\n5202")
            .await
            .assert_status(StatusCode::CREATED);
        let snapshot: Value = paste(&request, &auth, id, "duki\n5202\nangelmeeza")
            .await
            .json();

        let path = format!(
            "/accounts/{id}/snapshots/{}",
            snapshot["id"].as_str().unwrap()
        );
        request
            .delete(&path)
            .add_header(auth.0.clone(), auth.1.clone())
            .await
            .assert_status(StatusCode::NO_CONTENT);
        request
            .delete(&path)
            .add_header(auth.0.clone(), auth.1.clone())
            .await
            .assert_status(StatusCode::NOT_FOUND);

        let detail = get_json(&request, &auth, &format!("/accounts/{id}")).await;
        assert_eq!(detail["followers"]["latest"]["total"], 1);
        assert_eq!(detail["followers"]["changes"], Value::Null);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn cors_allows_put_and_delete_from_the_frontend() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request
            .method(
                Method::OPTIONS,
                "/accounts/00000000-0000-0000-0000-000000000000",
            )
            .add_header(
                header::ORIGIN,
                HeaderValue::from_static("http://localhost:3000"),
            )
            .add_header(
                header::ACCESS_CONTROL_REQUEST_METHOD,
                HeaderValue::from_static("DELETE"),
            )
            .await;
        let methods = response.header(header::ACCESS_CONTROL_ALLOW_METHODS);
        let methods = methods.to_str().unwrap();
        assert!(
            methods.contains("PUT") && methods.contains("DELETE"),
            "{methods}"
        );
    })
    .await;
}
