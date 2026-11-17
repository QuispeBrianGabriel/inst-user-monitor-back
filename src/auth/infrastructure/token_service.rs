//! Adaptador de `TokenService` con JWT (RFC 7519) firmado con HS256.
//!
//! Claims: `sub` (id del usuario), `iat`, `exp`, `iss` y `aud`. Al verificar
//! se exige el algoritmo exacto (no se acepta `none` ni otro), la firma, que
//! no haya caducado y que emisor y audiencia sean los nuestros.

use std::time::Duration;

use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode, get_current_timestamp,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::domain::ports::{AccessToken, BoxError, TokenService};

const ALGORITHM: Algorithm = Algorithm::HS256;
const ISSUER: &str = "inst-user-monitor";
const AUDIENCE: &str = "inst-user-monitor-api";

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: Uuid,
    iat: u64,
    exp: u64,
    iss: String,
    aud: String,
}

pub struct JwtTokenService {
    encoding: EncodingKey,
    decoding: DecodingKey,
    validation: Validation,
    ttl: Duration,
}

impl JwtTokenService {
    pub fn new(secret: &[u8], ttl: Duration) -> Result<Self, BoxError> {
        let mut validation = Validation::new(ALGORITHM);
        validation.set_issuer(&[ISSUER]);
        validation.set_audience(&[AUDIENCE]);
        validation.set_required_spec_claims(&["exp", "sub", "iss", "aud"]);
        // Tolerancia pequeña al desfase de reloj entre servidores.
        validation.leeway = 30;

        Ok(Self {
            encoding: EncodingKey::from_secret(secret)?,
            decoding: DecodingKey::from_secret(secret)?,
            validation,
            ttl,
        })
    }
}

impl TokenService for JwtTokenService {
    fn issue(&self, user_id: Uuid) -> Result<AccessToken, BoxError> {
        let now = get_current_timestamp();
        let expires_in = self.ttl.as_secs();
        let claims = Claims {
            sub: user_id,
            iat: now,
            exp: now + expires_in,
            iss: ISSUER.into(),
            aud: AUDIENCE.into(),
        };
        let token = encode(&Header::new(ALGORITHM), &claims, &self.encoding)?;
        Ok(AccessToken { token, expires_in })
    }

    fn verify(&self, token: &str) -> Option<Uuid> {
        decode::<Claims>(token, &self.decoding, &self.validation)
            .ok()
            .map(|data| data.claims.sub)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"test-secret-with-at-least-32-bytes!!";

    fn service() -> JwtTokenService {
        JwtTokenService::new(SECRET, Duration::from_secs(900)).unwrap()
    }

    #[test]
    fn issued_token_verifies() {
        let id = Uuid::now_v7();
        let token = service().issue(id).unwrap();
        assert_eq!(token.expires_in, 900);
        assert_eq!(service().verify(&token.token), Some(id));
    }

    #[test]
    fn rejects_token_signed_with_other_secret() {
        let other = JwtTokenService::new(
            b"another-secret-with-32-bytes-or-more",
            Duration::from_secs(900),
        )
        .unwrap();
        let token = other.issue(Uuid::now_v7()).unwrap();
        assert_eq!(service().verify(&token.token), None);
    }

    #[test]
    fn rejects_expired_token() {
        let now = get_current_timestamp();
        let claims = Claims {
            sub: Uuid::now_v7(),
            iat: now - 3600,
            exp: now - 600,
            iss: ISSUER.into(),
            aud: AUDIENCE.into(),
        };
        let key = EncodingKey::from_secret(SECRET).unwrap();
        let token = encode(&Header::new(ALGORITHM), &claims, &key).unwrap();
        assert_eq!(service().verify(&token), None);
    }

    #[test]
    fn rejects_garbage_and_tampered_tokens() {
        let token = service().issue(Uuid::now_v7()).unwrap().token;
        let tampered = format!("{token}x");
        assert_eq!(service().verify("not-a-jwt"), None);
        assert_eq!(service().verify(&tampered), None);
    }
}
