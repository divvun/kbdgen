//! Fault injection at every D-Bus entry point (`linux.test.planner`,
//! `linux.ibus.panic`): the factory and an engine object served over a
//! private peer-to-peer connection, each entry made to panic in turn.

use std::future::{Future, poll_fn};
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::sync::Arc;
use std::task::Poll;

use std::os::unix::net::UnixStream;
use tokio::sync::Notify;
use zbus::connection::Builder;
use zbus::{Connection, Guid, Message};
use zbus::{OwnedObjectPath, Variant};

use super::data::{description, install, model_bytes};
use crate::dbus::factory::Factory;
use crate::dbus::{FACTORY_PATH, text};
use crate::guard::{ENTRIES, Entry, Process, faults};

const ENGINE: &str = "org.freedesktop.IBus.Engine";
const PATH: &str = "/org/freedesktop/IBus/Engine/1";

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

async fn both<A: Future, B: Future>(a: A, b: B) -> (A::Output, B::Output) {
    let mut a = pin!(a);
    let mut b = pin!(b);
    let (mut x, mut y) = (None, None);
    poll_fn(|cx| {
        if x.is_none()
            && let Poll::Ready(v) = a.as_mut().poll(cx)
        {
            x = Some(v);
        }
        if y.is_none()
            && let Poll::Ready(v) = b.as_mut().poll(cx)
        {
            y = Some(v);
        }
        if x.is_some() && y.is_some() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
    (x.unwrap(), y.unwrap())
}

struct Served {
    _server: Connection,
    client: Connection,
    process: Arc<Process>,
    exit: Arc<Notify>,
}

/// The factory over the keyboards under `root`, and a client of it.
// [spec:kbdgen:req:linux.test.planner]
async fn serve(root: &Path) -> Served {
    let (a, b) = UnixStream::pair().unwrap();
    let process = Arc::new(Process::default());
    let exit = Arc::new(Notify::new());
    let factory = Factory::new(vec![root.to_path_buf()], process.clone(), exit.clone());
    let server = Builder::unix_stream(a)
        .server(Guid::generate())
        .p2p()
        .serve_at(FACTORY_PATH, factory)
        .build();
    let client = Builder::unix_stream(b).p2p().build();
    let (server, client) = both(server, client).await;
    Served {
        _server: server.unwrap(),
        client: client.unwrap(),
        process,
        exit,
    }
}

/// The detail of an `org.freedesktop.DBus.Error.Failed` reply, which zbus
/// reports as `fdo::Error::Failed`.
fn failure<T: std::fmt::Debug>(reply: zbus::Result<T>) -> String {
    match reply {
        Err(zbus::Error::FDO(error)) => match *error {
            zbus::fdo::Error::Failed(detail) => detail,
            other => panic!("expected Failed, got {other:?}"),
        },
        other => panic!("expected Failed, got {other:?}"),
    }
}

impl Served {
    async fn call<B>(
        &self,
        path: &str,
        interface: &str,
        member: &str,
        body: &B,
    ) -> zbus::Result<Message>
    where
        B: zbus::export::serde::Serialize + zbus::DynamicType,
    {
        self.client
            .call_method(None::<&str>, path, Some(interface), member, body)
            .await
    }

    async fn create(&self, name: &str) -> zbus::Result<OwnedObjectPath> {
        let reply = self
            .call(
                FACTORY_PATH,
                "org.freedesktop.IBus.Factory",
                "CreateEngine",
                &(name,),
            )
            .await?;
        Ok(reply.body().deserialize().unwrap())
    }

    async fn get(&self, property: &str) -> zbus::Result<bool> {
        let reply = self
            .call(
                PATH,
                "org.freedesktop.DBus.Properties",
                "Get",
                &(ENGINE, property),
            )
            .await?;
        let value: zbus::OwnedVariant = reply.body().deserialize().unwrap();
        Ok(bool::try_from(&*value).unwrap())
    }

    async fn key(&self, keycode: u32) -> zbus::Result<bool> {
        let reply = self
            .call(PATH, ENGINE, "ProcessKeyEvent", &(0u32, keycode, 0u32))
            .await?;
        Ok(reply.body().deserialize().unwrap())
    }

    /// Calls `entry`; whether it succeeded, and its reply where it has one.
    async fn enter(&self, entry: Entry) -> (bool, Option<bool>) {
        let text = text::encode("x", false).unwrap();
        let unit = |reply: zbus::Result<Message>| (reply.is_ok(), None);
        let engine = |member: &'static str| (PATH, ENGINE, member);
        match entry {
            Entry::CreateEngine => (self.create("kbd").await.is_ok(), None),
            Entry::ProcessKeyEvent => match self.key(30).await {
                Ok(handled) => (true, Some(handled)),
                Err(_) => (false, None),
            },
            Entry::GetFocusId => match self.get("FocusId").await {
                Ok(value) => (true, Some(value)),
                Err(_) => (false, None),
            },
            Entry::GetActiveSurroundingText => match self.get("ActiveSurroundingText").await {
                Ok(value) => (true, Some(value)),
                Err(_) => (false, None),
            },
            Entry::SetContentType => unit(
                self.call(
                    PATH,
                    "org.freedesktop.DBus.Properties",
                    "Set",
                    &(ENGINE, "ContentType", Variant::from((8u32, 0u32))),
                )
                .await,
            ),
            Entry::Destroy => unit(
                self.call(PATH, "org.freedesktop.IBus.Service", "Destroy", &())
                    .await,
            ),
            other => {
                let (path, interface, member) = engine(other.name());
                let reply = match other {
                    Entry::SetCursorLocation => {
                        self.call(path, interface, member, &(1i32, 2i32, 3i32, 4i32))
                            .await
                    }
                    Entry::ProcessHandWritingEvent => {
                        self.call(path, interface, member, &(vec![0f64, 0., 1., 1.],))
                            .await
                    }
                    Entry::CancelHandWriting | Entry::SetCapabilities => {
                        self.call(path, interface, member, &(1u32,)).await
                    }
                    Entry::PropertyActivate => {
                        self.call(path, interface, member, &("p", 1u32)).await
                    }
                    Entry::PropertyShow | Entry::PropertyHide | Entry::FocusOutId => {
                        self.call(path, interface, member, &("p",)).await
                    }
                    Entry::CandidateClicked => {
                        self.call(path, interface, member, &(0u32, 1u32, 0u32))
                            .await
                    }
                    Entry::FocusInId => self.call(path, interface, member, &("p", "c")).await,
                    Entry::SetSurroundingText => {
                        self.call(path, interface, member, &(&text, 1u32, 1u32))
                            .await
                    }
                    Entry::PanelExtensionReceived | Entry::PanelExtensionRegisterKeys => {
                        self.call(path, interface, member, &(&text,)).await
                    }
                    _ => self.call(path, interface, member, &()).await,
                };
                unit(reply)
            }
        }
    }
}

fn keyboards() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("keyboards");
    install(
        &root,
        "p",
        "kbd",
        &description("kbd", "K"),
        Some(&model_bytes()),
    );
    (dir, root)
}

// [spec:kbdgen:req:linux.test.planner/test]
// [spec:kbdgen:req:linux.ibus.panic/test]
#[test]
fn every_dbus_entry_contains_its_panic() {
    let (_dir, root) = keyboards();
    let runtime = runtime();
    for entry in ENTRIES {
        runtime.block_on(async {
            let served = serve(&root).await;
            if entry != Entry::CreateEngine {
                assert_eq!(served.create("kbd").await.unwrap().as_str(), PATH);
            }
            faults::arm(entry);
            let (ok, reply) = served.enter(entry).await;
            assert!(served.process.poisoned(), "{entry:?} poisons");
            match entry {
                Entry::CreateEngine => assert!(!ok, "a poisoned process creates no engine"),
                Entry::ProcessKeyEvent => assert_eq!(reply, Some(false), "the key passes"),
                Entry::GetFocusId => assert_eq!(reply, Some(false)),
                Entry::GetActiveSurroundingText => assert_eq!(reply, Some(true)),
                _ => assert!(ok, "{entry:?} succeeds"),
            }
        });
    }
}

// [spec:kbdgen:req:linux.test.planner/test]
// [spec:kbdgen:req:linux.ibus.panic/test]
#[test]
fn poisoned_process_passes_and_exits_without_engines() {
    let (_dir, root) = keyboards();
    runtime().block_on(async {
        let served = serve(&root).await;
        served.create("kbd").await.unwrap();
        assert!(!served.key(30).await.unwrap(), "an empty keyboard passes");
        faults::arm(Entry::FocusIn);
        served.enter(Entry::FocusIn).await;
        assert!(served.process.poisoned());
        assert!(!served.key(30).await.unwrap());
        assert_eq!(failure(served.create("kbd").await), "poisoned");
        assert!(!served.process.done(), "an engine object is left");
        served.enter(Entry::Destroy).await;
        assert!(served.process.done());
        served.exit.notified().await;
    });
}

// [spec:kbdgen:req:linux.ibus.factory/test]
// [spec:kbdgen:req:linux.ibus.inert/test]
#[test]
fn factory_numbers_engines_and_refuses_unknown() {
    let (_dir, root) = keyboards();
    runtime().block_on(async {
        let served = serve(&root).await;
        assert_eq!(served.create("kbd").await.unwrap().as_str(), PATH);
        assert_eq!(
            served.create("kbd").await.unwrap().as_str(),
            "/org/freedesktop/IBus/Engine/2"
        );
        assert_eq!(failure(served.create("none").await), "no-description");
        served.enter(Entry::Destroy).await;
        assert!(served.get("FocusId").await.is_err(), "unexported");
        assert!(!served.process.poisoned());
    });
}
