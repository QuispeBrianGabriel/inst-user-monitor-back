//! Adaptador de `PasswordHasher` con Argon2id (recomendación de OWASP).
//!
//! Parámetros por defecto del crate: Argon2id v19, 19 MiB, 2 iteraciones,
//! 1 hilo, sal aleatoria de 16 bytes. Son los mínimos que recomienda OWASP.
//! Los parámetros quedan guardados en cada hash (formato PHC): si se suben
//! en el futuro, los hashes antiguos se siguen verificando.

use argon2::{
    Argon2,
    password_hash::{
        Error as HashError, PasswordHasher as _, PasswordVerifier as _, phc::PasswordHash as Phc,
    },
};
use tokio::task;

use crate::auth::domain::{
    Password, PasswordHash,
    ports::{BoxError, PasswordHasher},
};

#[derive(Clone)]
pub struct Argon2PasswordHasher {
    argon2: Argon2<'static>,
    dummy: PasswordHash,
}

impl Argon2PasswordHasher {
    pub fn new() -> Result<Self, BoxError> {
        let argon2 = Argon2::default();
        let dummy = hash(&argon2, "dummy-password-for-timing")?;
        Ok(Self { argon2, dummy })
    }
}

fn hash(argon2: &Argon2<'_>, password: &str) -> Result<PasswordHash, BoxError> {
    let phc: Phc = argon2.hash_password(password.as_bytes())?;
    Ok(PasswordHash::new(phc.to_string()))
}

impl PasswordHasher for Argon2PasswordHasher {
    async fn hash(&self, password: &Password) -> Result<PasswordHash, BoxError> {
        // Argon2 consume CPU a propósito: fuera del runtime async para no bloquearlo.
        let argon2 = self.argon2.clone();
        let password = password.clone();
        task::spawn_blocking(move || hash(&argon2, password.expose())).await?
    }

    async fn verify(&self, candidate: &str, hash: &PasswordHash) -> Result<bool, BoxError> {
        let argon2 = self.argon2.clone();
        let candidate = candidate.to_owned();
        let hash = hash.as_str().to_owned();
        task::spawn_blocking(move || {
            let phc = Phc::new(&hash)?;
            match argon2.verify_password(candidate.as_bytes(), &phc) {
                Ok(()) => Ok(true),
                Err(HashError::PasswordInvalid) => Ok(false),
                Err(error) => Err(error.into()),
            }
        })
        .await?
    }

    fn dummy_hash(&self) -> &PasswordHash {
        &self.dummy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hashes_with_argon2id_and_verifies() {
        let hasher = Argon2PasswordHasher::new().unwrap();
        let password = Password::parse("Sup3r-Secret!").unwrap();

        let hash = hasher.hash(&password).await.unwrap();
        assert!(hash.as_str().starts_with("$argon2id$"));
        assert!(!hash.as_str().contains(password.expose()));

        assert!(hasher.verify("Sup3r-Secret!", &hash).await.unwrap());
        assert!(!hasher.verify("Wrong-Pass123", &hash).await.unwrap());
    }

    #[tokio::test]
    async fn same_password_gets_different_salt() {
        let hasher = Argon2PasswordHasher::new().unwrap();
        let password = Password::parse("Sup3r-Secret!").unwrap();
        let first = hasher.hash(&password).await.unwrap();
        let second = hasher.hash(&password).await.unwrap();
        assert_ne!(first, second);
    }
}
