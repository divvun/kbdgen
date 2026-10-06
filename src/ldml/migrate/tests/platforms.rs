//! `ldml.migrate.platforms`, `ldml.migrate.dead-keys` and
//! `ldml.migrate.fields` on small v3 layouts.

use super::*;

fn layer_rows(m: &Migration, variant: &str, key: &str) -> Vec<Vec<String>> {
    let text = yaml(m);
    let value: serde_yaml::Value = serde_yaml::from_str(text).unwrap();
    let rows = value["hardware"][variant]["layers"][key]
        .as_str()
        .unwrap_or_else(|| panic!("no layer {key} in {variant}:\n{text}"));
    rows.lines()
        .map(|l| l.split(' ').map(str::to_string).collect())
        .collect()
}

fn value(m: &Migration) -> serde_yaml::Value {
    serde_yaml::from_str(yaml(m)).unwrap()
}

// [spec:kbdgen:sem:ldml.migrate.platforms/test]
#[test]
fn windows_layers_become_modifier_sets_in_iso_rows() {
    let m = migrate(&desktop(
        "windows",
        &[
            ("default", KEYS),
            ("shift", SHIFTED),
            ("caps+shift", KEYS),
            ("alt", KEYS),
            ("alt+shift", SHIFTED),
            ("alt+caps", SHIFTED),
            ("ctrl", KEYS),
        ],
        "",
    ));
    assert!(!m.blocked(), "{:?}", m.defects);
    let keys: Vec<String> = value(&m)["hardware"]["windows"]["layers"]
        .as_mapping()
        .unwrap()
        .keys()
        .map(|k| k.as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        keys,
        [
            "none",
            "shift",
            "caps shift",
            "altR",
            "altR shift",
            "altR caps",
            "ctrl"
        ]
    );
    let rows = layer_rows(&m, "windows", "none");
    let counts: Vec<usize> = rows.iter().map(Vec::len).collect();
    assert_eq!(counts, [13, 12, 12, 11]);
    assert_eq!(rows[0][0], "§");
    assert_eq!(
        rows[3],
        "< z x c v b n m , . -".split(' ').collect::<Vec<_>>()
    );
}

// [spec:kbdgen:sem:ldml.migrate.platforms/test]
#[test]
fn macos_alt_stays_alt_and_cmd_is_native() {
    let m = migrate(&desktop(
        "macOS",
        &[
            ("default", KEYS),
            ("alt", KEYS),
            ("alt+shift", SHIFTED),
            ("alt+caps", SHIFTED),
            ("cmd", KEYS),
            ("cmd+shift", SHIFTED),
            ("cmd+alt", KEYS),
            ("cmd+alt+shift", SHIFTED),
        ],
        "",
    ));
    let keys: Vec<String> = value(&m)["hardware"]["macOS"]["layers"]
        .as_mapping()
        .unwrap()
        .keys()
        .map(|k| k.as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        keys,
        [
            "none",
            "alt",
            "alt shift",
            "alt caps",
            "cmd",
            "cmd shift",
            "cmd alt",
            "cmd alt shift"
        ]
    );
    let chrome = migrate(&desktop(
        "chromeOS",
        &[("default", KEYS), ("alt", KEYS)],
        "",
    ));
    assert!(layer_rows(&chrome, "chromeOS", "altR").len() == 4);
}

// [spec:kbdgen:sem:ldml.migrate.platforms/test]
#[test]
fn macos_space_entries_become_variant_space() {
    let m = migrate(&format!(
        "{}  space:\n    alt: \\u{{A0}}\n    default: ' '\n",
        desktop("macOS", &[("default", KEYS), ("alt", KEYS)], "")
    ));
    let space = &value(&m)["hardware"]["macOS"]["space"];
    assert_eq!(space["alt"].as_str(), Some("\\u{A0}"));
    assert!(space.get("none").is_none(), "U+0020 is the default");
}

const IOS: &str = r#"iOS:
  config:
    spellerPath: se.bhfst
  primary:
    layers:
      default: |
        q w e
        \s{shift} a \s{backspace}
      shift: |
        Q W E
        \s{shift} A \s{backspace}
  iPad-9in:
    layers:
      default: |
        q w e \s{backspace}
        \s{shift:1.5} a s
      shift: |
        Q W E \s{backspace}
        \s{shift:1.5} A S
      alt: |
        1 2 3 \s{backspace}
        \s{shift:1.5} @ #
      symbols-1: |
        1 2 3
"#;

// [spec:kbdgen:sem:ldml.migrate.platforms/test]
#[test]
fn ios_platforms_become_sizes_with_south_flicks() {
    let m = migrate(IOS);
    assert!(!m.blocked(), "{:?}", m.defects);
    let v = value(&m);
    let sizes = &v["touch"]["iOS"]["sizes"];
    let names: Vec<&str> = sizes
        .as_mapping()
        .unwrap()
        .keys()
        .map(|k| k.as_str().unwrap())
        .collect();
    assert_eq!(names, ["phone", "tablet"]);
    assert_eq!(
        sizes["phone"]["layers"]["base"].as_str(),
        Some("q w e\n\\s{shift} a \\s{backspace}\n")
    );
    let tablet = &sizes["tablet"]["layers"];
    assert_eq!(
        tablet["base"]["flicks"]["s"].as_str(),
        Some("1 2 3 \\s{backspace}\n\\s{shift:1.5} @ #\n")
    );
    assert!(tablet.get("alt").is_none());
    assert_eq!(tablet["symbols-1"].as_str(), Some("1 2 3\n"));
    assert_eq!(
        v["targets"]["iOS"]["spellerPath"].as_str(),
        Some("se.bhfst")
    );
}

// [spec:kbdgen:sem:ldml.migrate.platforms/test]
#[test]
fn android_platforms_become_phone_and_tablet() {
    let m = migrate(
        "android:\n  primary:\n    layers:\n      default: a b\n      shift: A B\n  tablet-600:\n    layers:\n      default: a b c\n",
    );
    let sizes = &value(&m)["touch"]["android"]["sizes"];
    assert_eq!(sizes["phone"]["layers"]["shift"].as_str(), Some("A B\n"));
    assert_eq!(sizes["tablet"]["layers"]["base"].as_str(), Some("a b c\n"));
}

// [spec:kbdgen:sem:ldml.migrate.platforms/test]
#[test]
fn variants_follow_host_order_not_file_order() {
    let m = migrate(&format!(
        "android:\n  primary:\n    layers:\n      default: a\n  tablet-600:\n    layers:\n      default: a\n{}{}{IOS}",
        desktop("chromeOS", &[("default", KEYS)], ""),
        desktop("windows", &[("default", KEYS)], "")
    ));
    let v = value(&m);
    let names = |section: &str| -> Vec<String> {
        v[section]
            .as_mapping()
            .unwrap()
            .keys()
            .map(|k| k.as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names("hardware"), ["windows", "chromeOS"]);
    assert_eq!(names("touch"), ["iOS", "android"]);
}

const TRANSFORMS: &str = "transforms:\n  ´:\n    ' ': ´\n    a: á\n    e: é\n  ˆ:\n    ' ': '^'\n    a: â\n    ˇ:\n      e: ế\n";

// [spec:kbdgen:sem:ldml.migrate.dead-keys+1/test]
#[test]
fn transforms_become_dead_keys_with_compose() {
    let keys = KEYS.replace("+ ´", "ˆ ´");
    let m = migrate(&format!(
        "{}{TRANSFORMS}",
        desktop(
            "windows",
            &[("default", &keys)],
            "  deadKeys:\n    default: ['´', 'ˆ']\n"
        )
    ));
    assert!(!m.blocked(), "{:?}", m.defects);
    let v = value(&m);
    let dead = &v["deadKeys"];
    assert!(
        dead["´"].get("standalone").is_none(),
        "equal to the identity"
    );
    let compose: Vec<(&str, &str)> = dead["´"]["compose"]
        .as_mapping()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.as_str().unwrap(), v.as_str().unwrap()))
        .collect();
    assert_eq!(compose, [("a", "á"), ("e", "é")]);
    assert_eq!(dead["ˆ"]["standalone"].as_str(), Some("^"));
    assert_eq!(
        dead["ˆ"]["compose"]["ˇ"]["compose"]["e"].as_str(),
        Some("ế")
    );
    assert_eq!(
        layer_rows(&m, "windows", "none")[0][11..],
        ["\\d{ˆ}", "\\d{´}"]
    );
}

// [spec:kbdgen:sem:ldml.migrate.dead-keys+1/test]
#[test]
fn dead_keys_follow_each_platform_rule() {
    let keys = KEYS.replace("+ ´", "´ ´");
    let windows = desktop(
        "windows",
        &[("default", &keys), ("shift", &keys)],
        "  deadKeys:\n    shift: ['´']\n",
    );
    let mac_keys = KEYS.replace("+ ´", "+ \\u{B4}");
    let mac = desktop(
        "macOS",
        &[("default", &mac_keys)],
        "  deadKeys:\n    default: ['\\u{B4}']\n",
    );
    let android = "android:\n  primary:\n    layers:\n      default: a ´\n  tablet-600:\n    layers:\n      default: ´\n";
    let m = migrate(&format!(
        "{windows}{mac}{android}transforms:\n  ´:\n    a: á\n"
    ));
    assert!(!m.blocked(), "{:?}", m.defects);
    assert_eq!(layer_rows(&m, "windows", "none")[0][11], "´", "not listed");
    assert_eq!(
        layer_rows(&m, "windows", "shift")[0][11..],
        ["\\d{´}", "\\d{´}"]
    );
    assert_eq!(
        layer_rows(&m, "macOS", "none")[0][12],
        "\\d{´}",
        "entries compare decoded"
    );
    let touch = &value(&m)["touch"]["android"]["sizes"];
    assert_eq!(
        touch["phone"]["layers"]["base"].as_str(),
        Some("a \\d{´}\n")
    );
    assert_eq!(touch["tablet"]["layers"]["base"].as_str(), Some("\\d{´}\n"));
}

// [spec:kbdgen:sem:ldml.migrate.fields+1/test]
#[test]
fn fields_carry_over_with_escapes_decoded() {
    let m = migrate(&format!(
        "decimal: ','\nkeyNames:\n  space: '\\u{{20}}space'\n  return: enter\nlongpress:\n  a: á  à\n  'n': ń\n{}  config:\n    locale: se-Latn-NO\n    id: SE01\n{}  config:\n    xkbLayout: 'no'\n",
        desktop("windows", &[("default", KEYS)], ""),
        desktop("chromeOS", &[("default", KEYS)], "")
    ));
    assert!(!m.blocked(), "{:?}", m.defects);
    let v = value(&m);
    assert_eq!(v["displayNames"]["se"].as_str(), Some("Davvisámegiella"));
    assert_eq!(v["decimal"].as_str(), Some(","));
    assert_eq!(v["keyNames"]["space"].as_str(), Some("\\u{20}space"));
    assert_eq!(v["longPress"]["a"].as_str(), Some("á à"));
    assert_eq!(v["longPress"]["n"].as_str(), Some("ń"));
    assert_eq!(v["targets"]["windows"]["id"].as_str(), Some("SE01"));
    assert_eq!(v["targets"]["chromeOS"]["xkbLayout"].as_str(), Some("no"));
    let layout = m.layout.as_ref().unwrap();
    assert_eq!(layout.space_label.as_deref(), Some(" space"));
    assert!(v.get("impliedLayers").is_none() && v.get("normalization").is_none());
}

// [spec:kbdgen:sem:ldml.migrate.fields+1/test]
#[test]
fn windows_keyboard_options_v3_never_read_are_dropped() {
    let m = migrate(&format!(
        "{}  config:\n    id: SE01\n    shiftLock: true\n    lrmRlm: true\n    keyNames:\n      Esc: Échap\n",
        desktop("windows", &[("default", KEYS)], "")
    ));
    assert!(!m.blocked(), "{:?}", m.defects);
    let v = value(&m);
    let windows = v["targets"]["windows"].as_mapping().unwrap();
    assert_eq!(windows.len(), 1, "{windows:?}");
    assert_eq!(v["targets"]["windows"]["id"].as_str(), Some("SE01"));
    let paths: Vec<&str> = with_code(&m, Code::M10)
        .iter()
        .map(|d| d.path.as_str())
        .collect();
    assert_eq!(
        paths,
        [
            "windows.config.shiftLock",
            "windows.config.lrmRlm",
            "windows.config.keyNames"
        ]
    );
}

// [spec:kbdgen:sem:ldml.migrate.fields+1/test]
#[test]
fn null_config_is_omitted_silently() {
    let m = migrate(&format!(
        "{}  config:\n#    locale: se\n",
        desktop("windows", &[("default", KEYS)], "")
    ));
    assert!(value(&m).get("targets").is_none());
    assert_eq!(
        codes_besides_caps(&m),
        [Code::M11],
        "only the comment is reported"
    );
}

// [spec:kbdgen:sem:ldml.migrate.fields+1/test]
#[test]
fn no_key_tokens_stay_no_key() {
    let keys = KEYS.replacen("§", "\\u{0}", 1);
    let m = migrate(&desktop("windows", &[("default", &keys)], ""));
    assert_eq!(layer_rows(&m, "windows", "none")[0][0], "\\u{0}");
    assert!(codes_besides_caps(&m).is_empty(), "{:?}", m.defects);
}
