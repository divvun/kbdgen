//! `kbdgen ldml compile`: compiled layouts to the `DVKB` engine models that
//! host packaging embeds.

use std::path::{Path, PathBuf};

use kbd_model::{EncodeError, Host, Layout};

/// The layouts and hosts a run covers. Empty lists mean every layout, in
/// bundle layout order, and every host, in `ldml.yaml.hosts` order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    pub layouts: Vec<String>,
    pub hosts: Vec<Host>,
}

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("no layout is tagged {0}")]
    UnknownLayout(String),
    #[error("layout {tag}: the {host} keyboard could not be encoded: {source}")]
    Encode {
        tag: String,
        host: &'static str,
        source: EncodeError,
    },
    #[error("layout {tag}: the {host} keyboard does not load in the engine: {source}")]
    Load {
        tag: String,
        host: &'static str,
        source: kbd_engine::Error,
    },
    #[error("could not write {}: {source}", path.display())]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// The selected models as (file name, bytes), layouts in the given order
/// and hosts in host order, each named `<tag>.<host>.dvkb`. Each model is
/// decoded again by the engine, so a file is only produced when the device
/// will load it.
/// Hosts that share a keyboard get identical bytes, because the shared
/// keyboard is encoded once per host from the same value.
pub fn encode(
    layouts: &[Layout],
    selection: &Selection,
) -> Result<Vec<(String, Vec<u8>)>, CompileError> {
    if let Some(tag) = selection
        .layouts
        .iter()
        .find(|tag| !layouts.iter().any(|l| &l.tag == *tag))
    {
        return Err(CompileError::UnknownLayout(tag.clone()));
    }
    let mut files = Vec::new();
    for layout in layouts {
        if !selection.layouts.is_empty() && !selection.layouts.contains(&layout.tag) {
            continue;
        }
        for host in Host::ALL {
            if !selection.hosts.is_empty() && !selection.hosts.contains(&host) {
                continue;
            }
            let Some(keyboard) = layout.keyboard_for(host) else {
                continue;
            };
            let bytes = keyboard.to_bytes().map_err(|source| CompileError::Encode {
                tag: layout.tag.clone(),
                host: host.name(),
                source,
            })?;
            kbd_engine::Model::from_bytes(&bytes).map_err(|source| CompileError::Load {
                tag: layout.tag.clone(),
                host: host.name(),
                source,
            })?;
            files.push((format!("{}.{}.dvkb", layout.tag, host.name()), bytes));
        }
    }
    Ok(files)
}

// [spec:kbdgen:req:ldml.cli.compile+1]
/// Writes `<out>/<tag>.<host>.dvkb` for each selected layout and host that
/// has a keyboard, creating `out` with its parents. Every model is encoded
/// and checked before the first file is written, so a failure writes
/// nothing. Returns the written paths in order.
pub fn compile(
    layouts: &[Layout],
    selection: &Selection,
    out: &Path,
) -> Result<Vec<PathBuf>, CompileError> {
    let files = encode(layouts, selection)?;
    std::fs::create_dir_all(out).map_err(|source| CompileError::Write {
        path: out.to_path_buf(),
        source,
    })?;
    files
        .into_iter()
        .map(|(name, bytes)| {
            let path = out.join(name);
            std::fs::write(&path, bytes).map_err(|source| CompileError::Write {
                path: path.clone(),
                source,
            })?;
            Ok(path)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kbd_model::{
        Form, Hardware, HardwareLayer, Info, Key, Keyboard, ModifierSet, Modifiers, Normalization,
        Text,
    };
    use std::collections::BTreeMap;

    fn keyboard(host: Host, output: &str) -> Keyboard {
        let mut k = Keyboard::new(
            "sme",
            46,
            Info {
                name: "Davvisámegiella".to_string(),
                ..Info::default()
            },
        );
        k.host = Some(host);
        k.normalization = Normalization::Disabled;
        k.keys = vec![Key::new("a", Text::from(output))];
        k.hardware = Some(Hardware {
            form: Form {
                id: "iso".to_string(),
                rows: vec![vec![0x1E]],
            },
            min_device_width: None,
            layers: vec![HardwareLayer {
                id: None,
                modifiers: vec![ModifierSet::Set(Modifiers::NONE)],
                rows: vec![vec![0]],
            }],
        });
        k.context_len = 1;
        k
    }

    fn layout(tag: &str) -> Layout {
        Layout::from_host_documents(
            tag.to_string(),
            BTreeMap::new(),
            vec![
                keyboard(Host::Windows, "á"),
                keyboard(Host::MacOs, "á"),
                keyboard(Host::Ios, "a"),
            ],
        )
        .unwrap()
    }

    fn names(paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    // [spec:kbdgen:req:ldml.cli.compile+1/test]
    #[test]
    fn writes_one_model_per_layout_host() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("nested/out");
        let paths = compile(&[layout("sme"), layout("smj")], &Selection::default(), &out).unwrap();
        assert_eq!(
            names(&paths),
            [
                "sme.windows.dvkb",
                "sme.macOS.dvkb",
                "sme.iOS.dvkb",
                "smj.windows.dvkb",
                "smj.macOS.dvkb",
                "smj.iOS.dvkb",
            ]
        );
        let read = |name: &str| std::fs::read(out.join(name)).unwrap();
        assert_eq!(read("sme.windows.dvkb"), read("sme.macOS.dvkb"));
        assert_ne!(read("sme.windows.dvkb"), read("sme.iOS.dvkb"));
        let model = kbd_engine::Model::from_bytes(&read("sme.iOS.dvkb")).unwrap();
        assert_eq!(model.keyboard().host, Some(Host::Ios));
    }

    // [spec:kbdgen:req:ldml.cli.compile+1/test]
    #[test]
    fn selection_restricts_layouts_and_hosts() {
        let selection = Selection {
            layouts: vec!["smj".to_string()],
            hosts: vec![Host::Ios, Host::Android, Host::Windows],
        };
        let files = encode(&[layout("sme"), layout("smj")], &selection).unwrap();
        let names: Vec<_> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["smj.windows.dvkb", "smj.iOS.dvkb"]);
    }

    // [spec:kbdgen:req:ldml.cli.compile+1/test]
    #[test]
    fn failures_write_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let selection = Selection {
            layouts: vec!["fkv".to_string()],
            ..Selection::default()
        };
        let err = compile(&[layout("sme")], &selection, &out).unwrap_err();
        assert!(matches!(err, CompileError::UnknownLayout(tag) if tag == "fkv"));
        let mut broken = layout("sme");
        broken.keyboards[1].context_len = 7;
        let err = compile(&[layout("smj"), broken], &Selection::default(), &out).unwrap_err();
        assert!(
            matches!(err, CompileError::Load { host: "iOS", .. }),
            "{err}"
        );
        assert!(!out.exists());
    }
}
