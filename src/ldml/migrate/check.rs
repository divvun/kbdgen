//! The migrated layout run through the engine: caps differences
//! (`ldml.migrate.caps-diff`, M09) and the equivalence check
//! (`ldml.migrate.equivalence`, M99).

use std::path::Path;

use kbd_engine::harness::Harness;
use kbd_engine::{Gesture, Key, KeyEvent, Model, ModifierState, Options};
use kbd_model::{Direction, Host};
use serde_yaml::Value;

use super::convert::{Cell, DeadDef, DesktopTrace, Out, POSITIONS, TouchTrace, Trace};
use super::defect::{Code, Defects};
use super::oracle::{CapsState, caps_output};
use super::source::{Desktop, Touch};
use crate::ldml::yaml::{Layout4, YamlProblem, load_value, lower, position_scan_code};

const SPACE_SCAN_CODE: u8 = 0x39;

/// A loaded migration: the v4 layout and one engine model per host.
pub struct Loaded {
    pub layout: Layout4,
    pub models: Vec<(Host, Model)>,
}

/// Loads migrated text as `kbdgen` would load it from `path`: parsed,
/// lowered for every host, resolved and given to the engine. The warnings
/// of loading and resolution come back with it.
pub fn load(text: &str, path: &Path, tag: &str) -> Result<(Loaded, Vec<String>), String> {
    let value: Value = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
    let layout = load_value(path, tag, &value).map_err(|e| e.to_string())?;
    let mut warnings: Vec<String> = layout.warnings.iter().map(YamlProblem::to_string).collect();
    let mut models = Vec::new();
    for (host, source) in lower(&layout).map_err(|e| e.to_string())? {
        let resolved = kbd_ldml::resolve(&source).map_err(|e| e.to_string())?;
        warnings.extend(resolved.warnings.iter().map(|w| w.to_string()));
        let mut keyboard = resolved.keyboard;
        keyboard.host = Some(host);
        let options = Options {
            host: Some(host),
            ..Options::default()
        };
        let model = Model::from_keyboard(keyboard, options)
            .map_err(|e| format!("the {} keyboard does not load: {e}", host.name()))?;
        models.push((host, model));
    }
    Ok((Loaded { layout, models }, warnings))
}

fn host_of_desktop(platform: Desktop) -> Host {
    match platform {
        Desktop::Windows => Host::Windows,
        Desktop::MacOs => Host::MacOs,
        Desktop::ChromeOs => Host::ChromeOs,
    }
}

fn host_of_touch(platform: Touch) -> Host {
    match platform {
        Touch::Ios => Host::Ios,
        Touch::Android => Host::Android,
    }
}

/// The modifier state of a v3 desktop layer, or none for a native-only
/// layer. Windows and ChromeOS reach `alt` through AltGr.
fn modifiers(platform: Desktop, layer: &str) -> Option<ModifierState> {
    let mut m = ModifierState::default();
    for part in layer.split('+') {
        match part {
            "default" => {}
            "shift" => m.shift_l = true,
            "caps" => m.caps = true,
            "alt" if platform == Desktop::MacOs => m.alt_l = true,
            "alt" => {
                m.alt_r = true;
                m.altgr = true;
            }
            _ => return None,
        }
    }
    Some(m)
}

/// The identity of a dead key from its marker, which the migrator never
/// names: `dk_` and the identity's scalars in hex (`ldml.yaml.dead-keys.keys`).
fn identity_of(marker: &str) -> String {
    marker
        .strip_prefix("dk_")
        .and_then(|hex| {
            hex.split('_')
                .map(|h| u32::from_str_radix(h, 16).ok().and_then(char::from_u32))
                .collect::<Option<String>>()
        })
        .unwrap_or_else(|| marker.to_string())
}

/// What the engine leaves after the events: the text typed, else the
/// last pending dead key, else nothing. A key that passes types nothing
/// of the layout's.
fn press(model: &Model, events: &[KeyEvent]) -> Out {
    let mut h = Harness::new(model);
    for e in events {
        h.send(e);
    }
    if !h.document().is_empty() {
        return Out::Text(h.document().to_string());
    }
    match model.pending_markers(h.state()).last() {
        Some(marker) => Out::Dead(identity_of(marker)),
        None => Out::NoKey,
    }
}

fn scan(position: usize, m: ModifierState) -> KeyEvent {
    let code = POSITIONS
        .get(position)
        .and_then(|p| position_scan_code(p))
        .unwrap_or(0);
    KeyEvent::with(Key::Scan(code), m)
}

fn show(out: &Out) -> String {
    match out {
        Out::NoKey => "no key".to_string(),
        Out::Text(t) => format!("{t:?}"),
        Out::Dead(t) => format!("dead {t:?}"),
    }
}

/// Reports an unexplained difference (M99) at a v3 key.
fn unexplained(
    defects: &mut Defects,
    cell: &Cell,
    actual: &Out,
    path: &str,
    row: usize,
    what: &str,
) {
    if cell.out == *actual || !cell.explained.is_empty() {
        return;
    }
    defects.add_at(
        Code::M99,
        path,
        Some(row),
        None,
        format!(
            "{what}: v3 typed {}, the migrated layout types {}",
            show(&cell.out),
            show(actual)
        ),
    );
}

/// The row of the ISO position `index` in a desktop layer, 1-based.
fn iso_row(index: usize) -> usize {
    match index {
        0..=12 => 1,
        13..=24 => 2,
        25..=36 => 3,
        _ => 4,
    }
}

/// The positions where a caps state of a desktop variant types otherwise
/// than its v3 platform did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapsDiff {
    pub platform: &'static str,
    pub state: CapsState,
    pub positions: Vec<&'static str>,
}

// [spec:kbdgen:req:ldml.migrate.caps-diff+1]
/// Lists, per caps state, every position where v4 types other than the v3
/// platform did (M09).
fn caps_diff(trace: &DesktopTrace, model: &Model, defects: &mut Defects) -> Vec<CapsDiff> {
    let path = format!("{}.primary.layers", trace.platform.name());
    let mut out = Vec::new();
    for state in CapsState::ALL {
        let Some(m) = modifiers(trace.platform, state.name()) else {
            continue;
        };
        let mut changes = Vec::new();
        let mut positions = Vec::new();
        for (i, name) in POSITIONS.iter().enumerate() {
            let old = caps_output(trace, state, i);
            let new = press(model, &[scan(i, m)]);
            if old != new {
                changes.push(format!("{name} {} → {}", show(&old), show(&new)));
                positions.push(*name);
            }
        }
        if !changes.is_empty() {
            defects.add(
                Code::M09,
                &path,
                format!(
                    "{} {} now types otherwise at {} position(s): {}",
                    trace.platform.name(),
                    state.name(),
                    changes.len(),
                    changes.join(", ")
                ),
            );
            out.push(CapsDiff {
                platform: trace.platform.name(),
                state,
                positions,
            });
        }
    }
    out
}

/// Whether a layer is one of the caps states, which `caps_diff` covers.
fn caps_layer(name: &str) -> bool {
    name.contains("caps")
}

/// The leaf v3 typed for a dead key followed by an input on `platform`:
/// as written where the platform showed transform strings as written.
fn v3_leaf(platform: &str, raw: &str, text: &str) -> String {
    let decoded = platform == Desktop::MacOs.name() || platform == Touch::Android.name();
    if decoded {
        text.to_string()
    } else {
        raw.to_string()
    }
}

/// Whether a difference in a dead key's output is explained: its v3
/// string was decoded differently, and its strings were reported as shown
/// as written (M14) or rewritten (M06).
fn leaf_explained(defects: &Defects, def: &DeadDef, raw: &str, text: &str) -> bool {
    raw != text
        && (defects.has_under(Code::M14, &def.path) || defects.has_under(Code::M06, &def.path))
}

/// One key of a host document: the events that press it, and what v3
/// typed with it.
struct Press {
    event: KeyEvent,
    v3: Out,
    v4: Out,
    combines: bool,
}

/// Presses each dead key then each compose input it has a key for, and the
/// dead key then space, comparing with the v3 outputs.
fn dead_keys(
    platform: &str,
    model: &Model,
    keys: &[Press],
    dead: &[DeadDef],
    with_space: bool,
    defects: &mut Defects,
) {
    for def in dead {
        let dead = Out::Dead(def.identity.clone());
        let Some(dead_key) = keys.iter().find(|k| k.v3 == dead && k.v4 == dead) else {
            continue;
        };
        let mut check =
            |events: &[KeyEvent], expected: String, raw: &str, text: &str, what: String| {
                let actual = press(model, events);
                if actual != Out::Text(expected.clone()) && !leaf_explained(defects, def, raw, text)
                {
                    defects.add(
                    Code::M99,
                    &def.path,
                    format!(
                        "{platform}: {what}: v3 typed {expected:?}, the migrated layout types {}",
                        show(&actual)
                    ),
                );
                }
            };
        if with_space {
            let space = KeyEvent::new(Key::Scan(SPACE_SCAN_CODE));
            let s = &def.standalone;
            check(
                &[dead_key.event.clone(), space],
                v3_leaf(platform, &s.raw, &s.text),
                &s.raw,
                &s.text,
                format!("{} then space", def.spelling),
            );
        }
        for (input, leaf) in &def.compose {
            let Some(leaf) = leaf else {
                continue;
            };
            let typed = Out::Text(input.text.clone());
            let Some(key) = keys
                .iter()
                .find(|k| k.combines && matches!(&k.v3, Out::Text(_)) && k.v4 == typed)
            else {
                continue;
            };
            check(
                &[dead_key.event.clone(), key.event.clone()],
                v3_leaf(platform, &leaf.raw, &leaf.text),
                &leaf.raw,
                &leaf.text,
                format!("{} then {}", def.spelling, input.text),
            );
        }
    }
}

/// Every key of a desktop trace that is not native-only, caps states last.
fn desktop_keys(trace: &DesktopTrace, model: &Model) -> Vec<Press> {
    let mut layers: Vec<_> = trace.layers.iter().filter(|l| !l.native).collect();
    layers.sort_by_key(|l| caps_layer(&l.name));
    let mut out = Vec::new();
    for layer in layers {
        let Some(m) = modifiers(trace.platform, &layer.name) else {
            continue;
        };
        let combines = trace.platform != Desktop::MacOs || layer.has_dead_list;
        for (i, cell) in layer.cells.iter().enumerate() {
            let event = scan(i, m);
            out.push(Press {
                v4: press(model, std::slice::from_ref(&event)),
                event,
                v3: cell.out.clone(),
                combines,
            });
        }
    }
    out
}

// [spec:kbdgen:req:ldml.migrate.equivalence+1]
fn desktop(trace: &DesktopTrace, model: &Model, dead: &[DeadDef], defects: &mut Defects) {
    for layer in &trace.layers {
        if layer.native || caps_layer(&layer.name) {
            continue;
        }
        let Some(m) = modifiers(trace.platform, &layer.name) else {
            continue;
        };
        for (i, cell) in layer.cells.iter().enumerate() {
            let actual = press(model, &[scan(i, m)]);
            let what = format!("{} {}", layer.name, POSITIONS[i]);
            unexplained(defects, cell, &actual, &layer.path, iso_row(i), &what);
        }
    }
    let keys = desktop_keys(trace, model);
    dead_keys(trace.platform.name(), model, &keys, dead, true, defects);
}

fn touch_event(set: usize, layer: usize, row: usize, col: usize, gesture: Gesture) -> KeyEvent {
    KeyEvent::new(Key::Touch {
        set,
        layer,
        row,
        col,
        gesture,
    })
}

// [spec:kbdgen:req:ldml.migrate.equivalence+1]
fn touch(trace: &TouchTrace, model: &Model, dead: &[DeadDef], defects: &mut Defects) {
    let platform = trace.platform.name();
    for size in &trace.sizes {
        let Some(set) = model.touch_set_by_name(&size.name) else {
            continue;
        };
        let mut keys = Vec::new();
        for layer in &size.layers {
            let Some(li) = model
                .keyboard()
                .touch
                .get(set)
                .and_then(|s| s.layers.iter().position(|l| l.id == layer.id))
            else {
                continue;
            };
            let grids = [
                (&layer.rows, Gesture::Tap),
                (&layer.flicks, Gesture::Flick(vec![Direction::S])),
            ];
            for (grid, gesture) in grids {
                for (r, row) in grid.iter().enumerate() {
                    for (c, cell) in row.iter().enumerate() {
                        let Some(cell) = cell else {
                            continue;
                        };
                        let event = touch_event(set, li, r, c, gesture.clone());
                        let actual = press(model, std::slice::from_ref(&event));
                        let what = format!(
                            "{platform} {} {} row {} key {}{}",
                            size.name,
                            layer.id,
                            r + 1,
                            c + 1,
                            if gesture == Gesture::Tap {
                                ""
                            } else {
                                " flick s"
                            }
                        );
                        unexplained(defects, cell, &actual, &layer.path, r + 1, &what);
                        keys.push(Press {
                            event,
                            v3: cell.out.clone(),
                            v4: actual,
                            combines: true,
                        });
                    }
                }
            }
        }
        dead_keys(platform, model, &keys, dead, false, defects);
    }
}

/// Runs the caps comparison over every desktop variant, and with
/// `equivalence` the full check of `ldml.migrate.equivalence`. Returns the
/// caps differences.
pub fn run(
    loaded: &Loaded,
    trace: &Trace,
    equivalence: bool,
    defects: &mut Defects,
) -> Vec<CapsDiff> {
    let model = |host: Host| {
        loaded
            .models
            .iter()
            .find(|(h, _)| *h == host)
            .map(|(_, m)| m)
    };
    let mut diffs = Vec::new();
    for desktop_trace in &trace.desktop {
        let Some(m) = model(host_of_desktop(desktop_trace.platform)) else {
            continue;
        };
        diffs.extend(caps_diff(desktop_trace, m, defects));
        if equivalence {
            desktop(desktop_trace, m, &trace.dead, defects);
        }
    }
    if equivalence {
        for touch_trace in &trace.touch {
            if let Some(m) = model(host_of_touch(touch_trace.platform)) {
                touch(touch_trace, m, &trace.dead, defects);
            }
        }
    }
    diffs
}
