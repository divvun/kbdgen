# ChromeOS

`target chromeos` writes a Chrome input-method extension to the output
directory. It contains `background.js`, `manifest.json` and
`_locales/*/messages.json`. Layouts without a `chromeOS` section are
ignored, and the rest are visited in bundle layout order.

> [spec:kbdgen:def:chromeos.keymap]
> The ChromeOS key map assigns a DOM `KeyboardEvent.code` to each of the 48
> desktop ISO positions, in order:
> - `Backquote`, `Digit1`…`Digit9`, `Digit0`, `Minus`, `Equal`;
> - `KeyQ`…`KeyP`, `BracketLeft`, `BracketRight`;
> - `KeyA`…`KeyL`, `Semicolon`, `Quote`, `Backslash`;
> - `IntlBackslash`, `KeyZ`…`KeyM`, `Comma`, `Period`, `Slash`.
>
> Letters follow QWERTY order. Tokens are assigned to these positions per
> `[spec:kbdgen:req:keys.iso-order.desktop-layers]`. `Space` is not mapped.

> [spec:kbdgen:req:chromeos.descriptor]
> `background.js` MUST be the bundled `template-chromeos-keyboard.js`
> verbatim, followed by `\n\nconst descriptor = {json}\n\nKeyboard.install(descriptor)`
> with no trailing newline. `{json}` is pretty JSON keyed by language tag.
> Each value is `{"dead_keys", "transforms", "layers"}`:
> - `dead_keys` is `chromeOS.deadKeys` as written, or `{}`.
> - `layers` maps each layer name to `{code: raw token}`, in key-map order.
> - `transforms` is re-read from `{bundle}/layouts/{tag}.yaml`, using the
>   normalised tag. On a case-sensitive filesystem, a file named
>   `se-fi.yaml` therefore panics. The value is deserialised strictly as
>   `map<string, map<string, string>>`, so a top-level leaf or deeper nesting
>   panics. If the layout has no transforms, it is `{}`.
>
> Tokens are not decoded. If no layout has a `chromeOS` section, the step
> panics with `Could not generate background.js`.

> [spec:kbdgen:sem:chromeos.descriptor.runtime]
> The embedded runtime builds the layer name from `caps`, `ctrl`, `alt` (only
> while AltRight is held) and `shift`, joined with `+`, or `default` if none
> apply. It tries that layer first, then `caps`, then `shift` when the name
> contains them, and finally `default`. It commits the result or passes the
> key through.
>
> The runtime reads `descriptor[id].deadKeys` and `.space`, but the
> generator emits `dead_keys` and no `space`. Every `keydown` on a defined
> layer therefore throws a `TypeError`.

> [spec:kbdgen:req:chromeos.manifest]
> If `targets/chromeos.yaml` is missing, the step panics with
> `Could not generate manifest` after `background.js` is written.
> Otherwise, `manifest.json` MUST be manifest v2 with these fields:
> - `name` `__MSG_name__` and `description` `__MSG_description__`;
> - `version` from `build` and `version_name` from `version`; `appId` is
>   unused;
> - `background.scripts` `["background.js"]` and `permissions` `["input"]`;
> - `default_locale` `en`;
> - icons `icon16.png`, `icon48.png` and `icon128.png`, which are never
>   produced;
> - one input component per layout:
>   `{"name"/"description": "__MSG_{tag_}__", "type": "ime", "id": tag, "language": config.locale ?? "en-US", "layouts": [config.xkbLayout ?? "us"]}`.
>
> `project.yaml` `locales` MUST contain `en`. For each language `k` in any
> `chromeOS` layout's `displayNames`, kbdgen writes
> `_locales/{k_}/messages.json`. It has one `{tag_: {"message": T}}` entry
> per layout that has a name for `k`, then `name` and `description` from
> `locales[k]`, falling back to `locales.en`. `T` substitutes the name into
> `{} tastatur` (`nb`, `no`, `nn`, `da`), `{} tangentbord` (`sv`),
> `{} näppäimistö` (`fi`), or `{} keyboard` (all other tags). In these
> rules, `x_` means `x` with `-`→`_`.
