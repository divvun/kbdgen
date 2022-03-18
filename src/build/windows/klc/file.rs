use core::fmt::Display;

use super::layout::KlcLayout;

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

        Ok(())
    }
}
