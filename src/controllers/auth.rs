//! `/auth`: registro, login y usuario actual.
//!
//! Los handlers solo traducen HTTP ⇄ modelo; la lógica vive en
//! `models::users`. Los errores siguen el formato de Loco:
//! `{"error": "<código>", "description": "<texto>"}`. El código es estable
//! (para el frontend); el texto es legible y puede cambiar.

use std::{net::IpAddr, time::Duration};

use axum::{
    extract::Request,
    http::{StatusCode, header},
    middleware::{self, Next},
};
use loco_rs::{
    controller::{ErrorDetail, middleware::remote_ip::RemoteIP},
    prelude::*,
};
use tower_governor::{
    GovernorError, GovernorLayer,
    governor::GovernorConfigBuilder,
    key_extractor::{KeyExtractor, PeerIpKeyExtractor},
};

use crate::{
    models::users::{self, AuthError, LoginParams, RegisterParams},
    views::auth::{TokenResponse, UserResponse},
};

/// Peticiones seguidas permitidas por IP en register y login.
const RATE_LIMIT_BURST: u32 = 5;
/// Cada cuántos segundos se recupera una petición (≈ 10 por minuto).
const RATE_LIMIT_REFILL_SECS: u64 = 6;

impl From<AuthError> for Error {
    fn from(error: AuthError) -> Self {
        let (status, code) = match error {
            // Loco lo registra en el log y responde 500 sin dar detalles.
            AuthError::Model(error) => return error.into(),
            AuthError::InvalidEmail(_) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_email"),
            AuthError::WeakPassword(_) => (StatusCode::UNPROCESSABLE_ENTITY, "weak_password"),
            AuthError::EmailTaken => (StatusCode::CONFLICT, "email_taken"),
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "invalid_credentials"),
        };
        Self::CustomError(status, ErrorDetail::new(code, error.to_string()))
    }
}

/// `POST /auth/register` → 201 con el usuario creado.
#[debug_handler]
async fn register(
    State(ctx): State<AppContext>,
    Json(params): Json<RegisterParams>,
) -> Result<Response> {
    let user = users::Model::create_with_password(&ctx.db, &params).await?;
    format::render()
        .status(StatusCode::CREATED)
        .json(UserResponse::from(&user))
}

/// `POST /auth/login` → 200 con el token de acceso.
#[debug_handler]
async fn login(State(ctx): State<AppContext>, Json(params): Json<LoginParams>) -> Result<Response> {
    let user = users::Model::find_by_credentials(&ctx.db, &params).await?;
    let jwt = ctx.config.get_jwt_config()?;
    let token = user.generate_jwt(&jwt.secret, jwt.expiration)?;
    // RFC 6749 §5.1: las respuestas con tokens no se cachean.
    format::render()
        .header(header::CACHE_CONTROL, "no-store")
        .json(TokenResponse::bearer(token, jwt.expiration))
}

/// `GET /auth/me` → 200 con el usuario del token. Sin token válido, o si el
/// usuario ya no existe, el extractor responde 401.
#[debug_handler]
async fn me(
    auth: auth::JWTWithUser<users::Model>,
    State(_ctx): State<AppContext>,
) -> Result<Response> {
    format::json(UserResponse::from(&auth.user))
}

/// Clave del rate limit: la IP del cliente según el middleware `remote_ip`
/// de Loco (`server.middlewares.remote_ip` en `config/*.yaml`). Detrás de un
/// proxy, la de la conexión es la del proxy: todos compartirían límite.
#[derive(Clone)]
struct ClientIp;

impl KeyExtractor for ClientIp {
    type Key = IpAddr;

    fn extract<T>(&self, request: &axum::http::Request<T>) -> Result<IpAddr, GovernorError> {
        match request.extensions().get::<RemoteIP>() {
            Some(RemoteIP::Forwarded(ip) | RemoteIP::Socket(ip)) => Ok(*ip),
            _ => PeerIpKeyExtractor.extract(request),
        }
    }
}

/// Resuelve `RemoteIP` (un extractor async) y lo deja en la petición, donde
/// lo lee [`ClientIp`] (síncrono) antes de llegar al handler.
async fn resolve_client_ip(remote_ip: RemoteIP, mut request: Request, next: Next) -> Response {
    request.extensions_mut().insert(remote_ip);
    next.run(request).await
}

/// Rutas de `/auth`. Register y login tienen límite de peticiones por IP de
/// cliente contra fuerza bruta (429 al superarlo), compartido entre ambas;
/// `me` no.
///
/// # Panics
///
/// Nunca: las constantes del rate limit son mayores que cero.
pub fn routes() -> Routes {
    let rate_limit = GovernorConfigBuilder::default()
        .key_extractor(ClientIp)
        .per_second(RATE_LIMIT_REFILL_SECS)
        .burst_size(RATE_LIMIT_BURST)
        .finish()
        .expect("las constantes del rate limit son mayores que cero");

    // Limpieza del estado en memoria del limitador: olvida las IPs que ya no
    // están limitadas, para que no crezca sin fin. No es trabajo de fondo
    // (no hay nada que persistir ni reintentar): vive y muere con el proceso.
    let limiter = rate_limit.limiter().clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            limiter.retain_recent();
        }
    });

    let rate_limited = Routes::new()
        .add("/register", post(register))
        .add("/login", post(login))
        .layer(GovernorLayer::new(rate_limit))
        // La última capa es la exterior: resuelve la IP antes del limitador.
        .layer(middleware::from_fn(resolve_client_ip));

    Routes::new()
        .prefix("/auth")
        .merge(rate_limited)
        .add("/me", get(me))
}
