//! An engine object: `org.freedesktop.IBus.Engine` as `src/ibusengine.c`
//! declares it, over a [`Session`], and `org.freedesktop.IBus.Service`,
//! whose `Destroy` unexports it.
//!
//! Every method and property access runs its body in [`Process::contain`]
//! and only then emits the signals the session asked for, so a panic
//! emits nothing. Signals go out on the connection before the reply, in
//! order, so ibus-daemon handles them before it acts on the reply.

use std::sync::Arc;

use tokio::sync::Notify;
use zbus::Variant;
use zbus::object_server::SignalEmitter;
use zbus::{ObjectServer, interface};

use super::text;
use crate::guard::{Entry, Process};
use crate::planner::{Reply, Session, Signal};

/// The engine interface of one engine object.
pub struct Engine {
    session: Session,
    process: Arc<Process>,
}

impl Engine {
    pub fn new(session: Session, process: Arc<Process>) -> Engine {
        Engine { session, process }
    }

    fn run<T>(&mut self, entry: Entry, fallback: T, body: impl FnOnce(&mut Session) -> T) -> T {
        let session = &mut self.session;
        self.process.contain(entry, fallback, || body(session))
    }
}

/// Emits `signals` in order. One that cannot be sent is dropped: the
/// connection is closing, and the program exits with it.
async fn emit(emitter: &SignalEmitter<'_>, signals: Vec<Signal>) {
    for signal in signals {
        let _ = match signal {
            Signal::CommitText(s) => match text::encode(&s, false) {
                Ok(value) => Engine::commit_text(emitter, &value).await,
                Err(_) => continue,
            },
            Signal::UpdatePreeditText {
                text: s,
                cursor,
                visible,
                mode,
            } => match text::encode(&s, true) {
                Ok(value) => {
                    Engine::update_preedit_text(emitter, &value, cursor, visible, mode as u32).await
                }
                Err(_) => continue,
            },
            Signal::DeleteSurroundingText { offset, nchars } => {
                Engine::delete_surrounding_text(emitter, offset, nchars).await
            }
            Signal::RequireSurroundingText => Engine::require_surrounding_text(emitter).await,
        };
    }
}

// [spec:kbdgen:req:linux.ibus.factory]
// [spec:kbdgen:req:linux.ibus.panic]
#[interface(
    name = "org.freedesktop.IBus.Engine",
    spawn = false,
    introspection_docs = false
)]
impl Engine {
    async fn process_key_event(
        &mut self,
        keyval: u32,
        keycode: u32,
        state: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> bool {
        let reply = self.run(Entry::ProcessKeyEvent, Reply::default(), |session| {
            session.process_key_event(keyval, keycode, state)
        });
        emit(&emitter, reply.signals).await;
        reply.handled
    }

    async fn set_cursor_location(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let _ = (x, y, w, h);
        self.run(Entry::SetCursorLocation, (), |_| ());
    }

    async fn process_hand_writing_event(&mut self, coordinates: Vec<f64>) {
        drop(coordinates);
        self.run(Entry::ProcessHandWritingEvent, (), |_| ());
    }

    async fn cancel_hand_writing(&mut self, n_strokes: u32) {
        let _ = n_strokes;
        self.run(Entry::CancelHandWriting, (), |_| ());
    }

    async fn set_capabilities(&mut self, caps: u32) {
        self.run(Entry::SetCapabilities, (), |session| {
            session.set_capabilities(caps)
        });
    }

    async fn property_activate(&mut self, name: String, state: u32) {
        let _ = (name, state);
        self.run(Entry::PropertyActivate, (), |_| ());
    }

    async fn property_show(&mut self, name: String) {
        drop(name);
        self.run(Entry::PropertyShow, (), |_| ());
    }

    async fn property_hide(&mut self, name: String) {
        drop(name);
        self.run(Entry::PropertyHide, (), |_| ());
    }

    async fn candidate_clicked(&mut self, index: u32, button: u32, state: u32) {
        let _ = (index, button, state);
        self.run(Entry::CandidateClicked, (), |_| ());
    }

    async fn focus_in(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let signals = self.run(Entry::FocusIn, Vec::new(), Session::focus_in);
        emit(&emitter, signals).await;
    }

    async fn focus_in_id(
        &mut self,
        object_path: String,
        client: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        let _ = (object_path, client);
        let signals = self.run(Entry::FocusInId, Vec::new(), Session::focus_in);
        emit(&emitter, signals).await;
    }

    async fn focus_out(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let signals = self.run(Entry::FocusOut, Vec::new(), Session::focus_out);
        emit(&emitter, signals).await;
    }

    async fn focus_out_id(
        &mut self,
        object_path: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        drop(object_path);
        let signals = self.run(Entry::FocusOutId, Vec::new(), Session::focus_out);
        emit(&emitter, signals).await;
    }

    async fn reset(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let signals = self.run(Entry::Reset, Vec::new(), Session::reset);
        emit(&emitter, signals).await;
    }

    async fn enable(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let signals = self.run(Entry::Enable, Vec::new(), Session::enable);
        emit(&emitter, signals).await;
    }

    async fn disable(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let signals = self.run(Entry::Disable, Vec::new(), Session::disable);
        emit(&emitter, signals).await;
    }

    async fn page_up(&mut self) {
        self.run(Entry::PageUp, (), |_| ());
    }

    async fn page_down(&mut self) {
        self.run(Entry::PageDown, (), |_| ());
    }

    async fn cursor_up(&mut self) {
        self.run(Entry::CursorUp, (), |_| ());
    }

    async fn cursor_down(&mut self) {
        self.run(Entry::CursorDown, (), |_| ());
    }

    async fn set_surrounding_text(&mut self, text: Variant<'_>, cursor_pos: u32, anchor_pos: u32) {
        self.run(Entry::SetSurroundingText, (), |session| {
            if let Some(text) = text::decode(&text) {
                session.set_surrounding_text(&text, cursor_pos, anchor_pos);
            }
        });
    }

    async fn panel_extension_received(&mut self, event: Variant<'_>) {
        drop(event);
        self.run(Entry::PanelExtensionReceived, (), |_| ());
    }

    async fn panel_extension_register_keys(&mut self, data: Variant<'_>) {
        drop(data);
        self.run(Entry::PanelExtensionRegisterKeys, (), |_| ());
    }

    /// Written by ibus-daemon: the field's purpose and hints.
    #[zbus(property)]
    async fn set_content_type(&mut self, content_type: (u32, u32)) {
        let (purpose, hints) = content_type;
        self.run(Entry::SetContentType, (), |session| {
            session.set_content_type(purpose, hints)
        });
    }

    /// False, so ibus-daemon calls `FocusIn` and `FocusOut`. libibus
    /// declares the type `(b)` but answers `b`, which ibus-daemon reads
    /// (`bus/engineproxy.c`).
    #[zbus(property)]
    async fn focus_id(&self) -> bool {
        self.process.contain(Entry::GetFocusId, false, || false)
    }

    /// True, so ibus-daemon asks the client for surrounding text on focus
    /// and on `Enable`.
    #[zbus(property)]
    async fn active_surrounding_text(&self) -> bool {
        self.process
            .contain(Entry::GetActiveSurroundingText, true, || true)
    }

    #[zbus(signal)]
    async fn commit_text(emitter: &SignalEmitter<'_>, text: &Variant<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn update_preedit_text(
        emitter: &SignalEmitter<'_>,
        text: &Variant<'_>,
        cursor_pos: u32,
        visible: bool,
        mode: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn delete_surrounding_text(
        emitter: &SignalEmitter<'_>,
        offset: i32,
        nchars: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn require_surrounding_text(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

/// `org.freedesktop.IBus.Service` of one engine object.
pub struct Service {
    process: Arc<Process>,
    exit: Arc<Notify>,
}

impl Service {
    pub fn new(process: Arc<Process>, exit: Arc<Notify>) -> Service {
        Service { process, exit }
    }
}

// [spec:kbdgen:req:linux.ibus.factory]
// [spec:kbdgen:req:linux.ibus.panic]
#[interface(
    name = "org.freedesktop.IBus.Service",
    spawn = false,
    introspection_docs = false
)]
impl Service {
    /// Unexports the engine object. The process exits once a poisoned
    /// process has no engine left.
    async fn destroy(
        &self,
        #[zbus(object_server)] server: &ObjectServer,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        self.process.contain(Entry::Destroy, (), || ());
        let path = emitter.path().to_owned();
        let engine = server.remove::<Engine, _>(&path).await.is_ok();
        let _ = server.remove::<Service, _>(&path).await;
        if engine {
            self.process.destroyed();
        }
        if self.process.done() {
            self.exit.notify_one();
        }
    }
}
