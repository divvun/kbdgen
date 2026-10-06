//! The four COM exports draw LNK4104, which asks for them to be `PRIVATE`
//! so that they stay out of the import library. Nothing imports the text
//! service, so the warning carries no information.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo::rustc-cdylib-link-arg=/IGNORE:4104");
    }
}
