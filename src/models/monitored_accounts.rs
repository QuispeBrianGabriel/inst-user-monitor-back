//! Cuentas de Instagram que un usuario monitorea a mano.
//!
//! La cuenta no se verifica contra Instagram: solo se valida la forma del
//! usuario (submódulo `username`). Cada usuario ve y toca únicamente sus
//! cuentas: todas las búsquedas filtran por dueño, y una cuenta de otro se
//! trata como inexistente (404, sin revelar que existe).

mod username;

use std::result::Result;

use loco_rs::prelude::*;
use sea_orm::{QueryOrder, SqlErr};
use serde::Deserialize;

pub use self::username::{Username, UsernameError};
pub use super::_entities::monitored_accounts::{self, ActiveModel, Column, Entity, Model};

/// Restricción UNIQUE `(user_id, username)` (ver la migración).
const UNIQUE_USERNAME_CONSTRAINT: &str = "monitored_accounts_user_username_key";

/// Máximo de caracteres del nombre visible (como `varchar(150)` en la base).
pub const MAX_FULL_NAME_LEN: usize = 150;
/// Máximo de caracteres de las notas.
pub const MAX_NOTES_LEN: usize = 2000;

/// Cuerpo de `POST /accounts` y de `PUT /accounts/{id}`: los datos que el
/// usuario indica de la cuenta. Los opcionales vacíos se guardan como `NULL`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountParams {
    pub username: String,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error(transparent)]
    InvalidUsername(#[from] UsernameError),
    #[error("el nombre no puede superar {MAX_FULL_NAME_LEN} caracteres")]
    FullNameTooLong,
    #[error("las notas no pueden superar {MAX_NOTES_LEN} caracteres")]
    NotesTooLong,
    #[error("ya estás monitoreando esa cuenta")]
    UsernameTaken,
    #[error(transparent)]
    Model(#[from] ModelError),
}

impl From<DbErr> for AccountError {
    fn from(error: DbErr) -> Self {
        Self::Model(error.into())
    }
}

/// Datos de [`AccountParams`] ya validados y normalizados.
struct ValidParams {
    username: Username,
    full_name: Option<String>,
    notes: Option<String>,
}

impl TryFrom<&AccountParams> for ValidParams {
    type Error = AccountError;

    fn try_from(params: &AccountParams) -> Result<Self, AccountError> {
        let full_name = optional_text(params.full_name.as_deref());
        if full_name
            .as_ref()
            .is_some_and(|name| name.chars().count() > MAX_FULL_NAME_LEN)
        {
            return Err(AccountError::FullNameTooLong);
        }
        let notes = optional_text(params.notes.as_deref());
        if notes
            .as_ref()
            .is_some_and(|notes| notes.chars().count() > MAX_NOTES_LEN)
        {
            return Err(AccountError::NotesTooLong);
        }
        Ok(Self {
            username: Username::parse(&params.username)?,
            full_name,
            notes,
        })
    }
}

/// Recorta un texto opcional; vacío cuenta como ausente.
fn optional_text(text: Option<&str>) -> Option<String> {
    text.map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// Traduce la violación de la restricción UNIQUE a `UsernameTaken`.
fn map_unique(error: DbErr) -> AccountError {
    match error.sql_err() {
        Some(SqlErr::UniqueConstraintViolation(detail))
            if detail.contains(UNIQUE_USERNAME_CONSTRAINT) =>
        {
            AccountError::UsernameTaken
        }
        _ => error.into(),
    }
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    // `updated_at` no se toca aquí: lo mantiene un trigger de la base.
    async fn before_save<C>(self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut this = self;
        if insert && !this.id.is_set() {
            this.id = ActiveValue::Set(Uuid::now_v7());
        }
        Ok(this)
    }
}

impl Model {
    /// Empieza a monitorear una cuenta para `user_id`.
    ///
    /// # Errors
    ///
    /// Si los datos no son válidos, `UsernameTaken` si el usuario ya monitorea
    /// esa cuenta, o un error inesperado de la base.
    pub async fn create(
        db: &DatabaseConnection,
        user_id: Uuid,
        params: &AccountParams,
    ) -> Result<Self, AccountError> {
        let params = ValidParams::try_from(params)?;
        // La unicidad la garantiza la base, sin carreras (ver `users`).
        monitored_accounts::ActiveModel {
            user_id: ActiveValue::Set(user_id),
            username: ActiveValue::Set(params.username.into()),
            full_name: ActiveValue::Set(params.full_name),
            notes: ActiveValue::Set(params.notes),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(map_unique)
    }

    /// Cuentas de `user_id`, por usuario de Instagram.
    ///
    /// # Errors
    ///
    /// Un error de la base.
    pub async fn list_for_user(db: &DatabaseConnection, user_id: Uuid) -> ModelResult<Vec<Self>> {
        Ok(Entity::find()
            .filter(Column::UserId.eq(user_id))
            .order_by_asc(Column::Username)
            .all(db)
            .await?)
    }

    /// Busca una cuenta de `user_id`. Las de otros usuarios no existen.
    ///
    /// # Errors
    ///
    /// `ModelError::EntityNotFound` si no existe o es de otro usuario, o un
    /// error de la base.
    pub async fn find_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
        id: Uuid,
    ) -> ModelResult<Self> {
        Entity::find_by_id(id)
            .filter(Column::UserId.eq(user_id))
            .one(db)
            .await?
            .ok_or(ModelError::EntityNotFound)
    }

    /// Reemplaza los datos de la cuenta.
    ///
    /// # Errors
    ///
    /// Los mismos que [`Model::create`].
    pub async fn update_details(
        self,
        db: &DatabaseConnection,
        params: &AccountParams,
    ) -> Result<Self, AccountError> {
        let params = ValidParams::try_from(params)?;
        let mut account = self.into_active_model();
        account.username = ActiveValue::Set(params.username.into());
        account.full_name = ActiveValue::Set(params.full_name);
        account.notes = ActiveValue::Set(params.notes);
        account.update(db).await.map_err(map_unique)
    }

    /// Deja de monitorear la cuenta: la borra junto con sus capturas.
    ///
    /// # Errors
    ///
    /// Un error de la base.
    pub async fn remove(self, db: &DatabaseConnection) -> ModelResult<()> {
        self.delete(db).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(username: &str, full_name: Option<&str>, notes: Option<&str>) -> AccountParams {
        AccountParams {
            username: username.to_owned(),
            full_name: full_name.map(str::to_owned),
            notes: notes.map(str::to_owned),
        }
    }

    #[test]
    fn valid_params_trim_and_drop_empty_texts() {
        let valid = ValidParams::try_from(&params(" @Duki ", Some("  "), Some(" hola "))).unwrap();
        assert_eq!(valid.username.as_str(), "duki");
        assert_eq!(valid.full_name, None);
        assert_eq!(valid.notes.as_deref(), Some("hola"));
    }

    #[test]
    fn valid_params_limit_lengths() {
        let long_name = "ñ".repeat(MAX_FULL_NAME_LEN + 1);
        assert!(matches!(
            ValidParams::try_from(&params("duki", Some(&long_name), None)),
            Err(AccountError::FullNameTooLong)
        ));
        let long_notes = "x".repeat(MAX_NOTES_LEN + 1);
        assert!(matches!(
            ValidParams::try_from(&params("duki", None, Some(&long_notes))),
            Err(AccountError::NotesTooLong)
        ));
        assert!(matches!(
            ValidParams::try_from(&params("Du ki", None, None)),
            Err(AccountError::InvalidUsername(UsernameError::InvalidFormat))
        ));
    }
}
