//! `/accounts`: cuentas de Instagram monitoreadas y sus capturas.
//!
//! Todas las rutas piden un token válido y solo tocan las cuentas de su
//! usuario: las de otros responden 404, como si no existieran. Los errores
//! siguen el formato de Loco (ver `controllers::auth`).

use axum::{extract::DefaultBodyLimit, http::StatusCode};
use loco_rs::{controller::ErrorDetail, prelude::*};

use crate::{
    models::{
        account_snapshots::{self, SnapshotError, SnapshotParams},
        monitored_accounts::{self, AccountError, AccountParams},
        users,
    },
    views::accounts::{AccountDetail, AccountResponse, AccountSummary, SnapshotResponse},
};

/// Tamaño máximo del cuerpo al pegar una lista. El resto de la API se queda
/// en el límite general (`server.middlewares.limit_payload`): una lista de
/// miles de cuentas ocupa cientos de KB.
const SNAPSHOT_BODY_LIMIT: usize = 2 * 1024 * 1024;

impl From<AccountError> for Error {
    fn from(error: AccountError) -> Self {
        let (status, code) = match error {
            AccountError::Model(error) => return error.into(),
            AccountError::InvalidUsername(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "invalid_username")
            }
            AccountError::FullNameTooLong => {
                (StatusCode::UNPROCESSABLE_ENTITY, "invalid_full_name")
            }
            AccountError::NotesTooLong => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_notes"),
            AccountError::UsernameTaken => (StatusCode::CONFLICT, "username_taken"),
        };
        Self::CustomError(status, ErrorDetail::new(code, error.to_string()))
    }
}

impl From<SnapshotError> for Error {
    fn from(error: SnapshotError) -> Self {
        match error {
            SnapshotError::Model(error) => error.into(),
            SnapshotError::InvalidFormat(error) => Self::CustomError(
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorDetail::new("invalid_format", error.to_string()),
            ),
        }
    }
}

type Auth = auth::JWTWithUser<users::Model>;

/// `GET /accounts` → 200 con las cuentas y el estado de sus listas.
#[debug_handler]
async fn list(auth: Auth, State(ctx): State<AppContext>) -> Result<Response> {
    let accounts = monitored_accounts::Model::list_for_user(&ctx.db, auth.user.id).await?;
    let ids: Vec<_> = accounts.iter().map(|account| account.id).collect();
    let timelines = account_snapshots::Model::timelines(&ctx.db, &ids).await?;
    format::json(
        accounts
            .iter()
            .map(|account| {
                AccountSummary::new(
                    account,
                    timelines.get(&account.id).map_or(&[], Vec::as_slice),
                )
            })
            .collect::<Vec<_>>(),
    )
}

/// `POST /accounts` → 201 con la cuenta creada.
#[debug_handler]
async fn create(
    auth: Auth,
    State(ctx): State<AppContext>,
    Json(params): Json<AccountParams>,
) -> Result<Response> {
    let account = monitored_accounts::Model::create(&ctx.db, auth.user.id, &params).await?;
    format::render()
        .status(StatusCode::CREATED)
        .json(AccountResponse::from(&account))
}

/// `GET /accounts/{id}` → 200 con la cuenta, sus listas y sus cambios.
#[debug_handler]
async fn show(auth: Auth, Path(id): Path<Uuid>, State(ctx): State<AppContext>) -> Result<Response> {
    let account = monitored_accounts::Model::find_for_user(&ctx.db, auth.user.id, id).await?;
    let mut timelines = account_snapshots::Model::timelines(&ctx.db, &[account.id]).await?;
    let timelines = timelines.remove(&account.id).unwrap_or_default();
    format::json(AccountDetail::new(&account, &timelines))
}

/// `PUT /accounts/{id}` → 200 con la cuenta actualizada.
#[debug_handler]
async fn update(
    auth: Auth,
    Path(id): Path<Uuid>,
    State(ctx): State<AppContext>,
    Json(params): Json<AccountParams>,
) -> Result<Response> {
    let account = monitored_accounts::Model::find_for_user(&ctx.db, auth.user.id, id).await?;
    let account = account.update_details(&ctx.db, &params).await?;
    format::json(AccountResponse::from(&account))
}

/// `DELETE /accounts/{id}` → 204. Borra también sus capturas.
#[debug_handler]
async fn remove(
    auth: Auth,
    Path(id): Path<Uuid>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let account = monitored_accounts::Model::find_for_user(&ctx.db, auth.user.id, id).await?;
    account.remove(&ctx.db).await?;
    format::render().status(StatusCode::NO_CONTENT).empty()
}

/// `POST /accounts/{id}/snapshots` → 201 con la captura guardada, o 422
/// `invalid_format` (y nada guardado) si el texto no es una lista válida.
#[debug_handler]
async fn create_snapshot(
    auth: Auth,
    Path(id): Path<Uuid>,
    State(ctx): State<AppContext>,
    Json(params): Json<SnapshotParams>,
) -> Result<Response> {
    let account = monitored_accounts::Model::find_for_user(&ctx.db, auth.user.id, id).await?;
    let snapshot = account_snapshots::Model::create(&ctx.db, account.id, &params).await?;
    format::render()
        .status(StatusCode::CREATED)
        .json(SnapshotResponse::from(&snapshot))
}

/// `DELETE /accounts/{id}/snapshots/{snapshot_id}` → 204.
#[debug_handler]
async fn remove_snapshot(
    auth: Auth,
    Path((id, snapshot_id)): Path<(Uuid, Uuid)>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let account = monitored_accounts::Model::find_for_user(&ctx.db, auth.user.id, id).await?;
    let snapshot =
        account_snapshots::Model::find_for_account(&ctx.db, account.id, snapshot_id).await?;
    snapshot.remove(&ctx.db).await?;
    format::render().status(StatusCode::NO_CONTENT).empty()
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/accounts")
        .add("/", get(list).post(create))
        .add("/{id}", get(show).put(update).delete(remove))
        .add(
            "/{id}/snapshots",
            // La capa interior manda sobre el límite general.
            post(create_snapshot).layer(DefaultBodyLimit::max(SNAPSHOT_BODY_LIMIT)),
        )
        .add("/{id}/snapshots/{snapshot_id}", delete(remove_snapshot))
}
