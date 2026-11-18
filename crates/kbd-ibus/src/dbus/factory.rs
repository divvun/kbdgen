//! `org.freedesktop.IBus.Factory` at `/org/freedesktop/IBus/Factory`,
//! which ibus-daemon calls once the component's bus name has an owner.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Notify;
use zbus::{ObjectServer, OwnedObjectPath, fdo, interface};

use super::engine::{Engine, Service};
use crate::data::{self, Refusal};
use crate::diag::{self, Event, FileName};
use crate::guard::{Entry, Process};
use crate::planner::Session;

pub struct Factory {
    roots: Vec<PathBuf>,
    /// The number of the last engine object exported.
    last: u32,
    process: Arc<Process>,
    exit: Arc<Notify>,
}

impl Factory {
    pub fn new(roots: Vec<PathBuf>, process: Arc<Process>, exit: Arc<Notify>) -> Factory {
        Factory {
            roots,
            last: 0,
            process,
            exit,
        }
    }

    /// The next engine object's number and its session, or why the engine
    /// `name` cannot be served.
    fn make(&self, name: &str) -> Result<(u32, Session), Refusal> {
        let model = data::lookup(&self.roots, name)?;
        Ok((self.last.saturating_add(1), Session::new(model)))
    }
}

// [spec:kbdgen:req:linux.ibus.factory]
// [spec:kbdgen:req:linux.ibus.panic]
#[interface(
    name = "org.freedesktop.IBus.Factory",
    spawn = false,
    introspection_docs = false
)]
impl Factory {
    /// Loads the model of the engine `name` and exports an engine object
    /// for it. A name the program cannot serve fails with the reason
    /// (`linux.ibus.inert`); a poisoned process serves none.
    // [spec:kbdgen:req:linux.ibus.inert]
    async fn create_engine(
        &mut self,
        name: String,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> fdo::Result<OwnedObjectPath> {
        let made = self
            .process
            .contain(Entry::CreateEngine, None, || Some(self.make(&name)));
        let result = match made {
            Some(Ok((number, session))) => {
                let path = format!("/org/freedesktop/IBus/Engine/{number}");
                let engine = Engine::new(session, self.process.clone());
                let service = Service::new(self.process.clone(), self.exit.clone());
                let exported = matches!(server.at(path.as_str(), engine).await, Ok(None));
                let served =
                    exported && matches!(server.at(path.as_str(), service).await, Ok(None));
                if exported && !served {
                    let _ = server.remove::<Engine, _>(path.as_str()).await;
                }
                match OwnedObjectPath::try_from(path) {
                    Ok(path) if served => {
                        self.last = number;
                        self.process.created();
                        Ok(path)
                    }
                    _ => Err(fdo::Error::Failed("export-failed".to_owned())),
                }
            }
            Some(Err(Refusal { reason, file })) => {
                diag::record(Event::Inert {
                    reason,
                    file: FileName::of(&file),
                });
                Err(fdo::Error::Failed(reason.code().to_owned()))
            }
            None => Err(fdo::Error::Failed("poisoned".to_owned())),
        };
        if self.process.done() {
            self.exit.notify_one();
        }
        result
    }
}
