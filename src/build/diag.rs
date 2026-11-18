//! The warnings and information messages a target raises while generating
//! one layout.

/// The target a [`Diagnostics`] reports for. Its messages are logged with
/// the target's module as the `tracing` target, which the formatter prints
/// before the message (`WARN kbdgen::build::macos: sme: …`), and which
/// `RUST_LOG=kbdgen::build::macos=…` selects together with the rest of
/// that target's log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    ChromeOs,
    Linux,
}

/// Logs an event at `level` under `platform`'s module. `tracing` takes the
/// target only as a constant, so each platform has its own call site.
macro_rules! event {
    ($platform:expr, $level:expr, $($arg:tt)+) => {
        match $platform {
            Platform::Windows => {
                tracing::event!(target: "kbdgen::build::windows", $level, $($arg)+)
            }
            Platform::MacOs => tracing::event!(target: "kbdgen::build::macos", $level, $($arg)+),
            Platform::ChromeOs => {
                tracing::event!(target: "kbdgen::build::chromeos", $level, $($arg)+)
            }
            Platform::Linux => tracing::event!(target: "kbdgen::build::linux", $level, $($arg)+),
        }
    };
}

/// Collects the warnings and information messages raised while generating
/// one layout. Every message is logged through `tracing` with the layout's
/// name as it is raised, and kept so callers can inspect what was reported.
#[derive(Debug, Clone)]
pub struct Diagnostics {
    platform: Platform,
    layout: String,
    warnings: Vec<String>,
    infos: Vec<String>,
}

impl Diagnostics {
    pub fn new(platform: Platform, layout: impl Into<String>) -> Self {
        Diagnostics {
            platform,
            layout: layout.into(),
            warnings: Vec::new(),
            infos: Vec::new(),
        }
    }

    pub fn layout(&self) -> &str {
        &self.layout
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        let message = message.into();
        event!(
            self.platform,
            tracing::Level::WARN,
            "{}: {}",
            self.layout,
            message
        );
        self.warnings.push(message);
    }

    pub fn inform(&mut self, message: impl Into<String>) {
        let message = message.into();
        event!(
            self.platform,
            tracing::Level::INFO,
            "{}: {}",
            self.layout,
            message
        );
        self.infos.push(message);
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn infos(&self) -> &[String] {
        &self.infos
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// What `report` logs, as `main`'s formatter writes it without time
    /// and colour.
    fn logged(report: impl FnOnce()) -> String {
        let captured = Captured::default();
        let writer = captured.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || writer.clone())
            .with_ansi(false)
            .without_time()
            .finish();
        tracing::subscriber::with_default(subscriber, report);
        let bytes = captured.0.lock().unwrap().clone();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn messages_are_logged_under_their_platform_module() {
        let log = logged(|| {
            Diagnostics::new(Platform::Windows, "kbdsme").warn("w");
            Diagnostics::new(Platform::MacOs, "sme").warn("m");
            Diagnostics::new(Platform::ChromeOs, "sme").inform("c");
            Diagnostics::new(Platform::Linux, "sme").warn("l");
        });
        assert_eq!(
            log,
            " WARN kbdgen::build::windows: kbdsme: w\n \
             WARN kbdgen::build::macos: sme: m\n \
             INFO kbdgen::build::chromeos: sme: c\n \
             WARN kbdgen::build::linux: sme: l\n"
        );
    }

    #[test]
    fn messages_are_kept() {
        let mut diag = Diagnostics::new(Platform::MacOs, "sme");
        diag.warn("w");
        diag.inform("i");
        assert_eq!(diag.layout(), "sme");
        assert_eq!(diag.warnings(), ["w"]);
        assert_eq!(diag.infos(), ["i"]);
    }
}
