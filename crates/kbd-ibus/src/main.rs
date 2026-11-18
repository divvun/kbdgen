//! `ibus-engine-divvun`, the Divvun keyboard IBus engine
//! (`docs/spec/linux.md`, "IBus engine"): one program, shared by every
//! keyboard kbdgen builds, that runs `kbd-engine` on the model of each
//! installed keyboard. Keyboards supply only data (`linux.ibus.data`);
//! the program is released and installed separately, with the component
//! file `data/divvun.xml.in` that packaging completes.
//!
//! `--xml` prints the engines of the installed keyboards for ibus-daemon,
//! which runs it again whenever an observed data directory changes.

// [spec:kbdgen:def:linux.ibus.component]
// [spec:kbdgen:req:linux.ibus.crate]

mod data;
#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: ibus-engine-divvun --xml [--data-dir DIR]...\n";

/// What the program was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    /// Print the `<engines>` list (`linux.ibus.lister`).
    Xml,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    mode: Mode,
    data_dirs: Vec<PathBuf>,
}

fn parse(args: impl IntoIterator<Item = OsString>) -> Option<Args> {
    let mut mode = None;
    let mut data_dirs = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--xml") if mode.is_none() => mode = Some(Mode::Xml),
            Some("--data-dir") => data_dirs.push(PathBuf::from(args.next()?)),
            _ => return None,
        }
    }
    Some(Args {
        mode: mode?,
        data_dirs,
    })
}

fn main() -> ExitCode {
    let Some(args) = parse(std::env::args_os().skip(1)) else {
        let _ = std::io::stderr().write_all(USAGE.as_bytes());
        return ExitCode::from(2);
    };
    match args.mode {
        Mode::Xml => {
            let roots = data::roots(&args.data_dirs);
            let xml = data::engines_xml(&roots);
            let mut stdout = std::io::stdout();
            if stdout
                .write_all(xml.as_bytes())
                .and(stdout.flush())
                .is_err()
            {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
    }
}
