//! Valida la configuración de JWT (`auth.jwt` en `config/*.yaml`), que Loco
//! acepta sin comprobar:
//!
//! - `secret`: base64 de al menos 64 bytes. Loco firma con HS512 y el
//!   RFC 7518 §3.2 exige una clave de, como mínimo, el tamaño del hash.
//! - `expiration`: entre 1 minuto y 1 día. Un token de acceso es de vida corta.
//!
//! Con una configuración inválida el servidor no arranca, y `cargo loco
//! doctor` lo señala.

use std::ops::RangeInclusive;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use loco_rs::{
    Error, Result,
    app::{AppContext, Initializer},
    config,
    doctor::{Check, CheckStatus},
};

pub struct JwtConfig;

const MIN_SECRET_BYTES: usize = 64;
const EXPIRATION_SECS: RangeInclusive<u64> = 60..=24 * 60 * 60;
const HOW_TO_FIX: &str = "genera un secreto con `make jwt-secret` y ponlo en JWT_SECRET";

#[async_trait::async_trait]
impl Initializer for JwtConfig {
    fn name(&self) -> String {
        "jwt-config".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        validate(ctx.config.get_jwt_config()?).map_err(Error::Message)
    }

    async fn check(&self, ctx: &AppContext) -> Result<Option<Check>> {
        let check = match validate(ctx.config.get_jwt_config()?) {
            Ok(()) => Check {
                status: CheckStatus::Ok,
                message: "JWT config: ok".to_string(),
                description: None,
            },
            Err(problem) => Check {
                status: CheckStatus::NotOk,
                message: "JWT config: invalid".to_string(),
                description: Some(problem),
            },
        };
        Ok(Some(check))
    }
}

fn validate(jwt: &config::JWT) -> std::result::Result<(), String> {
    let key = STANDARD
        .decode(&jwt.secret)
        .map_err(|_| format!("auth.jwt.secret (JWT_SECRET) no es base64 válido: {HOW_TO_FIX}"))?;
    if key.len() < MIN_SECRET_BYTES {
        return Err(format!(
            "auth.jwt.secret (JWT_SECRET) tiene {} bytes y el mínimo es {MIN_SECRET_BYTES}: {HOW_TO_FIX}",
            key.len()
        ));
    }
    if !EXPIRATION_SECS.contains(&jwt.expiration) {
        return Err(format!(
            "auth.jwt.expiration debe estar entre {} y {} segundos (JWT_TTL_MINUTES entre 1 y 1440)",
            EXPIRATION_SECS.start(),
            EXPIRATION_SECS.end()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jwt(secret: &str, expiration: u64) -> config::JWT {
        config::JWT {
            location: None,
            secret: secret.to_string(),
            expiration,
        }
    }

    fn secret(bytes: usize) -> String {
        STANDARD.encode(vec![7u8; bytes])
    }

    #[test]
    fn accepts_a_long_base64_secret_and_short_ttl() {
        assert!(validate(&jwt(&secret(64), 900)).is_ok());
    }

    #[test]
    fn rejects_short_or_non_base64_secrets() {
        assert!(validate(&jwt(&secret(63), 900)).is_err());
        assert!(validate(&jwt("not base64!", 900)).is_err());
    }

    #[test]
    fn rejects_out_of_range_expiration() {
        assert!(validate(&jwt(&secret(64), 59)).is_err());
        assert!(validate(&jwt(&secret(64), 24 * 60 * 60 + 1)).is_err());
    }
}
