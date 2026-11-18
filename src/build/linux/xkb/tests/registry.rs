use super::*;
use crate::build::linux::xkb::{RegistryEntry, Tree, evdev_xml};

const ROWS: &str = "§ 1 2 3 4 5 6 7 8 9 0 + ´\nq w e r t y u i o p å ¨\na s d f g h j k l ö æ '\n< z x c v b n m , . -";

// [spec:kbdgen:syn:linux.xkb.registry/test]
#[test]
fn registry_lists_each_layout_in_bundle_order() {
    let se = fixture_text("crates/kbd-engine/tests/golden/v3/se-NO.yaml");
    let es = v4_linux("es", "Español & <Latinoamérica>", "", ROWS);
    let sme = v4_linux("sme", "Davvisámegiella", "", ROWS);
    let layouts = linux_layouts(&[("sme", &sme), ("se-NO", &se), ("es-419", &es)]);
    let tree = Tree::new(&ids(), &layouts).unwrap();
    let [(path, xml)] = &tree.files[..] else {
        panic!("{:?}", tree.files);
    };
    assert_eq!(path, "rules/evdev.xml");
    assert_eq!(
        xml,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<xkbConfigRegistry version=\"1.1\">
  <layoutList>
    <layout>
      <configItem>
        <name>divvun-es-419</name>
        <shortDescription>es</shortDescription>
        <description>Español &amp; &lt;Latinoamérica&gt;</description>
        <languageList>
          <iso639Id>spa</iso639Id>
        </languageList>
      </configItem>
    </layout>
    <layout>
      <configItem>
        <name>divvun-se-no</name>
        <shortDescription>se</shortDescription>
        <description>Davvisámegiella (Norga)</description>
        <countryList>
          <iso3166Id>NO</iso3166Id>
        </countryList>
        <languageList>
          <iso639Id>sme</iso639Id>
        </languageList>
      </configItem>
    </layout>
    <layout>
      <configItem>
        <name>divvun-sme</name>
        <shortDescription>sme</shortDescription>
        <description>Davvisámegiella</description>
        <languageList>
          <iso639Id>sme</iso639Id>
        </languageList>
      </configItem>
    </layout>
  </layoutList>
</xkbConfigRegistry>
"
    );
}

// [spec:kbdgen:syn:linux.xkb.registry/test]
#[test]
fn languages_without_a_code_are_omitted() {
    let entry = RegistryEntry {
        name: "divvun-qaa".to_owned(),
        short_description: "qaa".to_owned(),
        description: "Private".to_owned(),
        country: None,
        language: None,
    };
    let xml = evdev_xml(&[entry]);
    assert!(!xml.contains("languageList"), "{xml}");
    assert!(!xml.contains("countryList"), "{xml}");
    let layouts = linux_layouts(&[("qaa", &v4_linux("qaa", "Private", "", ROWS))]);
    let entry = RegistryEntry::new(&ids(), &layouts[0]).unwrap();
    assert_eq!(entry.language, None);
}
