//! API de Inst User Monitor.
//!
//! Variables de entorno:
//! - `DATABASE_URL` (obligatoria): conexión a PostgreSQL.
//! - `PORT` (opcional, 8080): puerto HTTP.
//! - `RUST_LOG` (opcional, info): nivel de logs.

mod app;

use std::{env, error::Error, time::Duration};

use sqlx::postgres::PgPoolOptions;
use tokio::{net::TcpListener, signal};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let database_url = env::var("DATABASE_URL").map_err(|_| "falta la variable DATABASE_URL")?;
    let port: u16 = env::var("PORT").unwrap_or_else(|_| "8080".into()).parse()?;

    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    // Aplica las migraciones de migrations/, incrustadas en el binario al compilar.
    sqlx::migrate!().run(&pool).await?;

    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("API escuchando en {}", listener.local_addr()?);

    axum::serve(listener, app::router(pool))
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
