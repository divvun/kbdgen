//! Variables, resolved in definition order (`ldml.xml.resolve` step 3):
//! each may refer only to variables defined before it.

use kbd_model::ClassRange;

use crate::diag::Result;
use crate::escape::{Piece, decode_text};
use crate::syntax::{parse_set, parse_uset, substitute_strings};
use crate::tree::El;

struct StringVar {
    id: String,
    /// The value with its own `${…}` substituted, still escaped; `from`
    /// patterns substitute it textually.
    raw: String,
    pieces: Vec<Piece>,
}

#[derive(Default)]
pub(crate) struct Vars {
    strings: Vec<StringVar>,
    sets: Vec<(String, Vec<Vec<Piece>>)>,
    usets: Vec<(String, Vec<ClassRange>)>,
}

impl Vars {
    pub(crate) fn read(root: &El) -> Result<Vars> {
        let mut vars = Vars::default();
        let Some(variables) = root.child("variables") else {
            return Ok(vars);
        };
        for var in variables.children.iter().filter(|c| c.prefix.is_none()) {
            let id = var.attr("id").unwrap_or("").to_string();
            let value = var.attr("value").unwrap_or("");
            let err = |e: crate::escape::SyntaxError| var.attr_error("value", e.to_string());
            match var.name.as_str() {
                "string" => {
                    let raw = substitute_strings(value, &|s| vars.string_raw(s)).map_err(err)?;
                    let pieces = decode_text(&raw, &|_| None).map_err(err)?;
                    vars.strings.push(StringVar { id, raw, pieces });
                }
                "set" => {
                    let raw = substitute_strings(value, &|s| vars.string_raw(s)).map_err(err)?;
                    let items = parse_set(&raw, &|s| vars.set(s)).map_err(err)?;
                    vars.sets.push((id, items));
                }
                "uset" => {
                    let raw = substitute_strings(value, &|s| vars.string_raw(s)).map_err(err)?;
                    let ranges = parse_uset(&raw, &|s| vars.uset(s)).map_err(err)?;
                    vars.usets.push((id, ranges));
                }
                _ => {}
            }
        }
        Ok(vars)
    }

    pub(crate) fn string_raw(&self, id: &str) -> Option<String> {
        self.strings
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.raw.clone())
    }

    pub(crate) fn string_pieces(&self, id: &str) -> Option<Vec<Piece>> {
        self.strings
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.pieces.clone())
    }

    pub(crate) fn set(&self, id: &str) -> Option<Vec<Vec<Piece>>> {
        self.sets
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, v)| v.clone())
    }

    pub(crate) fn uset(&self, id: &str) -> Option<Vec<ClassRange>> {
        self.usets
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, v)| v.clone())
    }
}
