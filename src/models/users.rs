//! Usuarios: registro, login y búsquedas.
//!
//! Las reglas de entrada viven en los submódulos `email` (formato y
//! normalización) y `password` (política de contraseñas). El hash es Argon2id
//! (`loco_rs::hash`, parámetros recomendados por OWASP) en formato PHC: los
//! parámetros quedan guardados en cada hash, así que si se suben en el futuro
//! los hashes antiguos se siguen verificando.

mod email;
mod password;

use std::result::Result;

use loco_rs::{auth::jwt, hash, prelude::*};
use sea_orm::SqlErr;
use serde::Deserialize;
use serde_json::Map;
use tokio::task;

pub use self::{
    email::{Email, EmailError},
    password::{Password, PasswordError},
};
pub use super::_entities::users::{self, ActiveModel, Entity, Model};

/// Restricción UNIQUE del email (ver la migración de `users`).
const UNIQUE_EMAIL_CONSTRAINT: &str = "users_email_key";

/// Hash Argon2id válido de una contraseña cualquiera, con los mismos
/// parámetros que `hash::hash_password`. Se verifica contra él cuando el email
/// no existe, para que el login tarde lo mismo exista o no (si no, el tiempo
/// de respuesta revelaría qué emails están registrados).
const DUMMY_PASSWORD_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$o0pwYOdvfMk0AMZC+aCjSA$NwmAFywU3GvC3ND1ZLhBI4CyorApp3X1zijcagtc0QA";

/// Cuerpo de `POST /auth/register`.
///
/// Sin `Debug` a propósito: lleva la contraseña en claro y no debe acabar en
/// un log.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterParams {
    pub email: String,
    pub password: String,
}

/// Cuerpo de `POST /auth/login`. Sin `Debug`, como [`RegisterParams`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginParams {
    pub email: String,
    pub password: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error(transparent)]
    InvalidEmail(#[from] EmailError),
    #[error(transparent)]
    WeakPassword(#[from] PasswordError),
    #[error("el email ya está registrado")]
    EmailTaken,
    /// Email o contraseña incorrectos. El mismo error en ambos casos, a propósito.
    #[error("credenciales inválidas")]
    InvalidCredentials,
    #[error(transparent)]
    Model(#[from] ModelError),
}

impl From<DbErr> for AuthError {
    fn from(error: DbErr) -> Self {
        Self::Model(error.into())
    }
}

#[derive(Debug, Validate)]
pub struct Validator {
    /// `None` cuando la operación no toca el email.
    #[validate(custom(function = "email::validate_normalized"))]
    pub email: Option<String>,
}

impl Validatable for ActiveModel {
    fn validator(&self) -> Box<dyn Validate> {
        Box::new(Validator {
            email: self.email.try_as_ref().cloned(),
        })
    }
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    // `updated_at` no se toca aquí: lo mantiene un trigger de la base.
    async fn before_save<C>(self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        self.validate()?;
        let mut this = self;
        if insert && !this.id.is_set() {
            this.id = ActiveValue::Set(Uuid::now_v7());
        }
        Ok(this)
    }
}

/// Lo que necesitan los extractores de auth de Loco (`auth::JWTWithUser`).
#[async_trait]
impl Authenticable for Model {
    /// La API no emite API keys: ninguna identifica a un usuario.
    async fn find_by_api_key(_db: &DatabaseConnection, _api_key: &str) -> ModelResult<Self> {
        Err(ModelError::EntityNotFound)
    }

    /// El `pid` de los claims del JWT es el id del usuario.
    async fn find_by_claims_key(db: &DatabaseConnection, claims_key: &str) -> ModelResult<Self> {
        let id = Uuid::parse_str(claims_key).map_err(|_| ModelError::EntityNotFound)?;
        Self::find_by_id(db, id).await
    }
}

impl Model {
    /// Busca un usuario por su id.
    ///
    /// # Errors
    ///
    /// `ModelError::EntityNotFound` si no existe, o un error de la base.
    pub async fn find_by_id(db: &DatabaseConnection, id: Uuid) -> ModelResult<Self> {
        users::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(ModelError::EntityNotFound)
    }

    /// Busca un usuario por su email (ya normalizado).
    ///
    /// # Errors
    ///
    /// `ModelError::EntityNotFound` si no existe, o un error de la base.
    pub async fn find_by_email(db: &DatabaseConnection, email: &Email) -> ModelResult<Self> {
        users::Entity::find()
            .filter(
                query::condition()
                    .eq(users::Column::Email, email.as_str())
                    .build(),
            )
            .one(db)
            .await?
            .ok_or(ModelError::EntityNotFound)
    }

    /// Registra un usuario: valida el email y la contraseña, y guarda el hash.
    ///
    /// # Errors
    ///
    /// `InvalidEmail` o `WeakPassword` si la entrada no es válida, `EmailTaken`
    /// si el email ya existe, o un error inesperado de la base o del hash.
    pub async fn create_with_password(
        db: &DatabaseConnection,
        params: &RegisterParams,
    ) -> Result<Self, AuthError> {
        let email = Email::parse(&params.email)?;
        let password = Password::parse(&params.password)?;
        let password_hash = hash_password(password).await?;

        // La unicidad la garantiza la base: sin carreras entre comprobar e
        // insertar, y sin una consulta extra.
        users::ActiveModel {
            email: ActiveValue::Set(email.into()),
            password_hash: ActiveValue::Set(password_hash),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|error| match error.sql_err() {
            Some(SqlErr::UniqueConstraintViolation(detail))
                if detail.contains(UNIQUE_EMAIL_CONSTRAINT) =>
            {
                AuthError::EmailTaken
            }
            _ => error.into(),
        })
    }

    /// Comprueba unas credenciales y devuelve su usuario.
    ///
    /// No aplica la política de contraseñas (puede cambiar y dejar fuera a
    /// usuarios antiguos), solo el límite de tamaño.
    ///
    /// # Errors
    ///
    /// `InvalidCredentials` si el email no existe o la contraseña no
    /// corresponde (el mismo error, a propósito), o un error inesperado.
    pub async fn find_by_credentials(
        db: &DatabaseConnection,
        params: &LoginParams,
    ) -> Result<Self, AuthError> {
        let Ok(email) = Email::parse(&params.email) else {
            return Err(AuthError::InvalidCredentials);
        };
        if params.password.len() > Password::MAX_LEN {
            return Err(AuthError::InvalidCredentials);
        }

        let user = match Self::find_by_email(db, &email).await {
            Ok(user) => Some(user),
            Err(ModelError::EntityNotFound) => None,
            Err(error) => return Err(error.into()),
        };
        let hash = user
            .as_ref()
            .map_or(DUMMY_PASSWORD_HASH, |user| user.password_hash.as_str());
        let matches = verify_password(&params.password, hash).await?;

        match user {
            Some(user) if matches => Ok(user),
            _ => Err(AuthError::InvalidCredentials),
        }
    }

    /// Emite un token de acceso (JWT) para el usuario.
    ///
    /// # Errors
    ///
    /// Cuando no se puede firmar el token (secreto inválido).
    pub fn generate_jwt(&self, secret: &str, expiration: u64) -> ModelResult<String> {
        Ok(jwt::JWT::new(secret).generate_token(expiration, self.id.to_string(), Map::new())?)
    }
}

// Argon2 consume CPU a propósito: se ejecuta fuera del runtime async para no
// bloquear al resto de peticiones.

async fn hash_password(password: Password) -> Result<String, AuthError> {
    task::spawn_blocking(move || hash::hash_password(password.expose()))
        .await
        .map_err(|error| ModelError::Any(error.into()))?
        .map_err(|error| ModelError::Any(error.into()).into())
}

async fn verify_password(candidate: &str, hash: &str) -> Result<bool, AuthError> {
    let (candidate, hash) = (candidate.to_owned(), hash.to_owned());
    task::spawn_blocking(move || hash::verify_password(&candidate, &hash))
        .await
        .map_err(|error| ModelError::Any(error.into()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El hash señuelo debe costar lo mismo que uno real: mismos parámetros.
    #[test]
    fn dummy_hash_uses_current_argon2_params() {
        let params = |phc: &str| phc.rsplitn(3, '$').nth(2).map(str::to_owned);
        let real = hash::hash_password("Sup3r-Secret!").unwrap();
        assert_eq!(params(DUMMY_PASSWORD_HASH), params(&real));
        assert!(!hash::verify_password("Sup3r-Secret!", DUMMY_PASSWORD_HASH));
    }
}
