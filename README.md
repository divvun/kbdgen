# kbdgen

kbdgen builds keyboard layouts for Windows, macOS, ChromeOS, Linux, iOS and
Android from one source: a `.kbdgen` bundle, which is a directory of YAML files
describing the layouts, plus per-target metadata and resources such as icons.
It also converts layouts to and from CLDR LDML Keyboard 3.0 XML, and runs
typing tests against kbdgen's keyboard engine.

Documentation: <https://divvun.github.io/kbdgen/> (the user guide and the
developer guide). The normative specification is in [`docs/spec/`](docs/spec/).

## Outputs

| Target | Command | Output |
|---|---|---|
| Windows | `kbdgen target … windows` | keyboard layout DLLs for x86, x64, arm64 and WOW64, carrying the engine model that the Divvun text service runs |
| macOS | `kbdgen target … macos build` | a keyboard layout bundle of `.keylayout` files, and a `.pkg` installer |
| ChromeOS | `kbdgen target … chromeos` | an input method extension that runs kbdgen's engine compiled to WebAssembly |
| Linux | `kbdgen target … linux` | XKB layouts; data for the Divvun IBus engine, which runs kbdgen's engine; and on-screen keyboard layouts for GNOME Shell, plasma-keyboard and Stevia |
| iOS and iPadOS | `kbdgen target … ios build` | a keyboard app built from [giellakbd-ios](https://github.com/divvun/giellakbd-ios) |
| Android | `kbdgen target … android build` | a keyboard app built from [giellakbd-android](https://github.com/divvun/giellakbd-android) |
| CLDR LDML | `kbdgen ldml export`, `kbdgen ldml import` | LDML Keyboard 3.0 XML, one file per layout and host |

Layouts come in two formats. Format 4 (a layout file with `format: 4`) is
LDML-shaped and compiles to kbdgen's keyboard model; format 3 is the older
format. The Windows, macOS, ChromeOS and Linux targets build both. iOS and
Android build format 3 only, and refuse a bundle that has a format 4 layout.
`kbdgen ldml migrate` converts format 3 layouts to format 4.

`kbdgen tsf` builds the Windows text service, which is released separately
from keyboards. The Linux IBus engine, `ibus-engine-divvun`, is released
separately too; it is the crate `crates/kbd-ibus`.

## Install

kbdgen is built from source:

1. Install Rust with [rustup](https://rustup.rs).
2. Add the WebAssembly target. kbdgen's build script compiles the keyboard
   engine for it and embeds the module in every ChromeOS extension:

   ```sh
   rustup target add wasm32-unknown-unknown
   ```

3. Clone and install:

   ```sh
   git clone https://github.com/divvun/kbdgen.git
   cd kbdgen
   cargo install --path .
   ```

Each target also needs some tools of its own:

| Target | Needs |
|---|---|
| Windows | The three Windows targets: `rustup target add i686-pc-windows-msvc x86_64-pc-windows-msvc aarch64-pc-windows-msvc`. kbdgen builds the layout DLLs with `cargo` and links them with the `rust-lld` that rustup ships, so this works on macOS and Linux too. |
| macOS | ImageMagick for icons. Icons (`iconutil`) and the installer (`pkgbuild`, `productbuild`) need macOS; on other hosts `macos build` skips both with a warning. |
| ChromeOS | Nothing more. |
| Linux | Nothing more. The IBus engine the keyboards run on is built on Linux with `cargo build -p kbd-ibus --release`. |
| iOS | macOS with Xcode, CocoaPods, fastlane, xcbeautify, git and ImageMagick. |
| Android | git, tar, and ImageMagick for the icon. When the bundle has `targets/android.yaml`, kbdgen also runs `gradlew assembleRelease`, which needs a JDK and the Android SDK. |
| `kbdgen tsf` | Windows, with the MSVC build tools, the Windows SDK and the three Windows targets above. |

kbdgen runs ImageMagick as `convert`. ImageMagick 7 still installs that
command (it prints a deprecation notice, which is harmless). On macOS:
`brew install imagemagick`.

## Quick start

Build a published keyboard, North Sámi, for Windows and macOS:

```sh
git clone https://github.com/giellalt/keyboard-sme
kbdgen target -b keyboard-sme/sme.kbdgen -o out/windows windows
kbdgen target -b keyboard-sme/sme.kbdgen -o out/macos macos build
```

`out/windows/x64/kbdse-NO.dll` is one of the resulting layout DLLs, and
`out/macos` holds the `.bundle` and its `.pkg` installer.

Those layouts are in format 3. To see what converting them to format 4 would
report, without writing anything:

```sh
kbdgen ldml migrate -b keyboard-sme/sme.kbdgen --dry-run
```

`kbdgen --help` lists every command, and each command takes `--help` too.
Log lines go to stderr; set `RUST_LOG=debug` for more detail.

## Contributing

See [DEV.md](DEV.md) for building, testing and regenerating the
documentation, and the developer guide for how the code fits together.

## License

This project is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
