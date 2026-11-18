use inst_user_monitor_back::{
    app::App,
    models::users::{self, AuthError, LoginParams, RegisterParams},
};
use loco_rs::{
    model::{Authenticable, ModelError},
    testing::prelude::*,
};
use sea_orm::{ActiveModelTrait, ActiveValue};
use serial_test::serial;

const PASSWORD: &str = "Sup3r-Secret!";

fn register_params(email: &str) -> RegisterParams {
    RegisterParams {
        email: email.to_owned(),
        password: PASSWORD.to_owned(),
    }
}

fn login_params(email: &str, password: &str) -> LoginParams {
    LoginParams {
        email: email.to_owned(),
        password: password.to_owned(),
    }
}

#[tokio::test]
#[serial]
async fn create_with_password_stores_normalized_email_and_hash() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    let user = users::Model::create_with_password(db, &register_params(" John@Gmail.com"))
        .await
        .unwrap();

    assert_eq!(user.email, "john@gmail.com");
    assert_ne!(user.password_hash, PASSWORD);
    assert_eq!(user.created_at, user.updated_at);
}

#[tokio::test]
#[serial]
async fn find_by_credentials_checks_email_and_password() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let user = users::Model::create_with_password(db, &register_params("john@gmail.com"))
        .await
        .unwrap();

    let found = users::Model::find_by_credentials(db, &login_params("JOHN@gmail.com", PASSWORD))
        .await
        .unwrap();
    assert_eq!(found.id, user.id);

    for (email, password) in [
        ("john@gmail.com", "Wrong-Pass123"),
        ("nobody@gmail.com", PASSWORD),
    ] {
        assert!(matches!(
            users::Model::find_by_credentials(db, &login_params(email, password)).await,
            Err(AuthError::InvalidCredentials)
        ));
    }
}

#[tokio::test]
#[serial]
async fn find_by_claims_key_resolves_the_jwt_subject() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let user = users::Model::create_with_password(db, &register_params("john@gmail.com"))
        .await
        .unwrap();

    let found = <users::Model as Authenticable>::find_by_claims_key(db, &user.id.to_string())
        .await
        .unwrap();
    assert_eq!(found.id, user.id);

    assert!(matches!(
        <users::Model as Authenticable>::find_by_claims_key(db, "not-a-uuid").await,
        Err(ModelError::EntityNotFound)
    ));
}

/// La invariante del modelo (`Validatable`) cubre cualquier escritura, no solo
/// el registro, antes incluso de llegar al CHECK de la base.
#[tokio::test]
#[serial]
async fn saving_a_non_normalized_email_is_rejected() {
    let boot = boot_test::<App>().await.unwrap();

    let result = users::ActiveModel {
        email: ActiveValue::Set("John@Gmail.com".to_owned()),
        password_hash: ActiveValue::Set("$argon2id$irrelevante".to_owned()),
        ..Default::default()
    }
    .insert(&boot.app_context.db)
    .await;

    let error = result
        .expect_err("el email sin normalizar no se guarda")
        .to_string();
    assert!(error.contains("email_not_normalized"), "{error}");
}
