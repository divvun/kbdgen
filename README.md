# kbdgen

[![](https://divvun-tc.thetc.se/api/github/v1/repository/divvun/kbdgen/main/badge.svg)](https://divvun-tc.thetc.se/api/github/v1/repository/divvun/kbdgen/main/latest)

A tool to build keyboard packages for a multitude of platforms using a single, simple text file definition. Supported outputs:

- Linux (X11, m17n)
- macOS
- Windows
- ChromeOS
- iOS/iPadOS
- Android
- SVG
- hit-error correction model as an [FST](https://en.wikipedia.org/wiki/Finite-state_machine), to be used with our spellers
- [CLDR keyboard definitions](https://cldr.unicode.org/index/keyboard-workgroup) is presently missing, but is in the pipeline

We think it's pretty cool.
[Documentation](https://divvun.github.io/kbdgen/) (under construction -- [how to update](DEV.md#asciidoc)).

## Installation

1. get [Rust](https://www.rust-lang.org/learn/get-started)
1. clone this repo: `git clone https://github.com/divvun/kbdgen.git`
2. `cd kbdgen`
1. `cargo install --path .` (this installs `kbdgen` to the path)

**Alternatively** - download a precompiled binary from nightly builds:

* [Linux  ](https://pahkat.uit.no/devtools/download/kbdgen?channel=nightly&platform=linux)   (x86_64)
* [macOS  ](https://pahkat.uit.no/devtools/download/kbdgen?channel=nightly&platform=macos)   (x86_64)
* [Windows](https://pahkat.uit.no/devtools/download/kbdgen?channel=nightly&platform=windows) (i686)

Extract the archive, and move the binary to somewhere on your `$PATH`.

## Example Usage

`cargo run -- target --bundle-path C:\Projects\Divvun\keyboards\keyboard-sme\sme.kbdgen --output-path C:\KbdgenBuilds\sme_mac macos generate`


Let's build an android app. There's two. [divvun-dev-keyboard](https://github.com/divvun/divvun-dev-keyboard) and [divvun-keyboard](https://github.com/divvun/divvun-keyboard). The former is the unstable one where we do all the testing. The latter one gets pushed to production. Both are deployed as separate apps in google playstore. 

Alas, I digress. 

We want to build divvun-dev-keyboard. Clone divvun-dev-keyboard. 

```bash
cd source/divvun
git clone git@github.com:divvun/divvun-dev-keyboard.git 
cd ../kbdgen
cargo run -- fetch -b /Users/srdkvr/source/divvun/divvun-dev-keyboard/divvun-dev.kbdgen #fetches deps
cargo run -- target --bundle-path /Users/srdkvr/source/divvun/divvun-dev-keyboard/divvun-dev.kbdgen --output-path ~/source/divvun/android_keyboard android build
```

Note - you will need imagemagick to run converting of images - so 

```
$ brew install imagemagick@6
$ echo 'export PATH="/opt/homebrew/opt/imagemagick@6/bin:$PATH"' >> ~/.zshrc
```

Your typical commands are

- ... `android clone` # Clones the base android repository
- ... `android generate` # Generates all the layouts and speller configs, but doesn't clone stuff
- ... `android build` # Both of the above. 


### TL;DR Android

Your 


For Android run two commands:

```
kbdgen target --bundle-path C:\Projects\Divvun\keyboards\keyboard-sme\sme.kbdgen --output-path C:\KbdgenBuilds\sme_android android clone
kbdgen target --bundle-path C:\Projects\Divvun\keyboards\keyboard-sme\sme.kbdgen --output-path C:\KbdgenBuilds\sme_android android generate
```

## License

This project is licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Fork and PR on Github.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

