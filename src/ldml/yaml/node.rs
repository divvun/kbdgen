//! Strict access to a parsed YAML tree (`ldml.yaml.strict`): every
//! mapping is read field by field, and a field nobody asked for is an
//! error naming its path. A null value counts as an empty mapping or list,
//! so a field whose entries are all commented out is still valid.

use serde_yaml::Value;

use super::error::{At, Result};

fn describe(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("{s:?}"),
        Value::Sequence(_) => "a list".to_string(),
        Value::Mapping(_) => "a mapping".to_string(),
        Value::Tagged(t) => format!("a value tagged {}", t.tag),
    }
}

/// The entries of a mapping, in file order, with string keys.
pub fn entries<'a>(value: &'a Value, at: &At) -> Result<Vec<(&'a str, &'a Value, At)>> {
    let map = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Mapping(map) => map,
        other => return Err(at.error(format!("expected a mapping, found {}", describe(other)))),
    };
    map.iter()
        .map(|(k, v)| match k {
            Value::String(k) => Ok((k.as_str(), v, at.key(k))),
            other => Err(at.error(format!(
                "the key {} is not a string; quote it",
                describe(other)
            ))),
        })
        .collect()
}

/// A mapping with a fixed set of fields.
pub struct Fields<'a> {
    at: At,
    entries: Vec<(&'a str, &'a Value, At, bool)>,
}

impl<'a> Fields<'a> {
    pub fn new(value: &'a Value, at: &At) -> Result<Self> {
        Ok(Fields {
            at: at.clone(),
            entries: entries(value, at)?
                .into_iter()
                .map(|(k, v, a)| (k, v, a, false))
                .collect(),
        })
    }

    /// The value of field `name`, if present, marking it as known.
    pub fn take(&mut self, name: &str) -> Option<(&'a Value, At)> {
        let entry = self.entries.iter_mut().find(|e| e.0 == name)?;
        entry.3 = true;
        Some((entry.1, entry.2.clone()))
    }

    /// Field `name`, which must be present.
    pub fn require(&mut self, name: &str) -> Result<(&'a Value, At)> {
        let at = self.at.clone();
        self.take(name)
            .ok_or_else(|| at.error(format!("the field {name} is required")))
    }

    /// The names of the fields present, in file order.
    pub fn names(&self) -> Vec<&'a str> {
        self.entries.iter().map(|e| e.0).collect()
    }

    /// Fails on the first field that was never taken.
    pub fn finish(self) -> Result<()> {
        match self.entries.into_iter().find(|e| !e.3) {
            Some((name, _, at, _)) => Err(at.error(format!("unknown field {name}"))),
            None => Ok(()),
        }
    }
}

/// A YAML string; other scalars are refused rather than converted, so
/// `1.10` cannot silently become `1.1`.
pub fn string<'a>(value: &'a Value, at: &At) -> Result<&'a str> {
    match value {
        Value::String(s) => Ok(s),
        other => Err(at.error(format!(
            "expected a string, found {}; quote it",
            describe(other)
        ))),
    }
}

pub fn boolean(value: &Value, at: &At) -> Result<bool> {
    match value {
        Value::Bool(b) => Ok(*b),
        other => Err(at.error(format!("expected true or false, found {}", describe(other)))),
    }
}

pub fn integer(value: &Value, at: &At) -> Result<u64> {
    match value {
        Value::Number(n) if n.as_u64().is_some() => Ok(n.as_u64().unwrap_or_default()),
        other => Err(at.error(format!(
            "expected a whole number, found {}",
            describe(other)
        ))),
    }
}

/// A scalar written as a string or a number, as its text: widths and the
/// numeric lists of `reorder`.
pub fn number_text(value: &Value, at: &At) -> Result<String> {
    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        other => Err(at.error(format!(
            "expected a number or a string, found {}",
            describe(other)
        ))),
    }
}

pub fn list<'a>(value: &'a Value, at: &At) -> Result<Vec<(&'a Value, At)>> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Sequence(items) => Ok(items
            .iter()
            .enumerate()
            .map(|(i, v)| (v, at.index(i)))
            .collect()),
        other => Err(at.error(format!("expected a list, found {}", describe(other)))),
    }
}
