//! The request worker: the single edge thread that owns the real [`Client`] and executes
//! [`Dispatch`]es off the UI thread (ADR-0006 Model A).
//!
//! The UI thread sends a [`Dispatch`] over an `mpsc` channel; the worker runs the matching
//! synchronous [`Client`] call and sends back a [`ClientResponse`] echoing the
//! [`RequestId`](crate::app::RequestId).
//! Because the blocking client call happens here, the UI thread is never parked on I/O and stays
//! free to redraw, animate a spinner, and honour cancel/quit. This is edge code — like
//! [`crate::terminal::run`] — not part of the pure `App` core, so it is not covered by the
//! `TestBackend` suite.

use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};

use crate::app::Dispatch;
use crate::app::protocol::{ClientRequest, ClientResponse, Outcome};
use crate::client::Client;

/// Run one [`ClientRequest`] against the client, producing its [`Outcome`].
fn run<C: Client>(client: &C, request: ClientRequest) -> Outcome {
    match request {
        ClientRequest::Health => Outcome::Health(client.health()),
        ClientRequest::Register(req) => Outcome::Register(client.register(&req)),
        ClientRequest::Login(req) => Outcome::Login(client.login(&req)),
        ClientRequest::ListProfiles { token } => {
            let result = client.list_profiles(&token);
            Outcome::ListProfiles { token, result }
        }
        ClientRequest::ListTasks { token, profile_id } => {
            Outcome::ListTasks(client.list_tasks(&token, &profile_id))
        }
        ClientRequest::CreateTask {
            token,
            profile_id,
            req,
        } => Outcome::CreateTask(client.create_task(&token, &profile_id, &req)),
        ClientRequest::CloseTask {
            token,
            profile_id,
            task_id,
        } => Outcome::CloseTask(client.close_task(&token, &profile_id, &task_id)),
    }
}

/// The worker loop body: receive [`Dispatch`]es until the UI side hangs up, executing each and
/// returning its [`ClientResponse`]. Stamps the response with the request's [`RequestId`] so the
/// UI can drop a stale (cancelled) response. If the response channel is closed (UI quit), it
/// exits.
fn serve<C: Client>(client: C, rx: Receiver<Dispatch>, tx: Sender<ClientResponse>) {
    while let Ok(Dispatch { id, request }) = rx.recv() {
        let outcome = run(&client, request);
        if tx.send(ClientResponse { id, outcome }).is_err() {
            // UI thread has gone away; nothing more to do.
            break;
        }
    }
}

/// Spawns the worker thread owning `client`, returning the [`Sender`] the UI uses to dispatch
/// requests, the [`Receiver`] it drains for responses, and the thread handle.
///
/// The UI thread keeps the returned [`Sender`]; dropping it signals the worker to exit. The
/// worker runs for the process lifetime and holds no state needing flush, so on quit it can be
/// detached (the handle dropped) and the process exits cleanly (hard-constraint #1).
pub fn spawn<C: Client + Send + 'static>(
    client: C,
) -> (Sender<Dispatch>, Receiver<ClientResponse>, JoinHandle<()>) {
    let (req_tx, req_rx) = std::sync::mpsc::channel::<Dispatch>();
    let (resp_tx, resp_rx) = std::sync::mpsc::channel::<ClientResponse>();
    let handle = thread::spawn(move || serve(client, req_rx, resp_tx));
    (req_tx, resp_rx, handle)
}
