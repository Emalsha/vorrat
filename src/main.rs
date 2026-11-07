use std::sync::Arc;

use anyhow::Result;
use tracing_subscriber::EnvFilter;
use vorrat::{config::Config, db::Store, scheduler, web};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let cfg = Config::from_env()?;
    std::fs::create_dir_all(&cfg.data_dir)?;
    let store = Store::open(cfg.db_path())?;
    tracing::info!(db = %cfg.db_path().display(), schema = store.schema_version()?, "database ready");
    if cfg.demo {
        vorrat::demo::reset(&store, vorrat::db::today())?;
        tracing::info!("demo mode: sample data loaded, resets nightly");
    }
    let listen = cfg.listen;
    let state = Arc::new(web::AppState::new(store, cfg));
    tracing::info!(push = state.notifier.is_some(), backup = ?state.cfg.backup_dir, "jobs");
    tokio::spawn(scheduler::run(state.clone()));
    let listener = tokio::net::TcpListener::bind(listen).await?;
    tracing::info!(
        "Vorrat {} listening on http://{}",
        env!("CARGO_PKG_VERSION"),
        listen
    );
    axum::serve(listener, web::router(state))
        .with_graceful_shutdown(shutdown())
        .await?;
    tracing::info!("stopped");
    Ok(())
}

/// Ctrl-C, or SIGTERM from `docker stop` and systemd. As PID 1 in a container
/// SIGTERM is otherwise ignored, and Docker waits 10 s before killing.
async fn shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
