# Developing kbdgen

A quick start for contributors. The developer guide
(`docs/dev/`, published at <https://divvun.github.io/kbdgen/dev/kbdgen.html>)
explains the architecture, the build pipeline and the tests in depth, and
`docs/spec/` holds the specification the code is checked against.

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo build
```

kbdgen's `build.rs` compiles `crates/kbd-wasm` for `wasm32-unknown-unknown`
in a nested cargo build and embeds the module in the ChromeOS target, so the
build fails without that target.

`cargo build` builds the `default-members`: kbdgen, `kbd-model`, `kbd-engine`
and `kbd-ldml`. Add `--workspace` to include `kbd-tsf`, `kbd-wasm` and
`kbd-ibus`.

Run kbdgen from the checkout with `cargo run --`, for example on the test
bundle in the repository:

```sh
cargo run -- target -b crates/kbd-tsf/tests/acceptance/fixture.kbdgen -o /tmp/kbdgen-out windows
cargo run -- ldml compile -b crates/kbd-tsf/tests/acceptance/fixture.kbdgen -o /tmp/kbdgen-models
```

The Windows target needs the three Windows standard libraries, on any host:

```sh
rustup target add i686-pc-windows-msvc x86_64-pc-windows-msvc aarch64-pc-windows-msvc
```

Set `RUST_LOG=debug` (or `trace`) to see more of what a build does.

## Test

```sh
cargo install cargo-nextest --locked
cargo nextest run --workspace
```

`--workspace` matters: without it `kbd-tsf`, `kbd-wasm` and `kbd-ibus` are not
tested.

- `KBDGEN_BLESS=1 cargo nextest run --workspace` rewrites the golden files
  of the Windows, macOS, ChromeOS and Linux generators. Review the diff
  before committing it.
- The Linux tests against libxkbcommon and ibus-daemon run only where those
  are installed, and otherwise print a notice and pass, unless
  `KBDGEN_REQUIRE_LINUX_TESTS` is set. The developer guide's Linux chapter
  shows how to run them in Docker.
- The ChromeOS typing test drives the generated extension in Deno. Without
  `deno` on `PATH` (or `DENO` naming it) it prints a notice and passes.
- `cargo nextest run --workspace --run-ignored all` adds the slow and
  tool-dependent tests: Windows DLL builds, the device-triple builds of
  `kbd-model` and `kbd-engine`, and Keyman's `kmc` (needs Node.js).
- The Windows text service tests in `crates/kbd-tsf/tests/vm.rs` and
  `acceptance.rs` are compiled only on Windows and need a Windows 11
  machine with a signed-in session; their file headers say how to run
  them. On Windows, select tests by name rather than with
  `--run-ignored all`, which would pick these up too.

`nplan check` runs what a commit is gated on: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets` and `cargo nextest run --workspace`.
Commit messages use Conventional Commits prefixes (`feat:`, `fix:`, `docs:`
and so on).

## Documentation

The guides are AsciiDoc. Regenerate the HTML after changing them:

```sh
asciidoctor -a last-update-label! docs/index.adoc
asciidoctor -a last-update-label! docs/user/kbdgen.adoc
asciidoctor -a last-update-label! docs/dev/kbdgen.adoc
```

Each writes its `.html` beside the source, which is what
<https://divvun.github.io/kbdgen/> serves. `-a last-update-label!` keeps the
"Last updated" line out, so regenerating unchanged sources changes nothing.
