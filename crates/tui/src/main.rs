//! The `organized-koala` TUI binary: a thin shell over the [`tui`] library.
//!
//! Resolves the server base URL, builds the `reqwest`-backed client, performs an initial
//! health probe so an unreachable server is reported up front, and hands control to the
//! interactive loop. Application errors propagate via `anyhow`.

use anyhow::Context;
use tui::app::App;
use tui::client::HttpClient;
use tui::terminal;

/// Environment variable overriding the server base URL.
const SERVER_URL_ENV: &str = "OK_SERVER_URL";
/// Default server base URL (the server's default bind is `0.0.0.0:8080`).
const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8080";

fn main() -> anyhow::Result<()> {
    let base_url = std::env::var(SERVER_URL_ENV).unwrap_or_else(|_| DEFAULT_SERVER_URL.to_owned());
    let client = HttpClient::new(base_url).context("building the HTTP client")?;
    let app = App::new(client);
    terminal::run(app)
}
