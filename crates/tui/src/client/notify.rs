//! The desktop-notification effect: a narrow injected [`Notifier`] seam and its production
//! [`DesktopNotifier`] over [`notify-rust`](notify_rust).
//!
//! This is **edge** code — an external effect modelled exactly like the [`Client`](super::Client)
//! transport seam (ADR-0003 / rust-standards "separate the pure core from the effectful shell").
//! The pure app core never calls a notifier; it emits a one-shot signal and the poll loop performs
//! the effect through an injected [`Notifier`]. Tests inject a spy; the binary injects
//! [`DesktopNotifier`].

/// Fires a single transient desktop notification when a focus session completes.
///
/// **Best-effort by contract:** any delivery failure (no notification daemon, an unsupported
/// platform, a closed D-Bus session) is swallowed by the implementation — it is never an error
/// the caller must handle and never fatal to the TUI. Implementations must **not** write to
/// stdout/stderr: the alt-screen TUI owns the terminal, so any such write would corrupt the
/// display (Assumption A2).
pub trait Notifier {
    /// Fire one plain, sound-less, button-less desktop notification with the given title and body.
    fn notify_timer_complete(&self, title: &str, body: &str);
}

/// Production [`Notifier`] over [`notify-rust`](notify_rust): a plain title+body notification with
/// no sound, no action buttons, and no rich content.
///
/// Every delivery error is mapped to a no-op (see the trait's best-effort contract) — a missing
/// daemon or unsupported platform degrades silently and never blocks the TUI.
#[derive(Debug, Default, Clone, Copy)]
pub struct DesktopNotifier;

impl DesktopNotifier {
    /// A new notifier. Stateless — construction performs no I/O and never fails.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Notifier for DesktopNotifier {
    fn notify_timer_complete(&self, title: &str, body: &str) {
        // Best-effort: build a plain, sound-less notification and show it; map any delivery error
        // to a no-op. Nothing is written to the terminal (Assumption A2). The `Result` is
        // deliberately discarded — a failed delivery is non-fatal by contract.
        let _ = notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .show();
    }
}
