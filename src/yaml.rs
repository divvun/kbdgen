//! Every YAML text kbdgen reads is parsed here, and every one it writes is
//! serialized here, so they all follow the same rules.
//!
//! Reading follows YAML 1.2: `no`, `yes`, `on`, `off`, `y` and `n` are
//! strings, so `no` can name Norwegian, and so are `0755` and `1_000`. A
//! tag YAML itself does not define (`!foo bar`), a duplicate key, or a
//! non-finite number such as `.inf` is an error, and `<<` is an ordinary
//! key, so nothing merges mappings. A text holds one document.
//!
//! A text is read either as a tree ([`tree`]), where every mapping key must
//! be a string, or straight into a type ([`from_str`]), where any scalar
//! reads as a string field: `version: 1.0` gives `"1.0"`. Errors give the
//! path to the problem and its line and column, on one line.

use std::cell::RefCell;
use std::fmt;

use serde::de::{self, DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Number, Value};
use serde_path_to_error::Segment;
use serde_saphyr::{DuplicateKeyPolicy, MergeKeyPolicy, NonFiniteFloatPolicy, SingleQuoted};

fn options() -> serde_saphyr::Options {
    serde_saphyr::options! {
        strict_booleans: true,
        leading_zero_numbers_as_strings: true,
        underscored_numbers_as_strings: true,
        // A string field takes `~`, `null` and an empty value as that text.
        null_as_string: true,
        // A `!!binary` string is its base64 text, not the bytes it encodes.
        ignore_binary_tag_for_string: true,
        reject_unsupported_tags: true,
        duplicate_keys: DuplicateKeyPolicy::Error,
        merge_keys: MergeKeyPolicy::AsOrdinary,
        non_finite_float_policy: NonFiniteFloatPolicy::Reject,
        with_snippet: false,
    }
}

/// `n` spelled as YAML writes it: `1e16`, where serde_json writes `1e+16`.
pub fn number(n: &Number) -> String {
    n.to_string().replacen("e+", "e", 1)
}

/// A YAML text that does not parse, or a tree that does not have the shape
/// of the type read from it.
#[derive(Debug)]
pub struct Error {
    path: Vec<Segment>,
    problem: Box<Problem>,
}

#[derive(Debug)]
enum Problem {
    Text(serde_saphyr::Error),
    Tree(serde_json::Error),
}

impl Error {
    fn new(path: Option<&serde_path_to_error::Path>, problem: Problem) -> Error {
        Error {
            path: path
                .into_iter()
                .flatten()
                .filter(|segment| !matches!(segment, Segment::Unknown))
                .cloned()
                .collect(),
            problem: Box::new(problem),
        }
    }

    /// Where the problem is, from the document's root.
    pub fn segments(&self) -> &[Segment] {
        &self.path
    }

    /// The path to the problem, such as `windows.primary.layers` or
    /// `dependencies.sme.layouts[0]`, and `.` for the whole document.
    pub fn path(&self) -> String {
        if self.path.is_empty() {
            return ".".to_string();
        }
        let mut path = String::new();
        for segment in &self.path {
            if !path.is_empty() && !matches!(segment, Segment::Seq { .. }) {
                path.push('.');
            }
            path.push_str(&segment.to_string());
        }
        path
    }

    /// What is wrong, with its line and column when the text gives one.
    pub fn message(&self) -> String {
        match self.problem.as_ref() {
            Problem::Text(e) => e.render_with_formatter(&serde_saphyr::UserMessageFormatter),
            Problem::Tree(e) => e.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.path.is_empty() {
            write!(f, "{}: ", self.path())?;
        }
        f.write_str(&self.message())
    }
}

impl std::error::Error for Error {}

/// Reads `text` as a `T`. A list or mapping field takes only a list or a
/// mapping, so one that may be left empty needs [`empty_as_default`].
pub fn from_str<T: DeserializeOwned>(text: &str) -> Result<T, Error> {
    let mut path = None;
    serde_saphyr::with_deserializer_from_str_with_options(text, options(), |de| {
        serde_path_to_error::deserialize(de).map_err(|e| {
            path = Some(e.path().clone());
            e.into_inner()
        })
    })
    .map_err(|e| Error::new(path.as_ref(), Problem::Text(e)))
}

/// Reads `text` as a tree, its mappings in file order. A mapping key that
/// is not a string, such as `1` or `true`, is an error asking to quote it,
/// at the path of its mapping.
pub fn tree(text: &str) -> Result<Value, Error> {
    let path = RefCell::new(Vec::new());
    serde_saphyr::with_deserializer_from_str_with_options(text, options(), |de| {
        TreeSeed { path: &path }.deserialize(de)
    })
    .map_err(|e| Error {
        path: path.into_inner(),
        problem: Box::new(Problem::Text(e)),
    })
}

/// For `#[serde(deserialize_with)]` on a list or mapping field that
/// [`from_str`] reads: a null or empty value, such as a field whose entries
/// are all commented out, gives an empty collection.
pub fn empty_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: de::Deserializer<'de>,
    T: de::Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// Reads one node of a tree. `path` leads to the node being read, so after
/// an error it leads to the node that failed.
#[derive(Clone, Copy)]
struct TreeSeed<'p> {
    path: &'p RefCell<Vec<Segment>>,
}

impl<'de> DeserializeSeed<'de> for TreeSeed<'_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for TreeSeed<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a YAML value")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Value, E> {
        Ok(v.into())
    }

    fn visit_u64<E>(self, v: u64) -> Result<Value, E> {
        Ok(v.into())
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom(format_args!("{v} is not a finite number")))
    }

    fn visit_str<E>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_string<E>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        loop {
            let index = items.len();
            self.path.borrow_mut().push(Segment::Seq { index });
            let Some(item) = seq.next_element_seed(self)? else {
                self.path.borrow_mut().pop();
                return Ok(Value::Array(items));
            };
            self.path.borrow_mut().pop();
            items.push(item);
        }
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut entries = Map::new();
        while let Some(key) = map.next_key_seed(Key)? {
            self.path
                .borrow_mut()
                .push(Segment::Map { key: key.clone() });
            let value = map.next_value_seed(self)?;
            self.path.borrow_mut().pop();
            entries.insert(key, value);
        }
        Ok(Value::Object(entries))
    }
}

struct Key;

impl<'de> DeserializeSeed<'de> for Key {
    type Value = String;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<String, D::Error> {
        deserializer.deserialize_any(KeyVisitor)
    }
}

struct KeyVisitor;

fn not_a_string<E: de::Error>(key: impl fmt::Display) -> E {
    E::custom(format_args!("the key {key} is not a string; quote it"))
}

impl<'de> Visitor<'de> for KeyVisitor {
    type Value = String;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string key")
    }

    fn visit_str<E>(self, v: &str) -> Result<String, E> {
        Ok(v.to_string())
    }

    fn visit_string<E>(self, v: String) -> Result<String, E> {
        Ok(v)
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<String, E> {
        Err(not_a_string(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<String, E> {
        Err(not_a_string(v))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<String, E> {
        Err(not_a_string(v))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<String, E> {
        Err(not_a_string(
            Number::from_f64(v).map_or_else(|| v.to_string(), |n| number(&n)),
        ))
    }

    fn visit_unit<E: de::Error>(self) -> Result<String, E> {
        Err(not_a_string("null"))
    }

    fn visit_none<E: de::Error>(self) -> Result<String, E> {
        Err(not_a_string("null"))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, _: A) -> Result<String, A::Error> {
        Err(not_a_string("a list"))
    }

    fn visit_map<A: MapAccess<'de>>(self, _: A) -> Result<String, A::Error> {
        Err(not_a_string("a mapping"))
    }
}

/// `value` as YAML text, in block style with two-space indents. Strings
/// with line breaks are literal blocks, and the others are quoted only
/// where YAML or a YAML 1.1 reader would read them otherwise.
pub fn to_string(value: &Value) -> Result<String, serde_saphyr::SerializeError> {
    let options = serde_saphyr::ser_options! {
        // Keeps a long one-line string on one line instead of folding it.
        folded_wrap_chars: usize::MAX,
    };
    serde_saphyr::to_string_with_options(&Out(value), options)
}

/// A tree as [`to_string`] writes it.
struct Out<'a>(&'a Value);

impl Serialize for Out<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::String(text) if single_quoted(text) => {
                SingleQuoted(text.as_str()).serialize(serializer)
            }
            Value::Array(items) => serializer.collect_seq(items.iter().map(Out)),
            Value::Object(entries) => {
                serializer.collect_map(entries.iter().map(|(key, value)| (key, Out(value))))
            }
            other => other.serialize(serializer),
        }
    }
}

/// Whether to write `text` in single quotes: serde-saphyr would quote it,
/// and it has no character that only double quotes can escape. Double
/// quotes would double every backslash of kbdgen's escapes, so `\u{A0}`
/// stays `'\u{A0}'` rather than becoming `"\\u{A0}"`.
fn single_quoted(text: &str) -> bool {
    serde_saphyr::to_string(&text).is_ok_and(|plain| plain.starts_with('"'))
        && serde_saphyr::to_string(&SingleQuoted(text)).is_ok()
}

mod from_tree;
#[cfg(test)]
mod tests;

pub use from_tree::from_tree;
