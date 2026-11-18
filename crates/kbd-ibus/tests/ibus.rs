//! The built `ibus-engine-divvun` under a private ibus-daemon
//! (`linux.test.ibus`): its own address, `HOME` and `XDG_*` directories in
//! a temporary directory, `IBUS_COMPONENT_PATH` naming a directory with
//! only the engine's component, and no panel, emoji extension or config
//! service. The test acts as the client over D-Bus and applies the input
//! context's signals to a fake field. Without ibus-daemon on `PATH` (or
//! `$IBUS_DAEMON`), each test says so and passes, unless
//! `KBDGEN_REQUIRE_LINUX_TESTS` is set, when it fails. Run it under
//! `dbus-run-session`, as ibus-daemon also looks for a session bus.

#![cfg(unix)]
// Clippy counts only `#[test]` functions of a test binary as tests, not
// the helpers beside them.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::fs;
use std::future::poll_fn;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Child, Command, Stdio};
use std::task::Poll;
use std::time::{Duration, Instant};

use common::cases::{BACKSPACE, Case, Step, Stroke, Typed, cases, pairs};
use common::field::Field;
use common::fixture;
use common::native::Native;
use kbd_engine::{Key, KeyEvent, Model};
use kbd_ibus::planner::{CAP_PREEDIT_TEXT, CAP_SURROUNDING_TEXT, PreeditMode, Signal};
use kbd_model::{Info, Keyboard};
use zbus::export::futures_core::Stream;
use zbus::message::Type;
use zbus::{Connection, Message, MessageStream};
use zbus::{OwnedObjectPath, OwnedVariant, Variant};

const PROGRAM: &str = env!("CARGO_BIN_EXE_ibus-engine-divvun");
const TEMPLATE: &str = include_str!("../data/divvun.xml.in");
const IBUS: &str = "org.freedesktop.IBus";
const DIVVUN: &str = "org.freedesktop.IBus.Divvun";
const INPUT_CONTEXT: &str = "org.freedesktop.IBus.InputContext";
/// `IBUS_CAP_FOCUS`.
const CAP_FOCUS: u32 = 1 << 3;
/// `IBUS_INPUT_PURPOSE_PASSWORD`.
const PASSWORD: u32 = 8;
const RELEASE: u32 = 1 << 30;

fn ibus_daemon() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("IBUS_DAEMON") {
        return Some(PathBuf::from(path));
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join("ibus-daemon"))
        .find(|path| path.is_file())
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// A keyboard as a package installs it: its engine description and its
/// model.
struct Installed {
    name: String,
    model: Vec<u8>,
}

impl Installed {
    fn new(name: &str, model: Vec<u8>) -> Installed {
        Installed {
            name: name.to_owned(),
            model,
        }
    }

    fn description(&self) -> String {
        let name = &self.name;
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<engine>\n  <name>{name}</name>\n  \
             <longname>Test</longname>\n  <description>Test</description>\n  \
             <language>se</language>\n  <author>Test</author>\n  <layout>{name}</layout>\n  \
             <symbol>se</symbol>\n  <version>1.0</version>\n</engine>\n"
        )
    }
}

/// A private ibus-daemon, killed on drop.
struct Daemon {
    dir: tempfile::TempDir,
    child: Child,
    address: String,
}

impl Daemon {
    /// Starts ibus-daemon with the component and `keyboards` installed
    /// system-wide, or says why not and gives `None`.
    // [spec:kbdgen:req:linux.test.ibus]
    // [spec:kbdgen:sem:linux.test.ci/test]
    fn start(test: &str, keyboards: &[Installed]) -> Option<Daemon> {
        let Some(program) = ibus_daemon() else {
            assert!(
                std::env::var_os("KBDGEN_REQUIRE_LINUX_TESTS").is_none(),
                "{test}: ibus-daemon is not installed, and KBDGEN_REQUIRE_LINUX_TESTS is set"
            );
            eprintln!("{test}: ibus-daemon is not installed; skipping (linux.test.ibus)");
            return None;
        };
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let data = root.join("share/divvun/keyboards/no.divvun");
        fs::create_dir_all(&data).unwrap();
        for keyboard in keyboards {
            let file = |extension: &str| data.join(format!("{}.{extension}", keyboard.name));
            fs::write(file("xml"), keyboard.description()).unwrap();
            fs::write(file("dvkb"), &keyboard.model).unwrap();
        }
        let libexec = Path::new(PROGRAM).parent().unwrap();
        let component = TEMPLATE
            .replace("@LIBEXECDIR@", libexec.to_str().unwrap())
            .replace("@DATADIR@", root.join("share").to_str().unwrap())
            .replace("@VERSION@", env!("CARGO_PKG_VERSION"));
        fs::create_dir_all(root.join("component")).unwrap();
        fs::write(root.join("component/divvun.xml"), component).unwrap();
        for sub in ["home", "config", "cache", "data", "run"] {
            fs::create_dir_all(root.join(sub)).unwrap();
        }
        let socket = root.join("bus");
        let address = format!("unix:path={}", socket.display());
        let child = Command::new(program)
            .args([
                "--single",
                "--panel=disable",
                "--emoji-extension=disable",
                "--config=disable",
                "--cache=none",
                "--verbose",
            ])
            .arg(format!("--address={address}"))
            .env("HOME", root.join("home"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_RUNTIME_DIR", root.join("run"))
            .env("IBUS_COMPONENT_PATH", root.join("component"))
            .env_remove("IBUS_ADDRESS")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .stdout(Stdio::null())
            .stderr(fs::File::create(root.join("daemon.log")).unwrap())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !socket.exists() {
            assert!(Instant::now() < deadline, "ibus-daemon did not start");
            std::thread::sleep(Duration::from_millis(50));
        }
        Some(Daemon {
            dir,
            child,
            address,
        })
    }

    fn log(&self) -> String {
        fs::read_to_string(self.dir.path().join("daemon.log")).unwrap_or_default()
    }

    /// What `--xml` lists for this daemon's data, as ibus-daemon runs it.
    fn listed(&self) -> Vec<String> {
        let output = Command::new(PROGRAM)
            .args(["--xml", "--data-dir"])
            .arg(self.dir.path().join("share/divvun/keyboards"))
            .env("HOME", self.dir.path().join("home"))
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .filter_map(|line| line.trim().strip_prefix("<name>")?.strip_suffix("</name>"))
            .map(str::to_owned)
            .collect()
    }

    /// The processes running the engine under this daemon.
    fn engines(&self) -> Vec<u32> {
        let marker = self.dir.path().join("share");
        let marker = marker.to_str().unwrap();
        fs::read_dir("/proc")
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let pid: u32 = entry.file_name().to_str()?.parse().ok()?;
                let cmdline = fs::read(entry.path().join("cmdline")).ok()?;
                let cmdline = String::from_utf8_lossy(&cmdline);
                (cmdline.contains("--ibus") && cmdline.contains(marker)).then_some(pid)
            })
            .collect()
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The text of an `IBusText`.
fn ibus_text(value: &Variant<'_>) -> String {
    let mut value = value;
    while let Variant::Variant(inner) = value {
        value = inner;
    }
    let Variant::Structure(structure) = value else {
        panic!("an IBusText, got {value:?}");
    };
    let [Variant::Str(kind), _, Variant::Str(text), ..] = structure.fields() else {
        panic!("an IBusText, got {structure:?}");
    };
    assert_eq!(kind.as_str(), "IBusText");
    text.as_str().to_owned()
}

/// An `IBusText` of `text` with no attributes.
fn ibus_text_value(text: &str) -> Variant<'static> {
    use std::collections::HashMap;
    let list = zbus::Structure::builder()
        .add_field("IBusAttrList")
        .add_field(HashMap::<String, Variant<'static>>::new())
        .add_field(Vec::<Variant<'static>>::new())
        .build()
        .unwrap();
    Variant::Structure(
        zbus::Structure::builder()
            .add_field("IBusText")
            .add_field(HashMap::<String, Variant<'static>>::new())
            .add_field(text.to_owned())
            .add_field(Variant::Structure(list))
            .build()
            .unwrap(),
    )
}

/// The test's connection to the daemon, its one input context, and the
/// field it applies the context's signals to.
struct Client {
    connection: Connection,
    stream: MessageStream,
    context: OwnedObjectPath,
    field: Field,
    surrounding: bool,
}

impl Client {
    async fn connect(daemon: &Daemon) -> Client {
        let connection = zbus::connection::Builder::address(daemon.address.as_str())
            .build()
            .await
            .unwrap();
        let stream = MessageStream::from(&connection);
        let reply = connection
            .call_method(
                Some(IBUS),
                "/org/freedesktop/IBus",
                Some(IBUS),
                "CreateInputContext",
                &("kbd-ibus test",),
            )
            .await
            .unwrap();
        let context: OwnedObjectPath = reply.body().deserialize().unwrap();
        let client = Client {
            connection,
            stream,
            context,
            field: Field::default(),
            surrounding: true,
        };
        client
            .set_property("ClientCommitPreedit", Variant::from((true,)))
            .await;
        client
    }

    async fn call<B>(&self, method: &str, body: &B) -> zbus::Result<Message>
    where
        B: zbus::export::serde::Serialize + zbus::DynamicType,
    {
        self.connection
            .call_method(
                Some(IBUS),
                self.context.as_str(),
                Some(INPUT_CONTEXT),
                method,
                body,
            )
            .await
    }

    async fn set_property(&self, name: &str, value: Variant<'_>) {
        self.connection
            .call_method(
                Some(IBUS),
                self.context.as_str(),
                Some("org.freedesktop.DBus.Properties"),
                "Set",
                &(INPUT_CONTEXT, name, value),
            )
            .await
            .unwrap();
    }

    /// Focuses the field, as GTK sets it up, and makes `name` the engine.
    async fn set_engine(&mut self, name: &str) {
        self.set_capabilities(true, true).await;
        self.call("FocusIn", &()).await.unwrap();
        self.connection
            .call_method(
                Some(IBUS),
                "/org/freedesktop/IBus",
                Some(IBUS),
                "SetGlobalEngine",
                &(name,),
            )
            .await
            .unwrap();
        self.received().await;
    }

    async fn set_capabilities(&mut self, preedit: bool, surrounding: bool) {
        let mut capabilities = CAP_FOCUS;
        if preedit {
            capabilities |= CAP_PREEDIT_TEXT;
        }
        if surrounding {
            capabilities |= CAP_SURROUNDING_TEXT;
        }
        self.surrounding = surrounding;
        self.call("SetCapabilities", &(capabilities,))
            .await
            .unwrap();
    }

    async fn set_purpose(&self, purpose: u32) {
        self.set_property("ContentType", Variant::from((purpose, 0u32)))
            .await;
    }

    async fn engine(&self) -> String {
        let reply = self.call("GetEngine", &()).await.unwrap();
        let description: OwnedVariant = reply.body().deserialize().unwrap();
        let Variant::Structure(structure) = &*description else {
            panic!("an IBusEngineDesc, got {description:?}");
        };
        match structure.fields() {
            [_, _, Variant::Str(name), ..] => name.as_str().to_owned(),
            fields => panic!("an IBusEngineDesc, got {fields:?}"),
        }
    }

    /// The input context's signals received so far, but for
    /// `RequireSurroundingText`, which this client answers before every
    /// key anyway. ibus-daemon sends a call's signals before its reply, so
    /// after a reply they are here.
    async fn received(&mut self) -> Vec<Signal> {
        let mut received = Vec::new();
        while let Some(message) = poll_fn(|cx| match Pin::new(&mut self.stream).poll_next(cx) {
            Poll::Ready(message) => Poll::Ready(message),
            Poll::Pending => Poll::Ready(None),
        })
        .await
        {
            let message = message.unwrap();
            let header = message.header();
            if message.message_type() != Type::Signal
                || header.interface().map(|i| i.as_str()) != Some(INPUT_CONTEXT)
            {
                continue;
            }
            let body = message.body();
            received.push(match header.member().map(|m| m.as_str()) {
                Some("CommitText") => {
                    let (text,): (OwnedVariant,) = body.deserialize().unwrap();
                    Signal::CommitText(ibus_text(&text))
                }
                Some("UpdatePreeditTextWithMode") => {
                    let (text, cursor, visible, mode): (OwnedVariant, u32, bool, u32) =
                        body.deserialize().unwrap();
                    Signal::UpdatePreeditText {
                        text: ibus_text(&text),
                        cursor,
                        visible,
                        mode: if mode == 1 {
                            PreeditMode::Commit
                        } else {
                            PreeditMode::Clear
                        },
                    }
                }
                Some("DeleteSurroundingText") => {
                    let (offset, nchars): (i32, u32) = body.deserialize().unwrap();
                    Signal::DeleteSurroundingText { offset, nchars }
                }
                _ => continue,
            });
        }
        received
    }

    /// Starts a case: the engine resets, the field empties, and for a
    /// model with Shift Lock, a Shift tap undoes the lock an earlier case's
    /// Caps Lock left.
    async fn restart(&mut self, shift_lock: bool) {
        self.call("Reset", &()).await.unwrap();
        if shift_lock {
            self.event(0xffe1, 42, 0).await;
            self.event(0xffe1, 42, RELEASE).await;
        }
        self.received().await;
        self.field = Field::default();
    }

    /// A key event; the signals of a press go to the field, which types a
    /// key the engine passed.
    async fn event(&mut self, keyval: u32, keycode: u32, state: u32) -> bool {
        let reply = self
            .call("ProcessKeyEvent", &(keyval, keycode, state))
            .await
            .unwrap();
        let handled: bool = reply.body().deserialize().unwrap();
        let signals = self.received().await;
        self.field.apply(&signals);
        if state & RELEASE == 0 && !handled {
            self.field.type_passed(Typed::of(keyval, keycode, state));
        }
        handled
    }

    /// A stroke, the field read before its key as GTK reads it.
    async fn stroke(&mut self, stroke: &Stroke) -> bool {
        for &(keyval, code) in &stroke.taps {
            self.event(keyval, code, 0).await;
            self.event(keyval, code, RELEASE).await;
        }
        for &(keyval, code) in &stroke.held {
            self.event(keyval, code, 0).await;
        }
        if self.surrounding {
            let text = self.field.before.clone();
            let n = u32::try_from(text.chars().count()).unwrap();
            self.call("SetSurroundingText", &(ibus_text_value(&text), n, n))
                .await
                .unwrap();
        }
        let handled = self.event(stroke.keyval, stroke.code, stroke.state).await;
        let released = self
            .event(stroke.keyval, stroke.code, stroke.state | RELEASE)
            .await;
        assert_eq!(released, handled, "a release returns what its press did");
        for &(keyval, code) in stroke.held.iter().rev() {
            self.event(keyval, code, stroke.state | RELEASE).await;
        }
        handled
    }

    /// Calls the component's factory directly.
    async fn create_engine(&self, name: &str) -> zbus::Result<OwnedObjectPath> {
        let reply = self
            .connection
            .call_method(
                Some(DIVVUN),
                "/org/freedesktop/IBus/Factory",
                Some("org.freedesktop.IBus.Factory"),
                "CreateEngine",
                &(name,),
            )
            .await?;
        Ok(reply.body().deserialize().unwrap())
    }

    /// The names of the engines ibus-daemon lists.
    async fn engines(&self) -> Vec<String> {
        let reply = self
            .connection
            .call_method(
                Some(IBUS),
                "/org/freedesktop/IBus",
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &(IBUS, "Engines"),
            )
            .await
            .unwrap();
        let engines: OwnedVariant = reply.body().deserialize().unwrap();
        let Variant::Array(engines) = &*engines else {
            panic!("an array, got {engines:?}");
        };
        engines
            .inner()
            .iter()
            .filter_map(|engine| {
                let mut engine = engine;
                while let Variant::Variant(inner) = engine {
                    engine = inner;
                }
                match engine {
                    Variant::Structure(s) => match s.fields() {
                        [_, _, Variant::Str(name), ..] => Some(name.as_str().to_owned()),
                        _ => None,
                    },
                    _ => None,
                }
            })
            .collect()
    }
}

fn minimal_model() -> Vec<u8> {
    Keyboard::new("se", 45, Info::default()).to_bytes().unwrap()
}

/// Runs `case` through the daemon and natively, and says where they
/// differ, as `linux.test.planner` does. Without surrounding text, an
/// on-screen Backspace is a key press, as GNOME Shell sends it to a
/// terminal.
// [spec:kbdgen:req:linux.test.ibus]
async fn run(client: &mut Client, model: &Model, case: &Case, setup: Setup) -> Result<(), String> {
    client.restart(model.keyboard().windows.shift_lock).await;
    let mut native = if setup.password {
        Native::blind(model)
    } else {
        Native::new(model)
    };
    for step in &case.steps {
        let stroke = match step {
            Step::Key(stroke) => stroke.clone(),
            Step::OnScreenBackspace => Stroke {
                name: "Backspace".to_owned(),
                taps: Vec::new(),
                held: Vec::new(),
                keyval: BACKSPACE.0,
                code: BACKSPACE.1,
                state: 0,
                event: Some(KeyEvent::new(Key::Backspace)),
            },
        };
        let handled = client.stroke(&stroke).await;
        let expected = native.key(stroke.event.as_ref(), stroke.typed());
        if handled != expected {
            return Err(format!(
                "{}: {} handled {handled}, natively {expected}",
                case.name, stroke.name
            ));
        }
    }
    let (got, expected) = if setup.preedit && !setup.password {
        (
            client.field.text(),
            format!("{}{}", native.document, native.preedit),
        )
    } else {
        (client.field.before.clone(), native.document.clone())
    };
    if got != expected {
        return Err(format!(
            "{}: the field shows {got:?}, natively {expected:?}",
            case.name
        ));
    }
    Ok(())
}

// [spec:kbdgen:req:linux.test.ibus/test]
// [spec:kbdgen:req:linux.ibus.factory/test]
// [spec:kbdgen:def:linux.ibus.component/test]
// [spec:kbdgen:req:linux.ibus.lister/test]
#[test]
fn engine_registers_and_factory_creates_it() {
    let keyboards = [Installed::new("divvun-se", minimal_model())];
    let Some(daemon) = Daemon::start("engine_registers", &keyboards) else {
        return;
    };
    runtime().block_on(async {
        let mut client = Client::connect(&daemon).await;
        let listed = client.engines().await;
        assert!(
            listed.contains(&"divvun-se".to_owned()),
            "{listed:?}\n{}",
            daemon.log()
        );
        client.set_engine("divvun-se").await;
        assert_eq!(client.engine().await, "divvun-se", "{}", daemon.log());
        assert_eq!(daemon.engines().len(), 1, "one engine process");

        let introspection: String = client
            .connection
            .call_method(
                Some(DIVVUN),
                "/org/freedesktop/IBus/Engine/1",
                Some("org.freedesktop.DBus.Introspectable"),
                "Introspect",
                &(),
            )
            .await
            .unwrap()
            .body()
            .deserialize()
            .unwrap();
        for member in [
            "<method name=\"ProcessKeyEvent\">",
            "<method name=\"SetSurroundingText\">",
            "<method name=\"FocusIn\">",
            "<arg name=\"x\" type=\"i\" direction=\"in\"/>",
            "<method name=\"PanelExtensionReceived\">",
            "<signal name=\"CommitText\">",
            "<signal name=\"UpdatePreeditText\">",
            "<signal name=\"DeleteSurroundingText\">",
            "<signal name=\"RequireSurroundingText\">",
            "<property name=\"ContentType\" type=\"(uu)\" access=\"write\"",
            "<property name=\"FocusId\" type=\"b\" access=\"read\"/>",
            "<property name=\"ActiveSurroundingText\" type=\"b\" access=\"read\"/>",
            "<interface name=\"org.freedesktop.IBus.Service\">",
        ] {
            assert!(introspection.contains(member), "{member}\n{introspection}");
        }
    });
}

// [spec:kbdgen:req:linux.test.ibus/test]
// [spec:kbdgen:req:linux.ibus.inert/test]
// [spec:kbdgen:req:linux.ibus.lister/test]
#[test]
fn engine_list_matches_lister_and_refuses_corrupt_model() {
    let keyboards = [
        Installed::new("divvun-se", minimal_model()),
        Installed::new("divvun-sma", minimal_model()),
        Installed::new("divvun-bad", b"DVKB\x01\x00\x00\x00junk".to_vec()),
    ];
    let Some(daemon) = Daemon::start("engine_list", &keyboards) else {
        return;
    };
    runtime().block_on(async {
        let mut client = Client::connect(&daemon).await;
        let listed = daemon.listed();
        assert_eq!(listed, ["divvun-se", "divvun-sma"]);
        let mut engines: Vec<String> = client
            .engines()
            .await
            .into_iter()
            .filter(|name| name.starts_with("divvun-"))
            .collect();
        engines.sort();
        assert_eq!(engines, listed, "ibus-daemon lists what --xml prints");
        client.set_engine("divvun-se").await;
        assert_eq!(client.engine().await, "divvun-se");
        for (name, reason) in [
            ("divvun-bad", "model-malformed"),
            ("divvun-none", "no-description"),
        ] {
            let refused = client.create_engine(name).await;
            let Err(zbus::Error::FDO(error)) = refused else {
                panic!("{name} is refused with Failed, got {refused:?}");
            };
            let zbus::fdo::Error::Failed(detail) = *error else {
                panic!("{name} is refused with Failed, got {error:?}");
            };
            assert_eq!(detail, reason);
        }
        assert!(
            daemon
                .log()
                .contains("ibus-engine-divvun: divvun-bad.dvkb: model-malformed"),
            "{}",
            daemon.log()
        );
        let path = client.create_engine("divvun-se").await.unwrap();
        assert_eq!(path.as_str(), "/org/freedesktop/IBus/Engine/2");
    });
}

// [spec:kbdgen:req:linux.test.ibus/test]
// [spec:kbdgen:req:linux.ibus.factory/test]
#[test]
fn engine_exits_with_its_connection() {
    let keyboards = [Installed::new("divvun-se", minimal_model())];
    let Some(mut daemon) = Daemon::start("engine_exits", &keyboards) else {
        return;
    };
    runtime().block_on(async {
        let mut client = Client::connect(&daemon).await;
        client.set_engine("divvun-se").await;
        assert_eq!(client.engine().await, "divvun-se");
    });
    assert_eq!(daemon.engines().len(), 1);
    daemon.stop();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !daemon.engines().is_empty() {
        assert!(Instant::now() < deadline, "the engine outlived its bus");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// How a field is set up through the daemon.
#[derive(Debug, Clone, Copy)]
struct Setup {
    preedit: bool,
    surrounding: bool,
    password: bool,
}

const GTK: Setup = Setup {
    preedit: true,
    surrounding: true,
    password: false,
};
const TERMINAL: Setup = Setup {
    surrounding: false,
    ..GTK
};
const PASSWORD_FIELD: Setup = Setup {
    password: true,
    ..GTK
};

// [spec:kbdgen:req:linux.test.ibus/test]
// [spec:kbdgen:req:linux.ibus.keys/test]
// [spec:kbdgen:sem:linux.ibus.virtual-keys/test]
// [spec:kbdgen:req:linux.ibus.edit/test]
// [spec:kbdgen:sem:linux.ibus.context/test]
// [spec:kbdgen:sem:linux.ibus.context.fallback/test]
// [spec:kbdgen:req:linux.ibus.security/test]
#[test]
fn fixtures_type_through_the_daemon() {
    let fixtures: Vec<(String, Model)> = fixture::all()
        .into_iter()
        .map(|(name, tag, text)| (format!("divvun-{name}"), fixture::model(tag, &text)))
        .collect();
    let keyboards: Vec<Installed> = fixtures
        .iter()
        .map(|(name, model)| Installed::new(name, model.keyboard().to_bytes().unwrap()))
        .collect();
    let Some(daemon) = Daemon::start("fixtures_type", &keyboards) else {
        return;
    };
    runtime().block_on(async {
        let mut client = Client::connect(&daemon).await;
        let mut failures = Vec::new();
        for (name, model) in &fixtures {
            client.set_engine(name).await;
            assert_eq!(&client.engine().await, name, "{}", daemon.log());
            let mut runs = vec![(GTK, cases(model)), (TERMINAL, cases(model))];
            if model.keyboard().rewrites().is_empty() {
                runs.push((PASSWORD_FIELD, cases(model)));
            } else {
                runs.push((TERMINAL, pairs(model)));
            }
            for (setup, cases) in runs {
                client
                    .set_capabilities(setup.preedit, setup.surrounding)
                    .await;
                client
                    .set_purpose(if setup.password { PASSWORD } else { 0 })
                    .await;
                for case in &cases {
                    let onscreen_deletion = case
                        .steps
                        .iter()
                        .any(|step| matches!(step, Step::OnScreenBackspace));
                    if onscreen_deletion && setup.surrounding {
                        continue;
                    }
                    if let Err(failure) = run(&mut client, model, case, setup).await {
                        failures.push(format!("{name} {setup:?}: {failure}"));
                    }
                }
            }
        }
        let count = failures.len();
        failures.truncate(10);
        assert!(
            failures.is_empty(),
            "{count} failure(s):\n{}\n{}",
            failures.join("\n"),
            daemon.log()
        );
    });
}
