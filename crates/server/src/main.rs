#![doc = include_str!("../README.md")]

mod app;
mod auth;
mod config;
mod db;
mod error;
mod handlers;
mod telemetry;

use anyhow::Context as _;
use clap::{Parser, Subcommand};

/// The `organized-koalad` admin CLI (ADR-0004): run the server, or apply/revert migrations.
#[derive(Debug, Parser)]
#[command(name = "organized-koalad", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

/// Admin subcommands. Absence defaults to [`Command::Run`], so a bare invocation serves.
#[derive(Debug, Subcommand)]
enum Command {
    /// Run the long-running HTTP server (default). Never mutates schema unless the dev-only
    /// `OK_AUTO_MIGRATE=1` hatch is set.
    Run,
    /// Apply all pending migrations and exit. Idempotent.
    Migrate,
    /// Revert applied migrations and exit. One step by default; an explicit admin action.
    Rollback {
        /// Number of migrations to revert (most recent first).
        #[arg(long, default_value_t = 1)]
        steps: u32,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Run) {
        Command::Run => run().await,
        Command::Migrate => migrate().await,
        Command::Rollback { steps } => rollback(steps).await,
    }
}

/// Boot the HTTP server: init telemetry, connect the pool, optionally auto-migrate (dev
/// hatch), build the router, and serve until shutdown.
async fn run() -> anyhow::Result<()> {
    let config = config::Config::from_env().context("loading server configuration")?;
    let _telemetry =
        telemetry::init(config.otlp_endpoint.as_deref()).context("initializing telemetry")?;

    let pool = db::connect(&config.database_url)
        .await
        .context("connecting to the database")?;

    if config.auto_migrate {
        tracing::warn!(
            "OK_AUTO_MIGRATE is set: applying pending migrations on the run path (dev-only)"
        );
        db::migrate(&pool).await.context("auto-migrating on boot")?;
    }

    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .with_context(|| format!("binding {}", config.bind_addr))?;
    tracing::info!(addr = %config.bind_addr, "organized-koalad listening");

    let router = app::router(app::AppState::new(pool, config.jwt));
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serving HTTP")?;
    Ok(())
}

/// Apply all pending migrations and exit.
async fn migrate() -> anyhow::Result<()> {
    let database_url = config::database_url_from_env().context("reading the database URL")?;
    let pool = db::connect(&database_url)
        .await
        .context("connecting to the database")?;
    db::migrate(&pool).await.context("applying migrations")?;
    Ok(())
}

/// Revert `steps` applied migrations (most recent first) and exit.
async fn rollback(steps: u32) -> anyhow::Result<()> {
    let database_url = config::database_url_from_env().context("reading the database URL")?;
    let pool = db::connect(&database_url)
        .await
        .context("connecting to the database")?;
    db::rollback(&pool, steps)
        .await
        .context("reverting migrations")?;
    Ok(())
}

/// Resolve when the process receives Ctrl-C, for graceful shutdown.
async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to install Ctrl-C handler");
    }
}
