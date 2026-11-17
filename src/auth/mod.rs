//! Módulo de autenticación (arquitectura hexagonal):
//!
//! - `domain`: entidad `User`, value objects (`Email`, `Password`) y puertos.
//! - `application`: casos de uso (`AuthService`).
//! - `infrastructure`: adaptadores (PostgreSQL, Argon2, JWT).
//! - `http`: adaptador de entrada (rutas, DTOs, extractor `AuthUser`).
//!
//! Este archivo hace el cableado: elige qué adaptador implementa cada puerto.

pub mod application;
pub mod domain;
pub mod http;
pub mod infrastructure;

use std::{env, sync::Arc, time::Duration};

use sqlx::PgPool;

pub use http::router;

use application::AuthService;
use domain::ports::BoxError;
use infrastructure::{Argon2PasswordHasher, JwtTokenService, PostgresUserRepository};

/// Servicio con los adaptadores de producción.
pub type AppAuthService =
    AuthService<PostgresUserRepository, Argon2PasswordHasher, JwtTokenService>;

/// Estado compartido por los handlers de auth.
pub type AuthState = Arc<AppAuthService>;

/// Configuración de auth.
pub struct AuthConfig {
    jwt_secret: String,
    token_ttl: Duration,
}

impl AuthConfig {
    /// Mínimo para HS256 (RFC 7518 §3.2: clave ≥ tamaño del hash, 256 bits).
    const MIN_SECRET_LEN: usize = 32;
    const DEFAULT_TTL_MINUTES: u64 = 15;
    /// Un token de acceso es de vida corta: como mucho, un día.
    const MAX_TTL: Duration = Duration::from_secs(24 * 60 * 60);

    pub fn new(jwt_secret: impl Into<String>, token_ttl: Duration) -> Result<Self, BoxError> {
        let jwt_secret = jwt_secret.into();
        if jwt_secret.len() < Self::MIN_SECRET_LEN {
            return Err(format!(
                "JWT_SECRET debe tener al menos {} caracteres",
                Self::MIN_SECRET_LEN
            )
            .into());
        }
        if token_ttl.is_zero() || token_ttl > Self::MAX_TTL {
            return Err("la duración del token debe estar entre 1 minuto y 1 día".into());
        }
        Ok(Self {
            jwt_secret,
            token_ttl,
        })
    }

    /// Lee `JWT_SECRET` (obligatoria) y `JWT_TTL_MINUTES` (opcional, 15).
    pub fn from_env() -> Result<Self, BoxError> {
        let secret = env::var("JWT_SECRET").map_err(|_| "falta la variable JWT_SECRET")?;
        let minutes: u64 = match env::var("JWT_TTL_MINUTES") {
            Ok(value) => value
                .parse()
                .map_err(|_| "JWT_TTL_MINUTES debe ser un número de minutos")?,
            Err(_) => Self::DEFAULT_TTL_MINUTES,
        };
        let ttl = Duration::from_secs(minutes.saturating_mul(60));
        Self::new(secret, ttl)
    }
}

/// Construye el servicio de auth con sus adaptadores.
pub fn state(pool: PgPool, config: &AuthConfig) -> Result<AuthState, BoxError> {
    let service = AuthService::new(
        PostgresUserRepository::new(pool),
        Argon2PasswordHasher::new()?,
        JwtTokenService::new(config.jwt_secret.as_bytes(), config.token_ttl)?,
    );
    Ok(Arc::new(service))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_rejects_short_secret() {
        assert!(AuthConfig::new("short", Duration::from_secs(900)).is_err());
        assert!(AuthConfig::new("x".repeat(32), Duration::from_secs(900)).is_ok());
    }

    #[test]
    fn config_rejects_out_of_range_ttl() {
        let secret = "x".repeat(32);
        assert!(AuthConfig::new(secret.clone(), Duration::ZERO).is_err());
        assert!(AuthConfig::new(secret, Duration::from_secs(25 * 60 * 60)).is_err());
    }
}
