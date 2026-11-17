//! Puertos: lo que el dominio necesita del exterior, sin saber cómo se hace.
//! Las implementaciones (adaptadores) viven en `infrastructure`.

use std::{error::Error, future::Future};

use thiserror::Error;
use uuid::Uuid;

use super::{Email, Password, PasswordHash, User};

pub type BoxError = Box<dyn Error + Send + Sync>;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("el email ya está registrado")]
    EmailTaken,
    #[error(transparent)]
    Unexpected(BoxError),
}

/// Persistencia de usuarios.
pub trait UserRepository: Send + Sync {
    /// Guarda un usuario nuevo. `EmailTaken` si el email ya existe.
    fn create(&self, user: &User) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    fn find_by_email(
        &self,
        email: &Email,
    ) -> impl Future<Output = Result<Option<User>, RepositoryError>> + Send;

    fn find_by_id(
        &self,
        id: Uuid,
    ) -> impl Future<Output = Result<Option<User>, RepositoryError>> + Send;
}

/// Hash y verificación de contraseñas.
pub trait PasswordHasher: Send + Sync {
    fn hash(
        &self,
        password: &Password,
    ) -> impl Future<Output = Result<PasswordHash, BoxError>> + Send;

    /// `true` si `candidate` corresponde a `hash`.
    fn verify(
        &self,
        candidate: &str,
        hash: &PasswordHash,
    ) -> impl Future<Output = Result<bool, BoxError>> + Send;

    /// Hash válido de una contraseña cualquiera. Se verifica contra él cuando
    /// el email no existe, para que el login tarde lo mismo exista o no
    /// (si no, el tiempo de respuesta revelaría qué emails están registrados).
    fn dummy_hash(&self) -> &PasswordHash;
}

/// Token de acceso emitido tras un login.
#[derive(Debug, Clone)]
pub struct AccessToken {
    pub token: String,
    /// Segundos hasta que caduca.
    pub expires_in: u64,
}

/// Emisión y verificación de tokens de acceso.
pub trait TokenService: Send + Sync {
    fn issue(&self, user_id: Uuid) -> Result<AccessToken, BoxError>;

    /// Id del usuario si el token es válido (firma, caducidad, emisor…).
    fn verify(&self, token: &str) -> Option<Uuid>;
}
