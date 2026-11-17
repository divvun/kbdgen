# ChromeOS

`target chromeos` writes a Chrome input-method extension, Manifest V3,
to the output directory. Its service worker runs `kbd-engine`, compiled
to WebAssembly, on each layout's compiled `chromeOS` keyboard; which
layouts those are, and how the service worker types, is
`docs/spec/ldml/chromeos.md`. The extension is loaded unpacked on a
Chromebook, or packed and published from the output directory as it is.

> [spec:kbdgen:def:chromeos.extension]
> The extension is these files, written in this order once every layout
> has compiled and the manifest checks have passed, so that a failure
> writes nothing:
>
> - `background.js`, the service worker, the same for every bundle
> - `engine.wasm`, the engine module (`chromeos.engine`)
> - `models/{tag}.dvkb` for each layout (`ldml.chromeos.input`), named by
>   its normalised tag, which is also its input component's id
> - `manifest.json` (`chromeos.manifest`)
> - `_locales/{k}/messages.json` (`chromeos.locales`)
> - `icon.png` (`chromeos.icons`), when the bundle has one
>
> `models/` and `_locales/` are emptied first, and a stale `icon.png` is
> removed, so no layout, language or icon of an earlier run remains. The
> step MUST fail, before writing, with `targets/chromeos.yaml is missing`
> if that file is absent, then with `project.yaml: locales has no en
> entry`, then with any error of `ldml.chromeos.input`, then with `no
> layout has a chromeOS keyboard` when there are no layouts.

> [spec:kbdgen:req:chromeos.engine]
> `engine.wasm` MUST be `crates/kbd-wasm` (`ldml.chromeos.wasm`) built for
> `wasm32-unknown-unknown` by kbdgen's own build script from the same
> workspace, and embedded in the kbdgen binary. So a released kbdgen
> generates an extension with no Rust toolchain, and the engine always
> reads the model encoding that this kbdgen writes. The build script runs
> a nested `cargo build --release --locked` with its own target directory
> under `OUT_DIR`, to stay clear of the outer build's lock, with release
> settings for size given on its command line, and without the outer
> build's rustc flags and workspace wrapper, which belong to the host
> triple and to clippy. It reruns only when `crates/kbd-wasm`, the
> sources and manifests of `kbd-engine` and `kbd-model`, or `Cargo.lock`
> change. Building kbdgen therefore needs the `wasm32-unknown-unknown`
> target; without it the build fails saying how to add it. No built
> module is checked in.

> [spec:kbdgen:req:chromeos.manifest+2]
> `manifest.json` MUST be pretty JSON with:
>
> - `manifest_version` 3, `name` `__MSG_name__`, `description`
>   `__MSG_description__` and `default_locale` `en`
> - `version` from `build` and `version_name` from `version`; `appId` is
>   unused. A `build` that is not one to four dot-separated integers from
>   0 to 65535 without leading zeros, which Chrome refuses, is an error
>   naming `targets/chromeos.yaml` and the value
> - `background.service_worker` `background.js` and `permissions`
>   `["input"]`
> - `content_security_policy.extension_pages`
>   `script-src 'self' 'wasm-unsafe-eval'; object-src 'self';`, Chrome's
>   minimum policy with WebAssembly allowed, which Manifest V3 forbids by
>   default
> - one input component per layout, in layout order:
>   `{"name": "__MSG_{tag_}__", "id": tag, "language": language, "layouts": [xkb layout]}`
>   (`ldml.chromeos.input`)
> - `icons` `{"16", "48", "128"}`, each `icon.png`, only when the
>   extension has an icon
>
> In these rules, `x_` means `x` with `-`→`_`.

> [spec:kbdgen:req:chromeos.locales]
> kbdgen writes `_locales/{k_}/messages.json` for `en` and for each
> language `k` in a layout's display names, in that order. It has one
> `{tag_: {"message": T}}` entry per layout that has a name for `k`, then
> `name` and `description` from `locales[k]`, falling back to `locales.en`.
> Chrome needs every message in the default locale, so in `en` a layout
> without an English name takes its autonym, else its first name. `T`
> substitutes the name into `{} tastatur` (`nb`, `no`, `nn`, `da`),
> `{} tangentbord` (`sv`), `{} näppäimistö` (`fi`), or `{} keyboard` (all
> other tags).

> [spec:kbdgen:req:chromeos.icons+1]
> The extension's icon is the default icon of the bundle's ChromeOS
> resources (`icon.<ext>`, `bundle.resources`), copied as `icon.png`; Chrome
> scales the one PNG to each size the manifest names. A default icon that
> is not a `.png` is an error naming it. Per-layout icons are not used,
> since an input component has no icon of its own. Without one, the
> extension has no icon, which kbdgen reports as information.
