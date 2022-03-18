use core::fmt::Display;

use super::key::KlcKey;

pub struct KlcFile {
    pub keyboard_name: String,
    pub copyright: String,
    pub company: String,
    pub layout: KlcLayout,
}

impl Display for KlcFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("COPYRIGHT\t\"{}\"\n\n", self.copyright))?;
        f.write_fmt(format_args!("COMPANY\t\"{}\"\n\n", self.company))?;

        f.write_str("VERSION\t1.0\n\n")?;

        f.write_str(&self.layout.to_string())?;
        /*
        for row in &self.rows {
            f.write_str(&row.to_string())?;
        }
        */

        /*
          f.write_fmt(format_args!(
            "KBD\t{}\t\"{}\"\n\n",
            self.kbd, self.description
        ))?;
        f.write_fmt(format_args!("COPYRIGHT\t\"{}\"\n\n", self.copyright))?;
        f.write_fmt(format_args!("COMPANY\t\"{}\"\n\n", self.company))?;
        f.write_fmt(format_args!("LOCALENAME\t\"{}\"\n\n", self.locale_name))?;
        f.write_fmt(format_args!("LOCALEID\t\"{:08x}\"\n\n", self.locale_id))?;
        f.write_str("VERSION\t1.0\n\n")?;
        */

        /*
             let description = layout.native_name(tag.as_str()).unwrap();
        let copyright = bundle.project.copyright.to_string();
        let company = bundle.project.organisation.to_string();
        let kbd = format!(
            "kbd{}",
            target
                .and_then(|t| t.id.as_ref())
                .map(|x| x.to_string())
                .unwrap_or_else(|| tag.as_str().chars().take(5).collect::<String>())
        );
            */

        Ok(())
    }
}

pub struct KlcLayout {
    pub rows: Vec<KlcRow>,
}

impl Display for KlcLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SHIFTSTATE\n\n")?;
        f.write_str("0 // 4\n")?;
        f.write_str("1 // 5 Shift\n")?;
        f.write_str("2 // 6\n")?;
        f.write_str("6 // 7\n")?;
        f.write_str("7 // 8\n\n")?;

        //f.write_str("SHIFTSTATE\n\n0\n1\n2\n6\n7\n\n")?;
        f.write_str("LAYOUT\n\n")?;

        for row in &self.rows {
            f.write_str(&row.to_string())?;
        }

        Ok(())
    }
}

pub struct KlcRow {
    pub scancode: String,
    pub virtual_key: String,
    pub caps_mode: i8,
    pub default_key: KlcKey,
    pub shift_key: KlcKey,
}

impl Display for KlcRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{}\t{}\t{}\t{}\t{}\n",
            &self.scancode, &self.virtual_key, &self.caps_mode, &self.default_key, &self.shift_key
        ))?;

        Ok(())
    }
}

// sections

// 1. Preliminary text (keyboard name, copyright, etc.)

// 2. Shiftstate (??)

// 3. 0-1-2-6-7

// 4. LAYOUT
// - columns here. Involves ligatures

// 5. LIGATURE

// 6. DEADKEY

// 7. Random numbers? or these more DEADKEY?

// 8. KEYNAME because keyboard loves you

// 9.
