//! Writes the spike's OSK layout as a GResource bundle with the `gvdb`
//! crate, as `linux.osk.gnome` says kbdgen will: one uncompressed file,
//! `/org/gnome/shell/osk-layouts/test-se.json`.
use std::borrow::Cow;

use gvdb::gresource::{BundleBuilder, FileData, PreprocessOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (json, out) = (args.next().ok_or("json path")?, args.next().ok_or("out path")?);
    let data = std::fs::read(&json)?;
    let file = FileData::new(
        "/org/gnome/shell/osk-layouts/test-se.json".to_string(),
        Cow::Owned(data),
        None,
        false,
        &PreprocessOptions::empty(),
    )?;
    let bytes = BundleBuilder::from_file_data(vec![file]).build()?;
    std::fs::write(&out, &bytes)?;
    println!("wrote {} bytes to {out}", bytes.len());
    Ok(())
}
