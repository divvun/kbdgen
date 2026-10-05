# Crates and workspace

Everything lives in the kbdgen repository as one Cargo workspace:

- the keyboard model
- the engine
- LDML XML support
- the Windows text service

What runs on a device is kept small. Devices embed the engine in many
places: the Windows TSF DLL, a macOS IMKit app, iOS keyboard extensions with
tight memory limits, Android apps, a wasm module, and IBus or Fcitx engines.
So the device carries only the model, its decoder and the engine, all
`no_std`. XML (through `xmlem`), YAML and parsing of LDML's syntaxes happen
once, at build time, in std crates.

Sources: `Cargo.toml`; `docs/spec/kbdl.md` `kbdl.crate` (generated layout
crates opt out of any enclosing workspace); `docs/spec/tsf.md`
`tsf.component.crate`, `tsf.arch.builds`, `tsf.engine.api`,
`tsf.engine.contract`, `tsf.data.version`.

## Workspace

> [spec:kbdgen:def:ldml.crate.layout+1]
> The root `Cargo.toml` keeps the `kbdgen` package and adds a `[workspace]`
> with:
>
> - `members = ["crates/*"]`
> - `default-members = [".", "crates/kbd-model", "crates/kbd-engine", "crates/kbd-ldml"]`
> - `resolver = "3"`
>
> Every member uses edition 2024:
>
> | Path | Package | Kind | Holds |
> |---|---|---|---|
> | `crates/kbd-model` | `kbd-model` | `no_std` + `alloc` lib | the superset model, its invariants and its binary encoding (`ldml.model.*`) |
> | `crates/kbd-engine` | `kbd-engine` | `no_std` + `alloc` lib | the engine (`ldml.engine.*`), which the text service runs (`tsf.engine.api`) |
> | `crates/kbd-ldml` | `kbd-ldml` | std lib | LDML keyboard3 source documents through `xmlem`, the embedded CLDR imports, parsers for LDML's syntaxes, resolution to the model, and export (`ldml.xml.*`) |
> | `crates/kbd-tsf` | `kbd-tsf` | `cdylib` | the Windows text service (`docs/spec/tsf.md`), built only per `tsf.arch.builds` |
>
> The `kbdgen` package keeps the v4 YAML format and its lowering, the
> migrator, the `kbdl` adapter and the `kbdgen ldml` commands. Layout crates
> generated per `kbdl.crate` keep their own empty `[workspace]`, so this
> workspace never captures them.

> [spec:kbdgen:req:ldml.crate.model]
> `kbd-model` MUST be `#![no_std]` with `extern crate alloc`. Its only
> dependencies are:
>
> - `serde`, with `default-features = false` and the `derive` and `alloc`
>   features
> - `postcard`, with `default-features = false` and the `alloc` feature
>
> It MUST NOT use `unsafe`, floating point, hash maps, or anything else
> whose iteration order or encoding varies by platform
> (`ldml.model.deterministic`).

> [spec:kbdgen:req:ldml.crate.engine+1]
> `kbd-engine` MUST be `#![no_std]` with `extern crate alloc`. Its only
> dependencies are:
>
> - `kbd-model`
> - `icu_normalizer` 2.x, with `default-features = false` and
>   `compiled_data`, behind the default feature `normalization`
>
> Without `normalization` it MUST refuse a model whose normalization is
> enabled, returning an error. It MUST NOT use `unsafe`, I/O, clocks,
> randomness, threads or global mutable state, and its public types MUST be
> `Send` and `Sync`. It is the only engine: every host, the text service
> included (`tsf.engine.api`), runs it on the model of
> `ldml.model.encoding`.

> [spec:kbdgen:req:ldml.crate.ldml]
> `kbd-ldml` MUST read and write XML only through the `xmlem` crate, at the
> version the workspace already uses (0.3.3 at the time of writing). It MUST
> NOT depend on, or call directly, any other XML parser or serializer;
> `xmlem` brings `quick-xml` with it. It depends on:
>
> - `kbd-model`
> - `icu_normalizer`, for NFD of keyboard data
> - `xmlem`
>
> It embeds the CLDR import files (`ldml.xml.cldr-data`). It has its own
> parser for the regex-like syntax, because the `regex` crate can express
> neither markers, mapped sets nor LDML's restrictions.

> [spec:kbdgen:req:ldml.crate.tsf+1]
> `kbd-tsf` is the text service of `tsf.component.crate`. It depends on
> `kbd-engine` and `kbd-model`, and on `windows` and `windows-core`
> 0.62.x. It calls `kbd_engine::Model::from_bytes` on the resource of
> `tsf.data.resource`. It is excluded from `default-members` and is built
> only for the triples of `tsf.arch.builds`. So a plain `cargo build` on
> macOS or Linux never compiles it. The text service is released,
> versioned and installed separately from keyboards (`tsf.component`), but
> its source is this crate.

> [spec:kbdgen:req:ldml.crate.targets]
> `kbd-model` and `kbd-engine`, with default features, MUST build without std
> for these triples:
>
> - `wasm32-unknown-unknown`
> - `x86_64-pc-windows-msvc`, `i686-pc-windows-msvc` and `aarch64-pc-windows-msvc`
> - `aarch64-apple-darwin` and `x86_64-apple-darwin`
> - `aarch64-apple-ios`
> - `aarch64-linux-android`
> - `x86_64-unknown-linux-gnu`
>
> Neither crate may define a global allocator or a panic handler; the host
> binary provides them. CI MUST at least build both crates for
> `wasm32-unknown-unknown`.

> [spec:kbdgen:req:ldml.crate.ffi]
> Host bindings other than `kbd-tsf` are separate crates owned by the host
> work, and are deferred: a C ABI for Swift and IMKit, JNI for Android, and
> wasm-bindgen for the web and ChromeOS. To keep them thin, the engine API
> (`ldml.engine.api`) MUST:
>
> - use only plain data: integers, `bool`, `&str`, `String`, `Vec`, and
>   enums without generics
> - take no callbacks and no trait objects
> - return an error for every failure, never panic
> - keep all mutable state in the `State` value, which the host owns

> [spec:kbdgen:req:ldml.crate.kbdgen]
> The `kbdgen` package MUST depend on `kbd-model`, `kbd-engine` and `kbd-ldml`
> by path. The `kbdl` adapter derives dead-key tables by running the engine
> (`ldml.kbdl.dead-tree`). As a result, the native table fallback and the
> engine cannot disagree on what a dead key composes to.
