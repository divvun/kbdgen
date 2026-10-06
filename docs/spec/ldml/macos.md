# Model to macOS keyboard layout

The macOS target writes one static `.keylayout` per layout into a
keyboard-layout bundle (`docs/spec/macos.md`). For a v4 layout, the
document is derived from the `macOS` host document of its compiled layout
(`ldml.model.layout`) by the adapter below. There is no Input Method Kit
host yet, so whatever a `.keylayout` cannot express is lost on macOS, and
each such feature is reported per layout.

Both v3 and v4 layouts feed one *keylayout input*, and one writer turns it
into XML. So v3 output stays as `keylayout.*` specifies, and the two paths
can be compared document by document.

Sources: `docs/spec/macos.md` (`keylayout.*`, `macbundle.*`);
`docs/spec/ldml/kbdl.md` as the template for an adapter; Apple TN2056
(Keyboard Layout XML) and `/System/Library/DTDs/KeyboardLayout.dtd`.

## Input and writer

> [spec:kbdgen:def:ldml.macos.input]
> A *keylayout input* has the keyboard name (`keylayout.document.name`),
> the language tag, the display names, a default index, a list of key maps
> and a list of terminators. A key map has a list of modifier terms, the
> `keys` of its `<modifier>` elements, and keys in emission order. A key
> has a macOS key code and either an output or an action: an id and a list
> of `when`s, each a state with an output, a next state, or both. Texts
> are decoded. The writer emits the map at index *i* as `keyMapSelect
> mapIndex=i`, omitted when it has no terms, and `keyMap index=i`, then
> each action in key order, then the terminators, per `keylayout.document`.
> The v3 path builds the input as the `keylayout.*` rules read tokens.

> [spec:kbdgen:def:ldml.macos.adapter]
> The adapter's input is a compiled layout with a `macOS` keyboard K. Its
> output is one keylayout input, named from the layout's tag. A layout
> without a `macOS` document has no macOS output. The adapter MUST NOT
> panic. It warns, or fails naming the layout and the position, the layer
> or the marker. Pressing a dead key from the reset state that does not
> leave exactly its marker pending is fatal.

## Key maps

> [spec:kbdgen:sem:ldml.macos.modifiers]
> Each hardware layer of K becomes one key map, in model order. A set with
> an `extra` component, or with `caps` when Caps Lock is bound as an extra
> modifier, is dropped with a warning; a layer left with no set and not
> `other` is dropped. The `other` layer's map is the default index, else
> 0. Sets S and S+`caps` of one layer share one term with `caps?`. A term
> is the v3 term of `keylayout.keymaps` for the layer the set migrates
> from; `ctrl` takes v3's loose term only when no typing set names ctrl
> and no layer is `other`. Other sets give `anyShift`, `caps`,
> `anyOption`/`option`/`rightOption`, `anyControl`/`control`/`rightControl`
> and `command`, plus `command?` on typing sets. Sided sets warn.

> [spec:kbdgen:sem:ldml.macos.keys]
> A map's keys are the 48 ISO positions, at the key codes of
> `keylayout.keymaps.iso-codes` for their scan codes, grouped by value in
> order of first occurrence; then the fixed keys of
> `keylayout.keymaps.iso-codes`; code 65 with the decimal, or `.` when K
> has none; and code 49 with the key at scan code `39`. A value is the
> output text, or a dead key's marker when the output is one marker. No
> key, a gap and an empty output give no `<key>`; an output mixing text
> and markers warns and gives none. Keys at other scan codes warn and are
> dropped. Native layers are written like the others.

## Dead keys

> [spec:kbdgen:sem:ldml.macos.dead-keys]
> States are derived by running the engine (`ldml.engine.api`; `Nfc`,
> `CancelOrPass`, host `macOS`) from each dead key's marker, pressing every
> distinct value by key id or `Decimal`. The default for state m and value
> v is m's terminator, `flush[m]`, followed by v's text, or v's marker left
> pending. A different result gives v a `when` for m: its committed text,
> or `next` when it commits nothing and leaves one marker n, which is a
> state too. Other results warn and are omitted. States are numbered
> `dead_key%03d` in the order their markers first start a `simple` rule,
> which is `deadKeys` order. A value with `when`s, or a dead key, is an
> action key; action ids are numbered `action%03d` in emission order.

> [spec:kbdgen:sem:ldml.macos.semantics]
> A `.keylayout` is taken to behave as TN2056 describes. The first map,
> in index order, with a term matching the held modifiers is used, else
> the default index. A term lists required keys and, with `?`, optional
> ones; an unlisted key must be up, `anyX` accepts either side and `X`
> names the left. A key the map lacks types nothing and keeps the state.
> An output key, or an action without a `when` for the pending state,
> types the state's terminator and then acts in state `none`; a matching
> `when` types its output and moves to its `next`, else to `none`. The
> adapter's terms select at most one map in any state the engine types in,
> so typing does not depend on match order.

## Reporting

> [spec:kbdgen:req:ldml.macos.classify]
> The adapter MUST warn, once per layout and feature, with a count and up
> to ten examples, for each feature of K the `.keylayout` cannot express:
>
> - transforms that do not start with a marker, `reorder` groups, and
>   `backspace` rules other than the generated one
> - outputs mixing text and markers, and omitted dead-key results
> - long-press, multi-tap and flick gestures on keys of written layers
> - displays other than the automatic ones, and space and return labels
> - extra-modifier bindings and LRM/RLM
> - an enabled `normalization`
>
> A layout with no warning at all gets one information message saying so.

## Target integration

> [spec:kbdgen:req:ldml.macos.target]
> `target macos` MUST build each v4 layout through the adapter and each v3
> layout as before, in bundle layout order, into one bundle.
> Everything outside the keylayout input follows `docs/spec/macos.md`
> unchanged: `Info.plist`, `InfoPlist.strings`, icons and the installer.
> For a v4 layout, `displayNames` come from the compiled layout. Two
> layouts with one keyboard name are fatal. This replaces the `macos`
> clause of `ldml.yaml.coexistence`.

## Tests

> [spec:kbdgen:req:ldml.macos.test.typing]
> For each golden v4 layout and each v3 fixture migrated in memory, the
> written `.keylayout`, read back from its text and simulated per
> `ldml.macos.semantics`, MUST type what the engine types. Cases are every
> ISO position and the space bar in every state of Shift, Caps Lock,
> Option and Control by side, without Command and without a Right Control
> bound as an extra modifier, then dead-key paths up to three keys. Cases
> the engine passes are not compared. Committed text and pending display
> must agree, unless the difference is explained by a reported feature:
> text transforms, reorder, normalization, mixed outputs or omitted
> dead-key results. Each explanation is exercised by a synthetic layout.

> [spec:kbdgen:req:ldml.macos.test.golden]
> For the golden v4 layouts with a macOS variant, and for a v4 fixture
> exercising a chained dead key, an empty standalone, an `other` layer, a
> sided layer, a native layer, gestures and displays, the written
> `.keylayout` and the adapter's warnings and information messages MUST
> equal checked-in golden files byte for byte.

> [spec:kbdgen:req:ldml.macos.test.differential]
> For each v3 fixture with a `macOS` section, migrated in memory, the
> documents from the v3 input and from the adapter's input MUST be equal,
> with key maps matched by their terms, states matched by the dead keys
> entering them, and action ids not compared, except for differences each
> explained by: native layers listed last; M13 (`\u{0}` is no key); the
> space bar following `macOS.space` and composing with dead keys; dead
> keys composing on layers without a `deadKeys` list; a dead key keeping
> its composition with a pending dead key; or a transform root no macOS
> key makes dead (M03). A v3 layout without dead keys or native layers
> MUST give byte-identical documents.

## Open questions

These need an operator decision; the conservative option is implemented.

1. **Match order.** v3's `command?` on typing terms shadows its `cmd`
   layers if macOS takes the first matching map, as the migration oracle
   assumes. Typing is unaffected, but Command shortcuts may never reach
   authored `cmd` layers. Kept as v3 did.
2. **Sides.** `option`/`rightOption` are written as the model says, with
   a warning; macOS may not tell the sides apart in practice.
3. **Output and next together.** A `when` with both is legal in the DTD,
   but its behaviour is not verified, so such engine results are omitted.
