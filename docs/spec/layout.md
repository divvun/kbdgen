# Layout model and key notation

The layout model is the shared input contract consumed by every target
generator: one YAML file per language tag under `layouts/`. This document
pins the schema, the transforms tree, the key-escape grammar and the ISO key
order that desktop generators map layer tokens onto.

## Layout file schema

> [spec:kbdgen:def:layout.schema]
> A layout is one `layouts/<tag>.yaml` file: a YAML mapping, deserialised from
> an already-parsed YAML value, so string fields MUST be YAML strings (an
> unquoted number is an error). Unknown fields are ignored. Fields:
> `languageTag` (injected by the loader, overwriting any authored value);
> `displayNames` (required; BCP 47 tag → name, keys case-normalised);
> `decimal`; the optional target sections `windows`, `chromeOS`, `macOS`,
> `iOS`, `android`; `transforms`; `keyNames` (required `space` and `return`
> strings); and `longpress`, a mapping from key to one string that is split on
> whitespace into an ordered list of alternatives (used only by Android).
> Absent optional fields are absent, not empty. The loader decodes no escapes
> in any field.

> [spec:kbdgen:req:layout.schema.targets]
> Each target section MUST follow its fixed structure. A platform is a mapping
> whose only field `layers` maps layer names to layer strings in YAML order;
> an unknown layer name is a deserialisation error.
>
> | Section | Platforms | Other fields | Layer names |
> |---|---|---|---|
> | `windows` | `primary` (required) | `config` {`locale`, `id`}, `deadKeys` | `default` `shift` `caps` `caps+shift` `alt` `alt+shift` `alt+caps` `ctrl` |
> | `chromeOS` | `primary` (required) | `config` {`locale`, `xkbLayout`}, `deadKeys` | `default` `shift` `caps` `caps+shift` `alt` `alt+shift` `ctrl` |
> | `macOS` | `primary` (required) | `deadKeys`, `space` (parsed, unused) | Windows set plus `cmd` `cmd+shift` `cmd+alt` `cmd+alt+shift` |
> | `iOS` | `primary`, `iPad-9in`, `iPad-12in` (all optional) | `config` {`spellerPackageKey`, `spellerPath`}, `deadKeys` | `default` `shift` `caps` `alt` `alt+shift` `symbols-1` `symbols-2` |
> | `android` | `primary`, `tablet-600` (both required) | `config` {`spellerPackageKey`, `spellerPath`} | `default` `shift` |
>
> Config fields are optional. `deadKeys` maps a layer name to the list of key
> outputs that are dead keys on that layer.

## Transforms

> [spec:kbdgen:def:layout.transforms]
> `transforms` is an ordered tree for dead-key composition: a mapping from a
> dead key's output to a node, where a node is a leaf (YAML string: composed
> output) or a branch (mapping from the next input to a child node). Top-level
> keys MUST be YAML strings (else a deserialisation error). Inside the tree a
> non-string mapping key panics ("Only Strings are supported within map
> transforms!") and a number, boolean, null or sequence value panics ("Only
> Strings and Maps are supported within transforms!"), so digits MUST be
> quoted.

> [spec:kbdgen:req:layout.transforms.dead-key-entries]
> Generators treat each top-level entry as one dead key and support exactly
> one level of branching. A top-level leaf is logged ("Transform ended too
> soon for dead key …") and ignored. A branch child that is itself a branch
> panics (`todo!`) in the Windows, macOS, iOS and Android generators. The
> child keyed by a single space, the transform escape, is the dead key's
> standalone (terminator) output. Windows requires it for every dead key that
> occurs in its layers, and macOS for every top-level branch once any layer
> has `deadKeys`; there it MUST be a leaf, else generation panics ("The escape
> transform ` ` not found …" / "… should be a string …"). iOS and Android
> do not require it.

## Key notation

> [spec:kbdgen:syn:keys.escape]
> A key escape is `\u{H}` with H 1–6 hex digits of either case (regex
> `\\u\{([0-9A-Fa-f]{1,6})\}`). Decoding replaces every match anywhere in a
> string with the character of scalar value H; non-matching text (`\u{}`,
> seven digits, `\u` without braces) stays verbatim. H outside the Unicode scalar values
> (surrogates, above U+10FFFF) panics; the U+FEFF fallback for unparseable hex
> is unreachable. Whitespace outputs MUST be escaped (e.g. `\u{20}`) because
> layers are split on whitespace. Only two generators decode: Windows KLC
> decodes each layer token (not `deadKeys` or transforms), and macOS decodes
> every `output` attribute it writes, transform outputs included, while
> matching against raw tokens. iOS, Android and ChromeOS emit all strings
> undecoded.

> [spec:kbdgen:def:keys.iso-order]
> The ISO key order is the fixed sequence of 49 ISO/IEC 9995 positions,
> indices 0–48: `E00`–`E12` (13 keys, starting left of `1`), `D01`–`D12`,
> `C01`–`C12`, `B00`–`B11` (`B00` is the extra ISO key left of `B01`; `B11` is
> Brazilian-only). The desktop key tables (Windows scancode/VK, macOS key
> code, ChromeOS `KeyboardEvent.code`) each cover exactly the first 48
> positions, `E00`…`B10`, in this order; nothing maps `B11`.

> [spec:kbdgen:req:keys.iso-order.desktop-layers]
> For Windows, macOS and ChromeOS, every `primary` layer string is split on
> whitespace (newlines included) and token *i* is assigned to position *i*
> of the 48-key order `E00`…`B10`. Each layer MUST have at least 48 tokens;
> fewer panics with "Provided layer does not have enough keys, expected 48
> keys but got N" (the ChromeOS message says "Windows"). Tokens past the 48th
> are silently ignored, so a 49th `B11` token has no effect.
