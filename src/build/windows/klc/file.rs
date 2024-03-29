use core::fmt::Display;

use super::{dead_key::KlcDeadKeys, layout::KlcLayout, ligature::KlcLigature};

pub const KLC_EXT: &str = "klc";

pub struct KlcFile<'a> {
    pub metadata: KlcFileMetadata,
    pub layout: KlcLayout,
    pub ligature: KlcLigature,
    pub dead_keys: KlcDeadKeys<'a>,
}

pub struct KlcFileMetadata {
    pub keyboard_name: String,
    pub description: String,
    pub copyright: String,
    pub company: String,
    pub autonym: String,
    pub locale_id: u32,
    pub locale_name: String,
}

impl Display for KlcFile<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "KBD\t{}\t\"{}\"\n\n",
            self.metadata.keyboard_name, self.metadata.description
        ))?;

        f.write_fmt(format_args!(
            "COPYRIGHT\t\"{}\"\n\n",
            self.metadata.copyright
        ))?;
        f.write_fmt(format_args!("COMPANY\t\"{}\"\n\n", self.metadata.company))?;

        f.write_fmt(format_args!(
            "LOCALENAME\t\"{}\"\n\n",
            self.metadata.locale_name
        ))?;

        f.write_fmt(format_args!(
            "LOCALEID\t\"{:08x}\"\n\n",
            self.metadata.locale_id
        ))?;

        f.write_str("VERSION\t1.0\n\n")?;

        f.write_str(&self.layout.to_string())?;

        f.write_str(&self.ligature.to_string())?;

        f.write_str(&self.dead_keys.to_string())?;

        f.write_str(FOOTER_CONTENT)?;

        f.write_str("\nDESCRIPTIONS\n\n")?;
        f.write_fmt(format_args!(
            "{:04x}\t{}\n\n",
            self.metadata.locale_id, self.metadata.description
        ))?;

        f.write_str("LANGUAGENAMES\n\n")?;
        f.write_fmt(format_args!(
            "{:04x}\t{}\n\n",
            self.metadata.locale_id, self.metadata.autonym
        ))?;

        f.write_str("ENDKBD\n")?;

        Ok(())
    }
}

const FOOTER_CONTENT: &str = r#"
KEYNAME

01	Esc
0e	Backspace
0f	Tab
1c	Enter
1d	Ctrl
2a	Shift
36	"Right Shift"
37	"Num *"
38	Alt
39	Space
3a	"Caps Lock"
3b	F1
3c	F2
3d	F3
3e	F4
3f	F5
40	F6
41	F7
42	F8
43	F9
44	F10
45	Pause
46	"Scroll Lock"
47	"Num 7"
48	"Num 8"
49	"Num 9"
4a	"Num -"
4b	"Num 4"
4c	"Num 5"
4d	"Num 6"
4e	"Num +"
4f	"Num 1"
50	"Num 2"
51	"Num 3"
52	"Num 0"
53	"Num Del"
54	"Sys Req"
57	F11
58	F12
7c	F13
7d	F14
7e	F15
7f	F16
80	F17
81	F18
82	F19
83	F20
84	F21
85	F22
86	F23
87	F24

KEYNAME_EXT

1c	"Num Enter"
1d	"Right Ctrl"
35	"Num /"
37	"Prnt Scrn"
38	"Right Alt"
45	"Num Lock"
46	Break
47	Home
48	Up
49	"Page Up"
4b	Left
4d	Right
4f	End
50	Down
51	"Page Down"
52	Insert
53	Delete
54	<00>
56	Help
5b	"Left Windows"
5c	"Right Windows"
5d	Application
"#;
