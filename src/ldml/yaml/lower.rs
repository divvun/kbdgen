//! A v4 layout to one source document per host (`ldml.yaml.hosts`,
//! `ldml.yaml.lowering`).

use kbd_ldml::escape::encode_text;
use kbd_ldml::{LayoutData, SourceDocument, implied_keys, nfd_str};
use kbd_model::{Host, Normalization};
use xmlem::Document;

use super::deadkeys;
use super::document::{Parts, build, conforms_to};
use super::error::Result;
use super::keys::KeyTable;
use super::layers;
use super::schema::{HardwareVariant, Layout4, LdmlRef, TouchVariant};
use crate::ldml::LdmlError;

/// Each host with the variants it may take, first present wins: hardware
/// chain, then touch chain.
pub const HOSTS: [(Host, &[&str], &[&str]); 7] = [
    (Host::Windows, &["windows", "default"], &[]),
    (Host::MacOs, &["macOS", "default"], &[]),
    (Host::ChromeOs, &["chromeOS", "default"], &[]),
    (Host::Linux, &["linux", "default"], &[]),
    (Host::Ios, &[], &["iOS", "default"]),
    (
        Host::Android,
        &["android", "default"],
        &["android", "default"],
    ),
    (Host::Web, &["default"], &["default"]),
];

fn longest_output(table: &KeyTable, normalization: Normalization) -> usize {
    table
        .entries
        .iter()
        .map(|e| {
            let text: String = e
                .output
                .iter()
                .filter_map(|p| match p {
                    kbd_ldml::escape::Piece::Char(c) => Some(*c),
                    kbd_ldml::escape::Piece::Marker(_) => None,
                })
                .collect();
            match normalization {
                Normalization::Enabled => nfd_str(&text).chars().count(),
                Normalization::Disabled => text.chars().count(),
            }
        })
        .max()
        .unwrap_or(0)
}

/// Lowers the variants one host takes (`ldml.yaml.lowering` steps 1–5):
/// `inherits` was resolved at load; tokens become keys with ids, implied
/// layers are added, then the dead-key keys, displays and groups.
fn document(
    layout: &Layout4,
    hardware: Option<&HardwareVariant>,
    touch: Option<&TouchVariant>,
    host: Host,
) -> Result<Document> {
    let conforms = conforms_to(layout);
    let mut table = KeyTable::new(&layout.keys, implied_keys(conforms), &layout.dead_keys);
    let hardware = hardware
        .map(|v| layers::hardware(v, &mut table, &layout.long_press, conforms))
        .transpose()?;
    let touch = match touch {
        Some(v) => layers::touch(v, &mut table, &layout.long_press)?,
        None => Vec::new(),
    };
    let longest = longest_output(&table, layout.normalization);
    let dead = deadkeys::lower(&layout.dead_keys, &table.reached, longest);
    let parts = Parts {
        layout,
        conforms_to: conforms,
        hardware,
        touch,
        table,
        dead,
    };
    Ok(build(&parts, host))
}

// [spec:kbdgen:req:ldml.yaml.ldml-ref+2]
fn referenced(
    layout: &Layout4,
    reference: &LdmlRef,
) -> Result<Vec<(Host, SourceDocument)>, LdmlError> {
    let mut out = Vec::new();
    for host in Host::ALL {
        let path = match reference {
            LdmlRef::All(path) => Some(path),
            LdmlRef::PerHost(map) => map
                .iter()
                .find(|(h, _)| h == host.name())
                .or_else(|| map.iter().find(|(h, _)| h == "default"))
                .map(|(_, p)| p),
        };
        let Some(path) = path else {
            continue;
        };
        let mut source = kbd_ldml::read_keyboard_file(path)?;
        let data = LayoutData {
            tag: Some(layout.tag.clone()),
            host: Some(host),
            decimal: layout.decimal.as_ref().map(|d| encode_text(d)),
            space_label: layout.space_label.clone(),
            return_label: layout.return_label.clone(),
            implied_layers: None,
            display_names: layout.display_names.clone(),
            targets: layout.targets.entries.clone(),
        };
        kbd_ldml::replace_extensions(&mut source.document, &data);
        out.push((host, source));
    }
    Ok(out)
}

/// The indices of the hardware and touch variants a host takes.
type Variants = (Option<usize>, Option<usize>);

// [spec:kbdgen:sem:ldml.yaml.hosts]
/// The host documents of a layout in host order. A host takes the first
/// variant of its chains that the layout has; a host with neither a
/// hardware nor a touch variant has no document. Hosts that take the same
/// variants get the same document, apart from `kbdgen:keyboard@host`, so
/// they share a model (`ldml.model.layout`). A layout with `ldml:` takes
/// its documents from the files it names.
pub fn lower(layout: &Layout4) -> Result<Vec<(Host, SourceDocument)>, LdmlError> {
    if let Some(reference) = &layout.ldml {
        return referenced(layout, reference);
    }
    let pick = |chain: &[&str], names: Vec<&str>| {
        chain.iter().find_map(|c| names.iter().position(|n| n == c))
    };
    let mut built: Vec<(Variants, Document)> = Vec::new();
    let mut out = Vec::new();
    for (host, hardware_chain, touch_chain) in HOSTS {
        let hw = pick(
            hardware_chain,
            layout.hardware.iter().map(|v| v.name.as_str()).collect(),
        );
        let touch = pick(
            touch_chain,
            layout.touch.iter().map(|v| v.name.as_str()).collect(),
        );
        if hw.is_none() && touch.is_none() {
            continue;
        }
        let mut doc = match built.iter().find(|(k, _)| *k == (hw, touch)) {
            Some((_, doc)) => doc.clone(),
            None => {
                let doc = document(
                    layout,
                    hw.and_then(|i| layout.hardware.get(i)),
                    touch.and_then(|i| layout.touch.get(i)),
                    host,
                )?;
                built.push(((hw, touch), doc.clone()));
                doc
            }
        };
        kbd_ldml::set_host(&mut doc, host);
        let name = format!("{} ({})", layout.file, host.name());
        out.push((host, SourceDocument::generated(&name, doc)));
    }
    Ok(out)
}
