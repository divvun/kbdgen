//! v3 to v4 (`ldml.migrate.platforms`, `ldml.migrate.dead-keys`,
//! `ldml.migrate.fields`): the v4 layout, the defects found on the way,
//! and a trace of what each v3 key did on its platform, which the
//! equivalence check (`ldml.migrate.equivalence`) compares with the
//! engine.

mod dead;
mod desktop;
mod touch;

use std::collections::BTreeSet;

pub use dead::DeadDef;
pub use desktop::POSITIONS;

use super::defect::{Code, Defects};
use super::out::{HardwareOut, Out4, TouchOut};
use super::source::{Desktop, Source3, Touch};
use super::text::{braced, decode_v3, output, plain, token};

/// What a key did on its v3 platform, or does in the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    NoKey,
    Text(String),
    /// A dead key, by its decoded identity.
    Dead(String),
}

/// A v3 key: what it did on its platform and layer, and the defects that
/// explain a different v4 result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub out: Out,
    pub explained: Vec<Code>,
}

/// A desktop layer: its v3 name, its v4 key, whether it is native-only,
/// whether v3 listed dead keys for it, and its 48 keys in ISO order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerTrace {
    pub name: String,
    pub key: String,
    pub native: bool,
    pub has_dead_list: bool,
    pub path: String,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopTrace {
    pub platform: Desktop,
    pub layers: Vec<LayerTrace>,
}

/// A touch layer: its v4 id, the v3 key at each row and column (none for
/// role and gap tokens), and the v3 `alt` key that became the south flick
/// of each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchLayerTrace {
    pub id: String,
    pub path: String,
    pub rows: Vec<Vec<Option<Cell>>>,
    pub flicks: Vec<Vec<Option<Cell>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeTrace {
    pub name: String,
    pub layers: Vec<TouchLayerTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchTrace {
    pub platform: Touch,
    pub sizes: Vec<SizeTrace>,
}

/// What the migrated layout should reproduce.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trace {
    pub desktop: Vec<DesktopTrace>,
    pub touch: Vec<TouchTrace>,
    pub dead: Vec<DeadDef>,
}

/// A migrated layout before writing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    pub out: Out4,
    pub trace: Trace,
}

/// The state shared while converting one layout: its dead keys, its
/// defects, and the identities each platform makes dead.
pub struct Ctx<'a> {
    pub dead: &'a [DeadDef],
    pub defects: &'a mut Defects,
    pub used: BTreeSet<(&'static str, String)>,
}

impl<'a> Ctx<'a> {
    pub fn dead_def(&self, identity: &str) -> Option<&'a DeadDef> {
        self.dead.iter().find(|d| d.identity == identity)
    }

    pub fn decode(
        &mut self,
        raw: &str,
        path: &str,
        row: Option<usize>,
    ) -> Option<(String, String)> {
        decode_at(self.defects, raw, path, row)
    }

    pub fn output(&mut self, raw: &str, path: &str) -> String {
        match self.decode(raw, path, None) {
            Some((fixed, text)) => output(&fixed, &text),
            None => raw.to_string(),
        }
    }

    pub fn plain(&mut self, raw: &str, path: &str) -> String {
        match self.decode(raw, path, None) {
            Some((fixed, text)) => plain(&fixed, &text),
            None => raw.to_string(),
        }
    }

    pub fn literal(&mut self, raw: &str, path: &str, row: usize) -> String {
        match self.decode(raw, path, Some(row)) {
            Some((fixed, text)) => token(&fixed, &text),
            None => raw.to_string(),
        }
    }
}

/// `raw` with M06 rewrites applied and decoded, reporting at `path`.
/// `None`, with an M99, when an escape is not a scalar value.
pub fn decode_at(
    defects: &mut Defects,
    raw: &str,
    path: &str,
    row: Option<usize>,
) -> Option<(String, String)> {
    let fixed = match braced(raw) {
        Some(fixed) => {
            defects.add_at(
                Code::M06,
                path,
                row,
                Some(raw),
                format!("\\u without braces, which v3 kept as text, is now {fixed}"),
            );
            fixed
        }
        None => raw.to_string(),
    };
    match decode_v3(&fixed) {
        Ok(text) => Some((fixed, text)),
        Err(message) => {
            defects.add_at(Code::M99, path, row, Some(raw), message);
            None
        }
    }
}

/// `longpress` → `longPress`: outputs and candidate tokens in v4 spelling.
/// A later entry whose output decodes like an earlier one is dropped.
fn long_press(src: &Source3, ctx: &mut Ctx) -> Vec<(String, String)> {
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (key, value) in &src.longpress {
        let path = format!("longpress.{key}");
        if key.contains("\\u{") || value.contains("\\u{") {
            ctx.defects.add(
                Code::M14,
                &path,
                "an escape that iOS and Android showed as written is now decoded",
            );
        }
        let decoded = ctx
            .decode(key, &path, None)
            .map(|(_, t)| t)
            .unwrap_or_default();
        if seen.contains(&decoded) {
            ctx.defects.add(
                Code::M10,
                &path,
                format!("{key} decodes like an earlier entry; this one is not carried over"),
            );
            continue;
        }
        seen.push(decoded);
        let output = ctx.output(key, &path);
        let candidates: Vec<String> = value
            .split_whitespace()
            .map(|c| ctx.literal(c, &path, 1))
            .collect();
        out.push((output, candidates.join(" ")));
    }
    out
}

/// The `targets` of the v3 `config` blocks, in `ldml.yaml.targets` order.
fn targets(src: &Source3) -> Vec<(String, Vec<(String, String)>)> {
    let mut out = Vec::new();
    for section in &src.desktop {
        if section.platform != Desktop::MacOs && !section.config.is_empty() {
            out.push((section.platform.name().to_string(), section.config.clone()));
        }
    }
    for section in &src.touch {
        if !section.config.is_empty() {
            out.push((section.platform.name().to_string(), section.config.clone()));
        }
    }
    out
}

// [spec:kbdgen:sem:ldml.migrate.fields+1]
fn fields(src: &Source3, ctx: &mut Ctx, out: &mut Out4) {
    out.display_names = src.display_names.clone();
    out.decimal = src.decimal.as_ref().map(|d| ctx.output(d, "decimal"));
    out.key_names = src
        .key_names
        .iter()
        .map(|(name, value)| (name.clone(), ctx.plain(value, &format!("keyNames.{name}"))))
        .collect();
    out.long_press = long_press(src, ctx);
    out.targets = targets(src);
}

/// Reports the transform roots no platform makes dead (M03), the
/// multi-scalar identities Windows makes dead (M08), and iOS and Android
/// disagreeing on dead keys (M12).
fn dead_usage(src: &Source3, ctx: &mut Ctx) {
    for def in ctx.dead {
        if !ctx.used.iter().any(|(_, i)| *i == def.identity) {
            ctx.defects.add(
                Code::M03,
                &def.path,
                format!(
                    "no layer of any platform makes {} dead; kept as an unreferenced dead key",
                    def.spelling
                ),
            );
        }
        if def.identity.chars().count() > 1
            && ctx
                .used
                .contains(&(Desktop::Windows.name(), def.identity.clone()))
        {
            ctx.defects.add(
                Code::M08,
                &def.path,
                format!(
                    "the identity {} has more than one scalar, so the Windows layout DLL cannot make it dead",
                    def.spelling
                ),
            );
        }
    }
    let on = |platform: &str| -> BTreeSet<String> {
        ctx.used
            .iter()
            .filter(|(p, _)| *p == platform)
            .map(|(_, i)| i.clone())
            .collect()
    };
    let has = |t: Touch| src.touch.iter().any(|s| s.platform == t);
    if has(Touch::Ios) && has(Touch::Android) {
        let ios = on(Touch::Ios.name());
        let android = on(Touch::Android.name());
        if ios != android {
            let list = |s: &BTreeSet<String>| {
                s.iter()
                    .map(|i| format!("{i:?}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            ctx.defects.add(
                Code::M12,
                "",
                format!(
                    "iOS and Android dead keys differ: only iOS [{}], only Android [{}]",
                    list(&ios.difference(&android).cloned().collect()),
                    list(&android.difference(&ios).cloned().collect())
                ),
            );
        }
    }
}

// [spec:kbdgen:sem:ldml.migrate.platforms]
/// Converts a v3 layout, reporting each defect into `defects`.
pub fn convert(src: &Source3, defects: &mut Defects) -> Converted {
    let (dead, dead_keys) = dead::table(src, defects);
    let mut ctx = Ctx {
        dead: &dead,
        defects,
        used: BTreeSet::new(),
    };
    let mut out = Out4 {
        dead_keys,
        ..Out4::default()
    };
    fields(src, &mut ctx, &mut out);
    let mut trace = Trace::default();
    for section in &src.desktop {
        let (variant, layers): (HardwareOut, DesktopTrace) = desktop::convert(section, &mut ctx);
        out.hardware.push(variant);
        trace.desktop.push(layers);
    }
    for section in &src.touch {
        let (variant, sizes): (TouchOut, TouchTrace) = touch::convert(src, section, &mut ctx);
        out.touch.push(variant);
        trace.touch.push(sizes);
    }
    dead::escaped_leaves(&mut ctx);
    dead_usage(src, &mut ctx);
    drop(ctx);
    trace.dead = dead;
    Converted { out, trace }
}
