//! `--ibus`: the program as ibus-daemon runs it (`linux.ibus.factory`).
//! It connects to ibus-daemon's own bus, exports the factory, owns the
//! component's bus name, and serves until the connection closes or a
//! poisoned process has no engine object left (`linux.ibus.panic`).
//!
//! zbus runs on tokio's current-thread runtime and spawns no task per
//! call, so one thread handles every call in the order ibus-daemon sends
//! them, and a key's signals and reply never interleave with another
//! call's.

pub mod engine;
pub mod factory;
pub mod text;

use std::future::{Future, poll_fn};
use std::path::PathBuf;
use std::pin::pin;
use std::process::ExitCode;
use std::sync::Arc;
use std::task::Poll;

use tokio::sync::Notify;

use self::factory::Factory;
use crate::address::{self, System};
use crate::diag::{self, Event};
use crate::guard::{self, Process};

/// The component's name, which ibus-daemon waits for an owner of.
pub const BUS_NAME: &str = "org.freedesktop.IBus.Divvun";
pub const FACTORY_PATH: &str = "/org/freedesktop/IBus/Factory";

/// Serves the engines `--xml` lists from `roots`.
// [spec:kbdgen:req:linux.ibus.factory]
pub fn serve(roots: Vec<PathBuf>) -> ExitCode {
    guard::install_hook();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            diag::record(Event::Failed("cannot start an async runtime"));
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(run(roots))
}

async fn run(roots: Vec<PathBuf>) -> ExitCode {
    let Some(address) = address::address(&System) else {
        diag::record(Event::Failed("found no address of ibus-daemon"));
        return ExitCode::FAILURE;
    };
    let process = Arc::new(Process::default());
    let exit = Arc::new(Notify::new());
    let factory = Factory::new(roots, process.clone(), exit.clone());
    let connection = match zbus::connection::Builder::address(address.as_str())
        .and_then(|builder| builder.serve_at(FACTORY_PATH, factory))
        .and_then(|builder| builder.name(BUS_NAME))
    {
        Ok(builder) => builder.build().await,
        Err(error) => Err(error),
    };
    let Ok(connection) = connection else {
        diag::record(Event::Failed("cannot serve on ibus-daemon's bus"));
        return ExitCode::FAILURE;
    };
    let mut closed = pin!(connection.closed());
    let mut done = pin!(exit.notified());
    poll_fn(|cx| {
        if closed.as_mut().poll(cx).is_ready() || done.as_mut().poll(cx).is_ready() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
    if process.poisoned() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
