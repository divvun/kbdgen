//! The address of ibus-daemon's bus, found as libibus's `ibus_get_address`
//! finds it (`src/ibusshare.c`): `IBUS_ADDRESS`, else the `IBUS_ADDRESS=`
//! line of the address file ibus-daemon writes. ibus-daemon is a bus of
//! its own, not the session bus.

use std::path::{Path, PathBuf};

/// What the lookup reads, so that tests can supply it.
pub trait Environment {
    fn var(&self, name: &str) -> Option<String>;
    fn read(&self, path: &Path) -> Option<String>;
    fn home(&self) -> Option<PathBuf>;
}

/// The process's own environment and file system.
pub struct System;

impl Environment for System {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn read(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn home(&self) -> Option<PathBuf> {
        std::env::home_dir()
    }
}

/// `ibus_get_local_machine_id`: D-Bus's machine id, trimmed, else
/// `machine-id`.
fn machine_id(env: &dyn Environment) -> String {
    ["/var/lib/dbus/machine-id", "/etc/machine-id"]
        .into_iter()
        .find_map(|path| env.read(Path::new(path)))
        .map_or_else(|| "machine-id".to_owned(), |id| id.trim().to_owned())
}

/// `g_get_user_config_dir`: `XDG_CONFIG_HOME`, else `~/.config`.
fn config_dir(env: &dyn Environment) -> Option<PathBuf> {
    match env.var("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => env.home().map(|home| home.join(".config")),
    }
}

/// `ibus_get_socket_path`: `IBUS_ADDRESS_FILE`, else
/// `<config>/ibus/bus/<machine id>-<host>-<display>`. Under Wayland the
/// display is `WAYLAND_DISPLAY` and the host `unix`; under X11 both come
/// from `DISPLAY`, `host:display.screen`, an empty host being `unix`.
// [spec:kbdgen:req:linux.ibus.factory]
pub fn address_file(env: &dyn Environment) -> Option<PathBuf> {
    if let Some(path) = env.var("IBUS_ADDRESS_FILE") {
        return Some(PathBuf::from(path));
    }
    let (host, display) = match env.var("WAYLAND_DISPLAY") {
        Some(display) => ("unix".to_owned(), display),
        None => match env.var("DISPLAY") {
            Some(display) => {
                let (host, rest) = display.split_once(':').unwrap_or((&display, "0"));
                let number = rest.split_once('.').map_or(rest, |(number, _)| number);
                let host = if host.is_empty() { "unix" } else { host };
                (host.to_owned(), number.to_owned())
            }
            None => ("unix".to_owned(), "0".to_owned()),
        },
    };
    let name = format!("{}-{host}-{display}", machine_id(env));
    Some(config_dir(env)?.join("ibus").join("bus").join(name))
}

/// The address ibus-daemon's bus listens on. Like libibus, a file without
/// an `IBUS_DAEMON_PID=` line gives none; unlike it, the daemon's process
/// is not probed, since a stale address fails to connect anyway.
// [spec:kbdgen:req:linux.ibus.factory]
pub fn address(env: &dyn Environment) -> Option<String> {
    if let Some(address) = env.var("IBUS_ADDRESS") {
        return Some(address);
    }
    let text = env.read(&address_file(env)?)?;
    let mut address = None;
    let mut pid = None;
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        if let Some(value) = line.strip_prefix("IBUS_ADDRESS=") {
            address = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("IBUS_DAEMON_PID=") {
            pid = value.trim().parse::<i64>().ok().filter(|pid| *pid != -1);
        }
    }
    pid.and(address)
}
