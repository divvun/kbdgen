# kbdgen

[![](https://divvun-tc.thetc.se/api/github/v1/repository/divvun/kbdgen/main/badge.svg)](https://divvun-tc.thetc.se/api/github/v1/repository/divvun/kbdgen/main/latest)

A tool to build keyboard packages for a multitude of platforms using a single, simple text file definition. Supported outputs:

- Linux (X11)
- macOS
- Windows
- ChromeOS
- iOS/iPadOS
- Android
- SVG

We think it's pretty cool.
[Documentation](https://divvun.github.io/kbdgen/) (under construction).

## Installation

1. get [Rust](https://www.rust-lang.org/learn/get-started)
1. clone this repo: `git clone https://github.com/divvun/kbdgen.git`
2. `cd kbdgen`
1. `cargo install --path .`

**Alternatively** - download a precompiled binary from nightly builds:

* [Linux  ](https://pahkat.uit.no/devtools/download/kbdgen?channel=nightly&platform=linux)   (x86_64)
* [macOS  ](https://pahkat.uit.no/devtools/download/kbdgen?channel=nightly&platform=macos)   (x86_64)
* [Windows](https://pahkat.uit.no/devtools/download/kbdgen?channel=nightly&platform=windows) (i686)

Extract the archive, and move the binary to somewhere on your `$PATH`.

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
