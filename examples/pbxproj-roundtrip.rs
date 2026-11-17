use std::path::Path;

use kbdgen::build::ios::pbxproj::Pbxproj;

fn main() -> anyhow::Result<()> {
    let filename = std::env::args()
        .nth(1)
        .expect("usage: pbxproj-roundtrip <path to project.pbxproj>");

    let pbxproj = Pbxproj::from_path(Path::new(&filename))?;
    print!("{}", pbxproj.to_pbxproj_string());
    Ok(())
}
