//! The built `ibus-engine-divvun` under a private ibus-daemon
//! (`linux.test.ibus`): its own address, `HOME` and `XDG_*` directories in
//! a temporary directory, `IBUS_COMPONENT_PATH` naming a directory with
//! only the engine's component, and no panel, emoji extension or config
//! service. The test acts as the client over D-Bus. Without ibus-daemon on
//! `PATH` (or `$IBUS_DAEMON`), each test says so and passes. Run it under
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

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use kbd_model::{Info, Keyboard};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, Message};

const PROGRAM: &str = env!("CARGO_BIN_EXE_ibus-engine-divvun");
const TEMPLATE: &str = include_str!("../data/divvun.xml.in");
const IBUS: &str = "org.freedesktop.IBus";
const DIVVUN: &str = "org.freedesktop.IBus.Divvun";
const INPUT_CONTEXT: &str = "org.freedesktop.IBus.InputContext";

/// `IBUS_CAP_PREEDIT_TEXT`, `IBUS_CAP_FOCUS` and
/// `IBUS_CAP_SURROUNDING_TEXT`, as GTK sets them.
const GTK_CAPABILITIES: u32 = 1 | 8 | 32;

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
    fn start(test: &str, keyboards: &[Installed]) -> Option<Daemon> {
        let Some(program) = ibus_daemon() else {
            eprintln!("{test}: ibus-daemon is not installed; skipping (linux.test.ibus)");
            return None;
        };
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let share = root.join("share");
        let data = share.join("divvun/keyboards/no.divvun");
        fs::create_dir_all(&data).unwrap();
        for keyboard in keyboards {
            fs::write(
                data.join(format!("{}.xml", keyboard.name)),
                keyboard.description(),
            )
            .unwrap();
            fs::write(
                data.join(format!("{}.dvkb", keyboard.name)),
                &keyboard.model,
            )
            .unwrap();
        }
        let libexec = Path::new(PROGRAM).parent().unwrap();
        let component = TEMPLATE
            .replace("@LIBEXECDIR@", libexec.to_str().unwrap())
            .replace("@DATADIR@", share.to_str().unwrap())
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

/// The test's connection to the daemon and its one input context.
struct Client {
    connection: Connection,
    context: OwnedObjectPath,
}

impl Client {
    async fn connect(daemon: &Daemon) -> Client {
        let connection = zbus::connection::Builder::address(daemon.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap();
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
            context,
        };
        client
            .set_property("ClientCommitPreedit", Value::from((true,)))
            .await;
        client
    }

    async fn call<B>(&self, method: &str, body: &B) -> zbus::Result<Message>
    where
        B: zbus::export::serde::Serialize + zbus::zvariant::DynamicType,
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

    async fn set_property(&self, name: &str, value: Value<'_>) {
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

    async fn set_engine(&self, name: &str) {
        self.call("SetCapabilities", &(GTK_CAPABILITIES,))
            .await
            .unwrap();
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
    }

    async fn engine(&self) -> String {
        let reply = self.call("GetEngine", &()).await.unwrap();
        let description: OwnedValue = reply.body().deserialize().unwrap();
        let Value::Structure(structure) = &*description else {
            panic!("an IBusEngineDesc, got {description:?}");
        };
        match structure.fields() {
            [_, _, Value::Str(name), ..] => name.as_str().to_owned(),
            fields => panic!("an IBusEngineDesc, got {fields:?}"),
        }
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
        let engines: OwnedValue = reply.body().deserialize().unwrap();
        let Value::Array(engines) = &*engines else {
            panic!("an array, got {engines:?}");
        };
        engines
            .inner()
            .iter()
            .filter_map(|engine| {
                let mut engine = engine;
                while let Value::Value(inner) = engine {
                    engine = inner;
                }
                match engine {
                    Value::Structure(s) => match s.fields() {
                        [_, _, Value::Str(name), ..] => Some(name.as_str().to_owned()),
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
        let client = Client::connect(&daemon).await;
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

// [spec:kbdgen:req:linux.ibus.inert/test]
#[test]
fn corrupt_model_refused_with_its_reason() {
    let keyboards = [
        Installed::new("divvun-se", minimal_model()),
        Installed::new("divvun-bad", b"DVKB\x01\x00\x00\x00junk".to_vec()),
    ];
    let Some(daemon) = Daemon::start("corrupt_model", &keyboards) else {
        return;
    };
    runtime().block_on(async {
        let client = Client::connect(&daemon).await;
        assert!(!client.engines().await.contains(&"divvun-bad".to_owned()));
        client.set_engine("divvun-se").await;
        assert_eq!(client.engine().await, "divvun-se");
        for (name, reason) in [
            ("divvun-bad", "model-malformed"),
            ("divvun-none", "no-description"),
        ] {
            let Err(zbus::Error::MethodError(error, detail, _)) = client.create_engine(name).await
            else {
                panic!("{name} is refused");
            };
            assert_eq!(error.as_str(), "org.freedesktop.DBus.Error.Failed");
            assert_eq!(detail.as_deref(), Some(reason));
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

// [spec:kbdgen:req:linux.ibus.factory/test]
#[test]
fn engine_exits_with_its_connection() {
    let keyboards = [Installed::new("divvun-se", minimal_model())];
    let Some(mut daemon) = Daemon::start("engine_exits", &keyboards) else {
        return;
    };
    runtime().block_on(async {
        let client = Client::connect(&daemon).await;
        client.set_engine("divvun-se").await;
        assert_eq!(client.engine().await, "divvun-se");
    });
    let running = daemon.engines();
    assert_eq!(running.len(), 1);
    daemon.stop();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !daemon.engines().is_empty() {
        assert!(Instant::now() < deadline, "the engine outlived its bus");
        std::thread::sleep(Duration::from_millis(50));
    }
}
