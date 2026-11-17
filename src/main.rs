//! API de Inst User Monitor.
//!
//! Comandos (primer argumento):
//! - `serve` (por defecto): atiende peticiones HTTP.
//! - `migrate`: aplica las migraciones pendientes y termina. Es un proceso
//!   puntual: se lanza una vez por release, no en cada arranque de `serve`.
//!
//! Variables de entorno:
//! - `DATABASE_URL` (obligatoria): conexión a PostgreSQL.
//! - `PORT` (opcional, 8080): puerto HTTP.
//! - `RUST_LOG` (opcional, info): nivel de logs.
//! - `JWT_SECRET` (obligatoria en `serve`, ≥ 32 caracteres): firma de tokens.
//! - `JWT_TTL_MINUTES` (opcional, 15): duración del token de acceso.

mod app;
mod auth;

use std::{env, error::Error, net::SocketAddr, time::Duration};

use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::{net::TcpListener, signal};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let command = env::args().nth(1).unwrap_or_else(|| "serve".into());
    if !matches!(command.as_str(), "serve" | "migrate") {
        return Err(format!("comando desconocido '{command}': usa serve o migrate").into());
    }
    let database_url = env::var("DATABASE_URL").map_err(|_| "falta la variable DATABASE_URL")?;

    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    if command == "migrate" {
        migrate(pool).await
    } else {
        serve(pool).await
    }
}

/// Aplica las migraciones de migrations/, incrustadas en el binario al compilar.
async fn migrate(pool: PgPool) -> Result<(), Box<dyn Error + Send + Sync>> {
    sqlx::migrate!().run(&pool).await?;
    tracing::info!("migraciones aplicadas");
    Ok(())
}

async fn serve(pool: PgPool) -> Result<(), Box<dyn Error + Send + Sync>> {
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "8080".into()).parse()?;
    let auth = auth::state(pool.clone(), &auth::AuthConfig::from_env()?)?;

    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("API escuchando en {}", listener.local_addr()?);

    // Con la IP de cada conexión: la usa el rate limit de auth.
    let app = app::router(pool, auth).into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("API detenida");
    Ok(())
}

/// Espera a Ctrl+C (desarrollo) o SIGTERM (`docker stop`) para cerrar en orden.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.ok();
    };
    let terminate = async {
        if let Ok(mut sigterm) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
            sigterm.recv().await;
        }
    };
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
