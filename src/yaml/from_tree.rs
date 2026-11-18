//! Reading a [`tree`](super::tree) as a type through a deserializer of its
//! own, since serde_json's reads a null, a struct and an enum otherwise.

use serde::de::value::{MapDeserializer, SeqDeserializer};
use serde::de::{self, DeserializeOwned, IntoDeserializer, Unexpected, Visitor};
use serde_json::{Map, Value};

use super::{Error, Problem};

/// Reads a tree from [`tree`] as a `T`. A null is an empty list, mapping or
/// struct, a struct comes only from a mapping, and an enum only from a
/// string naming a unit variant. Errors have a path but no line, which the
/// tree does not keep.
pub fn from_tree<T: DeserializeOwned>(tree: Value) -> Result<T, Error> {
    serde_path_to_error::deserialize(TreeDeserializer(tree))
        .map_err(|e| Error::new(Some(&e.path().clone()), Problem::Tree(e.into_inner())))
}

struct TreeDeserializer(Value);

impl<'de> IntoDeserializer<'de, serde_json::Error> for TreeDeserializer {
    type Deserializer = TreeDeserializer;

    fn into_deserializer(self) -> TreeDeserializer {
        self
    }
}

fn visit_items<'de, V: Visitor<'de>>(
    items: Vec<Value>,
    visitor: V,
) -> Result<V::Value, serde_json::Error> {
    let mut items = SeqDeserializer::new(items.into_iter().map(TreeDeserializer));
    let value = visitor.visit_seq(&mut items)?;
    items.end()?;
    Ok(value)
}

fn visit_entries<'de, V: Visitor<'de>>(
    entries: Map<String, Value>,
    visitor: V,
) -> Result<V::Value, serde_json::Error> {
    let mut entries =
        MapDeserializer::new(entries.into_iter().map(|(k, v)| (k, TreeDeserializer(v))));
    let value = visitor.visit_map(&mut entries)?;
    entries.end()?;
    Ok(value)
}

fn unexpected(value: &Value) -> Unexpected<'_> {
    match value {
        Value::Null => Unexpected::Unit,
        Value::Bool(b) => Unexpected::Bool(*b),
        Value::Number(n) => match (n.as_u64(), n.as_i64()) {
            (Some(u), _) => Unexpected::Unsigned(u),
            (None, Some(i)) => Unexpected::Signed(i),
            (None, None) => Unexpected::Float(n.as_f64().unwrap_or_default()),
        },
        Value::String(s) => Unexpected::Str(s),
        Value::Array(_) => Unexpected::Seq,
        Value::Object(_) => Unexpected::Map,
    }
}

impl<'de> de::Deserializer<'de> for TreeDeserializer {
    type Error = serde_json::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.0 {
            Value::Array(items) => visit_items(items, visitor),
            Value::Object(entries) => visit_entries(entries, visitor),
            scalar => scalar.deserialize_any(visitor),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.0 {
            Value::Null => visitor.visit_none(),
            value => visitor.visit_some(TreeDeserializer(value)),
        }
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.0 {
            Value::Array(items) => visit_items(items, visitor),
            Value::Null => visit_items(Vec::new(), visitor),
            other => Err(de::Error::invalid_type(unexpected(&other), &visitor)),
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.0 {
            Value::Object(entries) => visit_entries(entries, visitor),
            Value::Null => visit_entries(Map::new(), visitor),
            other => Err(de::Error::invalid_type(unexpected(&other), &visitor)),
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.0 {
            Value::String(variant) => visitor.visit_enum(variant.into_deserializer()),
            other => Err(de::Error::invalid_type(
                unexpected(&other),
                &"a string naming a variant",
            )),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct identifier ignored_any
    }
}
