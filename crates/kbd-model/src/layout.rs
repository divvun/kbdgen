//! Compiled layouts: one keyboard per host document.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize};

use crate::keyboard::{Host, Keyboard};
use crate::validate::{InvariantError, NfdCheck};

// [spec:kbdgen:def:ldml.model.layout+1]
/// A compiled layout.
///
/// Hosts whose keyboards are equal in every field except `host` share one
/// entry, which then has no `host`; each consumer supplies its own host
/// (the engine's `Options.host`). A keyboard used by one host keeps that
/// host, and no host maps to another host's keyboard.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Layout {
    /// The layout's normalised language tag.
    pub tag: String,
    /// Tag → name.
    pub display_names: BTreeMap<String, String>,
    /// No two are equal, even ignoring `host`.
    pub keyboards: Vec<Keyboard>,
    /// Host → index into `keyboards`.
    pub hosts: BTreeMap<Host, u16>,
}

/// Why a [`Layout`] is malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    /// Keyboard `index` violates an invariant.
    Keyboard { index: usize, error: InvariantError },
    /// Keyboards `first` and `second` differ at most in `host`.
    Duplicate { first: usize, second: usize },
    /// `host` names no keyboard.
    HostIndex { host: Host },
    /// `host` maps to a keyboard built for another host.
    HostMismatch { host: Host },
    /// Keyboard `index` serves no host.
    Unused { index: usize },
    /// Keyboard `index` has no `host` but serves only one host.
    HostDropped { index: usize },
    /// More than 65536 distinct keyboards.
    TooManyKeyboards,
    /// Host document `document` has no `host`.
    MissingHost { document: usize },
    /// Two host documents are for `host`.
    DuplicateHost { host: Host },
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutError::Keyboard { index, error } => write!(f, "keyboards[{index}]: {error}"),
            LayoutError::Duplicate { first, second } => write!(
                f,
                "keyboards[{first}] and keyboards[{second}] differ at most in host"
            ),
            LayoutError::HostIndex { host } => write!(f, "host {} names no keyboard", host.name()),
            LayoutError::HostMismatch { host } => {
                write!(f, "host {} maps to another host's keyboard", host.name())
            }
            LayoutError::Unused { index } => write!(f, "keyboards[{index}] serves no host"),
            LayoutError::HostDropped { index } => {
                write!(f, "keyboards[{index}] serves one host but has no host")
            }
            LayoutError::TooManyKeyboards => f.write_str("layout has too many keyboards"),
            LayoutError::MissingHost { document } => {
                write!(f, "host document {document} names no host")
            }
            LayoutError::DuplicateHost { host } => {
                write!(f, "two host documents are for {}", host.name())
            }
        }
    }
}

impl core::error::Error for LayoutError {}

fn same_except_host(a: &Keyboard, b: &Keyboard) -> bool {
    let mut a = a.clone();
    a.host = b.host;
    a == *b
}

impl Layout {
    /// Builds a layout from one keyboard per host document, in host order.
    /// Each keyboard's `host` names its host. Keyboards equal except for
    /// `host` become one entry without a host, at the position of the
    /// first of them.
    pub fn from_host_documents(
        tag: String,
        display_names: BTreeMap<String, String>,
        documents: Vec<Keyboard>,
    ) -> Result<Layout, LayoutError> {
        let mut keyboards: Vec<Keyboard> = Vec::new();
        let mut hosts = BTreeMap::new();
        for (n, document) in documents.into_iter().enumerate() {
            let Some(host) = document.host else {
                return Err(LayoutError::MissingHost { document: n });
            };
            let index = match keyboards
                .iter()
                .position(|k| same_except_host(k, &document))
            {
                Some(i) => {
                    if let Some(shared) = keyboards.get_mut(i) {
                        shared.host = None;
                    }
                    i
                }
                None => {
                    keyboards.push(document);
                    keyboards.len().saturating_sub(1)
                }
            };
            let index = u16::try_from(index).map_err(|_| LayoutError::TooManyKeyboards)?;
            if hosts.insert(host, index).is_some() {
                return Err(LayoutError::DuplicateHost { host });
            }
        }
        Ok(Layout {
            tag,
            display_names,
            keyboards,
            hosts,
        })
    }

    /// The keyboard of `host`, if the layout has a document for it.
    pub fn keyboard_for(&self, host: Host) -> Option<&Keyboard> {
        self.keyboards.get(usize::from(*self.hosts.get(&host)?))
    }

    /// Checks every keyboard, that keyboards are distinct even ignoring
    /// `host`, that every host names a keyboard built for it or shared,
    /// that every keyboard serves a host, and that a keyboard serving one
    /// host keeps that host.
    pub fn validate(&self, nfd: Option<&dyn NfdCheck>) -> Result<(), LayoutError> {
        for (index, keyboard) in self.keyboards.iter().enumerate() {
            keyboard
                .validate(nfd)
                .map_err(|error| LayoutError::Keyboard { index, error })?;
            if let Some(first) = self
                .keyboards
                .iter()
                .take(index)
                .position(|k| same_except_host(k, keyboard))
            {
                return Err(LayoutError::Duplicate {
                    first,
                    second: index,
                });
            }
        }
        let mut used: BTreeMap<u16, usize> = BTreeMap::new();
        for (&host, &index) in &self.hosts {
            let Some(keyboard) = self.keyboards.get(usize::from(index)) else {
                return Err(LayoutError::HostIndex { host });
            };
            if keyboard.host.is_some_and(|h| h != host) {
                return Err(LayoutError::HostMismatch { host });
            }
            let count = used.entry(index).or_insert(0);
            *count = count.saturating_add(1);
        }
        for (index, keyboard) in self.keyboards.iter().enumerate() {
            let hosts = u16::try_from(index)
                .ok()
                .and_then(|i| used.get(&i).copied())
                .unwrap_or(0);
            if hosts == 0 {
                return Err(LayoutError::Unused { index });
            }
            if hosts == 1 && keyboard.host.is_none() {
                return Err(LayoutError::HostDropped { index });
            }
        }
        Ok(())
    }
}
