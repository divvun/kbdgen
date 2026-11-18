use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::address::{Environment, address, address_file};

#[derive(Default)]
struct Fake {
    vars: BTreeMap<&'static str, &'static str>,
    files: BTreeMap<PathBuf, &'static str>,
}

impl Environment for Fake {
    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).map(|value| (*value).to_owned())
    }

    fn read(&self, path: &Path) -> Option<String> {
        self.files.get(path).map(|text| (*text).to_owned())
    }

    fn home(&self) -> Option<PathBuf> {
        Some(PathBuf::from("/home/u"))
    }
}

fn fake(vars: &[(&'static str, &'static str)]) -> Fake {
    let mut fake = Fake {
        vars: vars.iter().copied().collect(),
        ..Fake::default()
    };
    fake.files
        .insert(PathBuf::from("/etc/machine-id"), "0123abcd\n");
    fake
}

const FILE: &str = "# This file is created by ibus-daemon, please do not modify it.\n\
                    IBUS_ADDRESS=unix:path=/home/u/.cache/ibus/dbus-x,guid=7f\n\
                    IBUS_DAEMON_PID=4242\n";

// [spec:kbdgen:req:linux.ibus.factory/test]
#[test]
fn address_file_named_as_libibus_names_it() {
    let cases = [
        (
            &[("WAYLAND_DISPLAY", "wayland-0"), ("DISPLAY", ":1")][..],
            "0123abcd-unix-wayland-0",
        ),
        (&[("DISPLAY", ":1")][..], "0123abcd-unix-1"),
        (&[("DISPLAY", "host:10.2")][..], "0123abcd-host-10"),
        (&[("DISPLAY", "host")][..], "0123abcd-host-0"),
        (&[][..], "0123abcd-unix-0"),
    ];
    for (vars, name) in cases {
        assert_eq!(
            address_file(&fake(vars)),
            Some(PathBuf::from("/home/u/.config/ibus/bus").join(name)),
            "{vars:?}"
        );
    }
    let xdg = fake(&[("XDG_CONFIG_HOME", "/x"), ("DISPLAY", ":0")]);
    assert_eq!(
        address_file(&xdg),
        Some(PathBuf::from("/x/ibus/bus/0123abcd-unix-0"))
    );
    let mut without_id = fake(&[("DISPLAY", ":0")]);
    without_id.files.clear();
    assert_eq!(
        address_file(&without_id),
        Some(PathBuf::from("/home/u/.config/ibus/bus/machine-id-unix-0"))
    );
    let named = fake(&[("IBUS_ADDRESS_FILE", "/run/a"), ("DISPLAY", ":0")]);
    assert_eq!(address_file(&named), Some(PathBuf::from("/run/a")));
}

// [spec:kbdgen:req:linux.ibus.factory/test]
#[test]
fn address_from_environment_then_file() {
    let mut env = fake(&[("WAYLAND_DISPLAY", "wayland-0")]);
    env.files.insert(
        PathBuf::from("/home/u/.config/ibus/bus/0123abcd-unix-wayland-0"),
        FILE,
    );
    assert_eq!(
        address(&env).as_deref(),
        Some("unix:path=/home/u/.cache/ibus/dbus-x,guid=7f")
    );
    env.vars.insert("IBUS_ADDRESS", "unix:path=/elsewhere");
    assert_eq!(address(&env).as_deref(), Some("unix:path=/elsewhere"));
}

// [spec:kbdgen:req:linux.ibus.factory/test]
#[test]
fn address_file_without_daemon_pid_gives_none() {
    let mut env = fake(&[]);
    let path = PathBuf::from("/home/u/.config/ibus/bus/0123abcd-unix-0");
    env.files
        .insert(path.clone(), "IBUS_ADDRESS=unix:path=/s\n");
    assert_eq!(address(&env), None);
    env.files
        .insert(path, "IBUS_ADDRESS=unix:path=/s\nIBUS_DAEMON_PID=-1\n");
    assert_eq!(address(&env), None);
    assert_eq!(address(&fake(&[("DISPLAY", ":3")])), None, "no file");
}
