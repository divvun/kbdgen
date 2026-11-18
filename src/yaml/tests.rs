use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Value, json};

use super::*;

#[derive(Debug, Deserialize)]
struct Fields {
    s: Option<String>,
    list: Option<Vec<String>>,
}

fn error(text: &str) -> String {
    tree(text).unwrap_err().to_string()
}

#[test]
fn yaml_1_1_booleans_are_strings() {
    let words = ["no", "yes", "y", "n", "on", "off", "No", "YES", "Off"];
    for word in words {
        assert_eq!(
            tree(&format!("{word}: {word}\n")).unwrap(),
            json!({ word: word }),
            "{word}"
        );
        let typed: Fields = from_str(&format!("s: {word}\n")).unwrap();
        assert_eq!(typed.s.as_deref(), Some(word));
    }
    assert_eq!(
        tree("a: true\nb: False\n").unwrap(),
        json!({"a": true, "b": false})
    );
}

#[test]
fn numbers_keep_their_yaml_1_2_reading() {
    assert_eq!(
        tree("a: 0755\nb: 1_000\nc: 0x1F\nd: 1.10\ne: -7\nf: 0o17\n").unwrap(),
        json!({"a": "0755", "b": "1_000", "c": 31, "d": 1.1, "e": -7, "f": 15})
    );
}

#[test]
fn numbers_are_spelled_as_yaml_writes_them() {
    let spelled = |text: &str| match tree(&format!("n: {text}\n")).unwrap()["n"] {
        Value::Number(ref n) => number(n),
        ref other => panic!("{text}: {other}"),
    };
    for (text, written) in [
        ("4", "4"),
        ("-4", "-4"),
        ("1.10", "1.1"),
        ("1.0", "1.0"),
        ("1e3", "1000.0"),
        ("1e16", "1e16"),
        ("1.5e-7", "1.5e-7"),
    ] {
        assert_eq!(spelled(text), written, "{text}");
    }
}

#[test]
fn typed_string_fields_take_any_scalar() {
    for (text, read) in [
        ("1.0", "1.0"),
        ("1.10", "1.10"),
        ("true", "true"),
        ("0x1F", "0x1F"),
        ("'2'", "2"),
    ] {
        let typed: Fields = from_str(&format!("s: {text}\n")).unwrap();
        assert_eq!(typed.s.as_deref(), Some(read), "{text}");
    }
    let typed: Fields = from_str("list: [1, true, ~, '']\n").unwrap();
    assert_eq!(typed.list.unwrap(), ["1", "true", "~", ""]);
    let typed: Fields = from_str("s: ~\n").unwrap();
    assert_eq!(typed.s, None);
}

#[test]
fn tree_keys_must_be_strings() {
    for (text, key) in [
        ("1: a\n", "1"),
        ("1.10: a\n", "1.1"),
        ("true: a\n", "true"),
        ("~: a\n", "null"),
        ("[a]: b\n", "a list"),
        ("{a: b}: c\n", "a mapping"),
    ] {
        let message = error(text);
        assert!(
            message.starts_with(&format!(
                "the key {key} is not a string; quote it at line 1"
            )),
            "{text}: {message}"
        );
    }
    assert_eq!(tree("'1': a\n").unwrap(), json!({"1": "a"}));
}

#[test]
fn errors_name_path_line_and_column() {
    assert_eq!(
        error("a:\n  b:\n    - x\n    - 7: y\n"),
        "a.b[1]: the key 7 is not a string; quote it at line 4, column 7"
    );
    assert_eq!(
        error("a:\n  - x\n  - !foo y\n"),
        "a[1]: unsupported tag `!foo` at line 3, column 10"
    );
    assert_eq!(
        error("a:\n  b: 1\n  b: 2\n"),
        "a: duplicate mapping key `b` is not allowed here at line 3, column 3"
    );
    let typed = from_str::<Fields>("list: [a, [b]]\n").unwrap_err();
    assert_eq!(typed.path(), "list[1]");
    assert!(typed.message().ends_with("at line 1, column 11"), "{typed}");
}

#[test]
fn unknown_tags_are_errors() {
    for text in ["a: !foo bar\n", "a: !foo {b: c}\n", "!foo a: b\n"] {
        assert!(error(text).contains("unsupported tag `!foo`"), "{text}");
    }
    assert!(from_str::<Fields>("s: !foo x\n").is_err());
    assert_eq!(
        tree("a: !!str 1\nb: !!binary aGVsbG8=\n").unwrap(),
        json!({"a": "1", "b": "aGVsbG8="})
    );
}

#[test]
fn duplicate_keys_are_errors() {
    for text in ["a: 1\na: 2\n", "a:\n  b: 1\n  b: 2\n", "'no': 1\nno: 2\n"] {
        assert!(error(text).contains("duplicate mapping key"), "{text}");
    }
    assert!(from_str::<Fields>("s: a\ns: b\n").is_err());
    assert!(from_str::<BTreeMap<String, String>>("a: x\na: y\n").is_err());
}

#[test]
fn non_finite_numbers_are_errors() {
    for text in ["a: .inf\n", "a: -.inf\n", "a: .nan\n", "a: 1e999\n"] {
        let message = error(text);
        assert!(
            message.contains("is not a finite number"),
            "{text}: {message}"
        );
    }
    assert_eq!(tree("a: '.inf'\n").unwrap(), json!({"a": ".inf"}));
}

#[test]
fn merge_keys_are_ordinary_keys() {
    assert_eq!(
        tree("b: &b {x: 1}\nd:\n  <<: *b\n  y: 2\n").unwrap(),
        json!({"b": {"x": 1}, "d": {"<<": {"x": 1}, "y": 2}})
    );
}

#[test]
fn crlf_and_bom_read_like_lf() {
    assert_eq!(
        tree("\u{feff}a: |\r\n  x y\r\n  z\r\nb: 'c\r\n  d'\r\n# c\r\n").unwrap(),
        json!({"a": "x y\nz\n", "b": "c d"})
    );
}

#[test]
fn a_text_holds_one_document() {
    assert!(tree("a: 1\n---\nb: 2\n").is_err());
    assert_eq!(tree("").unwrap(), Value::Null);
    assert_eq!(tree("# only a comment\n").unwrap(), Value::Null);
}

#[test]
fn syntax_errors_are_one_line() {
    for text in ["a: [1, 2\n", "a: b: c\n", "a:\n\tb: c\n", "a: 'open\n"] {
        let message = error(text);
        assert!(!message.contains('\n'), "{text}: {message}");
        assert!(message.contains(" at line "), "{text}: {message}");
    }
}

#[test]
fn writes_block_style_quoting_only_where_needed() {
    let value = json!({
        "format": 4,
        "plain": "\\u{0} š é",
        "words": ["no", "y", "1.0", "", "~", ",", "'"],
        "escaped": "[\\u{09BF}-\\u{09C8}]",
        "rows": "a b c\nd e f\n",
        "control": "a\u{1}b",
        "~": {},
        "list": [],
        "none": null,
        "yes": true,
        "half": 0.5,
    });
    assert_eq!(
        to_string(&value).unwrap(),
        "format: 4\n\
         plain: \\u{0} š é\n\
         words:\n\
         - 'no'\n\
         - 'y'\n\
         - '1.0'\n\
         - ''\n\
         - '~'\n\
         - ','\n\
         - ''''\n\
         escaped: '[\\u{09BF}-\\u{09C8}]'\n\
         rows: |\n  a b c\n  d e f\n\
         control: \"a\\x01b\"\n\
         \"~\": {}\n\
         list: []\n\
         none: null\n\
         \"yes\": true\n\
         half: 0.5\n"
    );
}

#[test]
fn written_text_reads_back_as_written() {
    let value = json!({
        "no": "yes",
        " ": "\\u{20} ",
        "'": {"a: b": "a #b", "-": ["- x", "@", "`", "%", "!x"]},
        "long": "x ".repeat(80),
        "lines": "  indented\nlast",
        "trailing": "a\n\n",
    });
    assert_eq!(tree(&to_string(&value).unwrap()).unwrap(), value);
}

#[derive(Debug, Deserialize, PartialEq)]
struct Collections {
    map: BTreeMap<String, String>,
    list: Vec<String>,
    inner: Inner,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
struct Inner {
    a: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
enum Layer {
    #[serde(rename = "caps+shift")]
    CapsShift,
}

fn empty() -> Collections {
    Collections {
        map: BTreeMap::new(),
        list: Vec::new(),
        inner: Inner::default(),
    }
}

#[test]
fn tree_nulls_read_as_empty_collections() {
    for text in ["map:\nlist:\ninner:\n", "map: ~\nlist: ~\ninner: null\n"] {
        assert_eq!(
            from_tree::<Collections>(tree(text).unwrap()).unwrap(),
            empty()
        );
    }
}

#[test]
fn tree_structs_and_enums_keep_their_shapes() {
    let read = |text: &str| from_tree::<Collections>(tree(text).unwrap());
    let error = read("map: {}\nlist: []\ninner: [x]\n").unwrap_err();
    assert_eq!(error.path(), "inner");
    assert!(read("map: {}\nlist: {}\ninner: {}\n").is_err());
    assert!(read("map: {a: 1}\nlist: []\ninner: {}\n").is_err());

    let layer = |text: &str| from_tree::<BTreeMap<String, Layer>>(tree(text).unwrap());
    assert_eq!(layer("a: caps+shift\n").unwrap()["a"], Layer::CapsShift);
    assert!(layer("a: {caps+shift: x}\n").is_err());
}

#[derive(Debug, Deserialize, PartialEq)]
struct Defaulted {
    #[serde(deserialize_with = "empty_as_default")]
    map: BTreeMap<String, String>,
    #[serde(default, deserialize_with = "empty_as_default")]
    list: Vec<String>,
}

#[test]
fn empty_typed_fields_read_as_empty_collections() {
    for text in ["map:\nlist:\n", "map: ~\nlist: ~\n", "map:\n"] {
        let read: Defaulted = from_str(text).unwrap();
        assert!(read.map.is_empty() && read.list.is_empty(), "{text}");
    }
    let read: Defaulted = from_str("map: {a: b}\nlist: [c]\n").unwrap();
    assert_eq!(
        (read.map["a"].as_str(), read.list),
        ("b", vec!["c".to_string()])
    );
    assert!(from_str::<Defaulted>("list: []\n").is_err());
}
