//! The `organized-koala` TUI binary: a thin shell over the [`tui`] library.
//!
//! Resolves the server base URL, builds the `reqwest`-backed client, spawns the worker thread
//! that owns it, and hands control to the interactive loop — the UI thread drives the pure
//! [`tui::app::App`] core and never blocks on I/O (ADR-0006). Application errors propagate via
//! `anyhow`.

use anyhow::Context;
use tui::app::App;
use tui::client::HttpClient;
use tui::client::worker;
use tui::terminal;

/// Environment variable overriding the server base URL.
const SERVER_URL_ENV: &str = "OK_SERVER_URL";
/// Default server base URL (the server's default bind is `0.0.0.0:8080`).
const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8080";

fn main() -> anyhow::Result<()> {
    let base_url = std::env::var(SERVER_URL_ENV).unwrap_or_else(|_| DEFAULT_SERVER_URL.to_owned());
    let client = HttpClient::new(base_url).context("building the HTTP client")?;
    // Spawn the worker thread owning the real client; the UI thread drives the pure `App` core
    // and never blocks on I/O (ADR-0006). The worker handle is detached — on quit the process
    // exits and the worker holds no state needing flush (hard-constraint #1).
    let (requests, responses, _worker) = worker::spawn(client);
    let app = App::new();
    terminal::run(app, requests, responses)
}
