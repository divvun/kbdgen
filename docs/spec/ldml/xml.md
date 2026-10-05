# LDML keyboard3 XML

`kbd-ldml` reads and writes LDML keyboard3 documents, resolves them to the
keyboard model (`ldml.model.*`), and hosts the parsers for LDML's text
syntaxes: escapes, transform `from`/`to`, sets and usets. Every XML byte
goes through the `xmlem` crate (operator decision). An `xmlem::Document` is
the *source document*. v4 YAML lowers into one (`ldml.yaml.lowering`), and
`kbdgen ldml export` writes one. So the XML that kbdgen exports is exactly
what the engine's model was resolved from.

The rules about what `xmlem` preserves were checked against `xmlem` 0.3.3
(`Document::from_reader`, `display::process_entities`, `Config`).

Sources: UTS #35 Part 7 (§Escaping, §Element: import, §Implied Keys,
§Implied Form Values, §Element: variables, §Regex-like Syntax, §Transform
Grammar, §Normalization, §Extensibility); `ldmlKeyboard3.dtd`;
`keyboards/import/*.xml`; `xmlem` 0.3.3 source.

## Documents

> [spec:kbdgen:def:ldml.xml.document]
> A source document is an `xmlem::Document` that is either:
>
> - a keyboard, whose root is `keyboard3`
> - an import file, whose root is one of `displays`, `flicks`, `forms`,
>   `keys`, `layers`, `transformGroup`, `transforms` or `variables`
>
> Typed accessors in `kbd-ldml` read and build these trees. They never hold
> a second copy that could drift from the tree. Element names match on
> local name in the default namespace. kbdgen's extension elements are
> recognised by the namespace URI bound to their prefix on `keyboard3`
> (`ldml.xml.special`).

> [spec:kbdgen:req:ldml.xml.read]
> Reading MUST use `xmlem::Document::from_reader` (or `from_str`) on UTF-8
> input. A leading UTF-8 byte-order mark is skipped. Other encodings,
> malformed XML, a root other than `keyboard3` for a keyboard, and a
> duplicate attribute are errors that name the file and, where `xmlem`
> reports one, the position. A `keyboard3` root needs:
>
> - `locale` and `conformsTo`, with `conformsTo` from 45 to 49
> - an `xmlns`, if present, equal to
>   `https://schemas.unicode.org/cldr/<N>/keyboard3` for some `N` from 45
>   to 49, else a warning

> [spec:kbdgen:req:ldml.xml.write]
> Writing MUST serialize the source document with
> `Document::to_string_pretty_with_config(&Config::default_pretty())`. That
> gives two-space indent, `max_line_length` 120 and `EntityMode::Standard`.
> The output gets LF line breaks and a final newline. Generated documents
> carry `Declaration::v1_0()`. The writer MUST NOT call `Document::sort`,
> because that reorders attributes and elements. Writing the same document
> twice yields identical bytes.

> [spec:kbdgen:thm:ldml.xml.roundtrip]
> For a document D read per `ldml.xml.read`, read(write(D)) equals D in:
>
> - the order and names of elements
> - the order, names and values of attributes, with values decoded
> - comments: their text and position among sibling nodes, and comments
>   before and after the root
> - CDATA sections and non-whitespace text
> - the DOCTYPE string and the XML declaration's version, encoding and
>   standalone
>
> Proof sketch: `from_reader` stores attributes in an `IndexMap` in source
> order and children in source order. The pretty printer emits both in
> stored order and re-escapes exactly the characters the reader unescapes.
> So resolution of D and of read(write(D)) yields equal models.

> [spec:kbdgen:req:ldml.xml.lossy]
> `xmlem` does not preserve the following. The reader MUST warn once per file
> for each kind that occurs in it.
>
> - processing instructions, other than the XML declaration
> - whitespace-only text, i.e. indentation and blank lines; output is
>   re-indented
> - character and entity references: `&#x301;` comes back as a literal
>   U+0301; only `& < > " '` and Separator/Other characters other than
>   U+0020 are written as entities
> - single-quoted attributes, written with `"`
> - `<a></a>`, written as `<a />`
> - raw tab, CR or LF inside an attribute value
>
> The pre-scan that detects these runs on the raw input, before `xmlem`. The
> list is about the XML file only: what a document *means* never depends on
> anything in it. LDML wants control and format characters written as
> `\u{…}`, which survives unchanged.

## Validation

> [spec:kbdgen:req:ldml.xml.validate+1]
> After imports are resolved, a keyboard MUST satisfy:
>
> - the DTD content model, attribute enumerations and `@MATCH` patterns of
>   `ldmlKeyboard3.dtd`
> - the textual constraints of UTS #35 Part 7: id patterns, variable id
>   uniqueness and `[0-9A-Za-z_]{1,32}`, gap restrictions, at least one
>   `layers`, at most one hardware `layers`, distinct touch
>   `minDeviceWidth`, a `base` layer per touch set, `modifiers` required on
>   hardware layers
> - the model invariants (`ldml.model.invariants`)
>
> An unknown element or attribute outside `special` is an error. Content
> inside `special` is kept as is, and only kbdgen's own namespace is
> interpreted. Every error names the file, the element path
> (`keyboard3/keys/key[id=e-acute]`) and the attribute. `reorder@preBase`
> and `@tertiaryBase` also accept `1` and `0`, as UTS #35's own examples
> write them. Not errors, but warnings: a `display` whose string starts
> with a combining mark (CLDR `bn.xml`), and a `display@keyId` naming no
> key, which is dropped, since displays may be shared across keyboards.

## Imports and implied data

> [spec:kbdgen:sem:ldml.xml.import]
> An `import` is replaced by the children of the imported file's root, at
> the import's position. The imported root's name MUST equal the import's
> parent element, else it is an error. `base="cldr"` resolves `path` in the
> embedded CLDR data (`ldml.xml.cldr-data`); otherwise `path` is relative
> to the importing file's directory. Each file is imported at most once per
> keyboard; a repeat is a cycle error. A later element with the same
> identity replaces the earlier one in place:
>
> - key, flick and form by `id`
> - display by `output` or `keyId`
> - variable by `id`
> - layer by `id`, or by `modifiers` if it has no `id`
>
> Import files are validated like keyboards.

> [spec:kbdgen:def:ldml.xml.cldr-data]
> `kbd-ldml` embeds the `keyboards/import/*.xml` files of CLDR releases 45,
> 46, 47 and 48, each under its major version (`45/keys-Zyyy-punctuation.xml`).
> A CLDR release that changes the keyboard DTD adds its directory. A
> `base="cldr"` path whose version or file is not embedded is an error naming
> the path. The files stay under Unicode-3.0, with their copyright headers.

> [spec:kbdgen:sem:ldml.xml.implied+1]
> Before the keyboard's own children are read, resolution behaves as if:
>
> - `<keys>` began with an import of `keys-Latn-implied.xml`, whose keys
>   come first in the key table in that file's order: `gap` (a gap),
>   `space` (output U+0020, stretch), then `0`–`9`, `A`–`Z` and `a`–`z`
>   with output equal to the id
> - `<forms>` began with an import of `scanCodes-implied.xml`, which defines
>   `us`, `iso`, `jis`, `ks` and `abnt2`
>
> Both files are taken from the newest embedded CLDR release not above the
> keyboard's `conformsTo` (`ldml.xml.cldr-data`), so `conformsTo="49"`
> uses release 48. The keyboard's own elements override these per
> `ldml.xml.import`. `formId="touch"` names no form.

## Syntax

> [spec:kbdgen:syn:ldml.xml.escape+1]
> A UTS 18 escape is `\u{` *h* (` ` *h*)* `}`. Each *h* is 1–6 hex digits
> in either case, naming a Unicode scalar value. A surrogate, a value above
> U+10FFFF or an empty brace is an error, not literal text. A `\` or `$`
> that starts no escape, marker or variable is literal, so a `\u` without
> `{` is literal text, as CLDR `fr.xml` writes it (v4 YAML instead rejects
> it, `ldml.yaml.escape`). The escape is decoded in:
>
> - `key@output`
> - `transform@from` and `@to`
> - `string`, `set` and `uset` `@value`
> - `display@output` and `@display`
> - `displayOptions@baseCharacter`
> - `reorder@from` and `@before`
> - keyboardTest3 `startContext@to`, `emit@to` and `check@result`
>
> `\m{name}` (NMTOKEN) is a marker in outputs, display outputs, string and
> set values, and transforms; `\m{.}` outside `transform@from` is an error.
> `${id}` substitutes a string variable in those attributes,
> `display@display` included.

> [spec:kbdgen:syn:ldml.xml.from+1]
> `transform@from` MUST match `from-match` of §Transform From Grammar, and
> `kbd-ldml` MUST parse it into a Pattern (`ldml.model.pattern`). The
> grammar's well-formedness and validity constraints are errors:
>
> - at most nine captures; a capture holds a plain sequence, with no
>   alternation and no group inside it
> - referenced variables defined
> - every disallowed feature of §Disallowed Regex Features
> - a pattern that can match the empty text
>
> `${s}` is substituted textually before parsing; its value is parsed as
> pattern syntax. `$[v]` names a `set` or `uset`. LDML is ambiguous on
> whether a class can match a marker. Here a class matches markers only
> through `\m{…}` members: a positive class with `\m{.}` becomes
> `(?:\m{.}|[rest])`, and a negated class never matches a marker, its
> `\m{.}` dropped. With normalization enabled, each run of unquantified
> literal characters is put in NFD, and a quantified non-NFD character
> becomes a quantified group of its decomposition.

> [spec:kbdgen:syn:ldml.xml.to]
> `transform@to` MUST match `to-replacement` of §Transform To Grammar. It
> parses to a Replacement in which:
>
> - `$$`, `\$` and `\\` are literal
> - `$0`–`$9` are groups
> - `${s}` inserts a string variable's value, with its markers
> - `$[n:v]` is a mapped set
> - `\m{…}` emits a marker
>
> It is an error to reference a group that `from` lacks. `$[n:v]` is valid
> only when capture *n* contains exactly one `set` reference and nothing
> else, and *v* is a `set` with as many items. A `uset` is never mapped.

> [spec:kbdgen:syn:ldml.xml.sets+1]
> A `set` value is items separated by ASCII whitespace after `${…}`
> substitution. Whitespace inside a `\u{…}` escape separates scalar values
> of one item, not items. `$[set]` references splice in an earlier set's
> items, and must be whitespace-separated. A `uset` value is the
> UnicodeSet subset of §Element: uset:
>
> - brackets, ranges and `\u{…}` escapes
> - `$[uset]` references, union, and the difference `[$[a]-[b]]`
> - no `{…}` strings and no `\p{…}` or `[:…:]` properties
>
> Both resolve to explicit item lists or scalar ranges at build time.

> [spec:kbdgen:req:ldml.xml.nfd-classes+1]
> With normalization enabled, resolution MUST leave only NFD scalar values
> in classes (§Normalization and Character Classes), as `ldml.model.nfd`
> requires:
>
> - A `from` class or a `uset` that lists a non-NFD value is an error
>   naming the value.
> - A range that contains non-NFD values without listing them, such as
>   `[\u{20}-\u{1FF}]`, warns, and those values are removed; they never
>   occur in NFD text.
> - In `reorder@from` and `@before`, a bare non-NFD character is an error.
>   A bracketed class or `$[uset]` element warns and removes non-NFD
>   values, listed ones included (CLDR `bn.xml` lists U+09CB). An element
>   left with no value is an error.
>
> None of this applies with normalization disabled.

## Resolution

> [spec:kbdgen:sem:ldml.xml.resolve+1]
> Resolving a source document to a model runs these steps in order:
>
> 1. imports, implied keys and forms, and overrides
> 2. validation (`ldml.xml.validate`)
> 3. variables, in definition order
> 4. escape decoding and marker interning
> 5. normalization of texts per `ldml.model.nfd`
> 6. key, flick and display tables
> 7. forms and layers, with modifier sets canonicalised
> 8. patterns and replacements
> 9. reorder split-and-merge, then a stable sort into priority: most
>    `from` elements, then most `before` elements
> 10. `context_len`
> 11. extensions (`ldml.xml.special`)
> 12. model invariants (`ldml.model.invariants`)
>
> Markers are interned on first use in this order: display outputs, key
> outputs, transforms, then extensions. A variable's markers are interned
> where it is used. Any error stops resolution. Warnings are collected and
> returned with the model.

## Extensions

The model is a superset of LDML (`ldml.model.*`). Its superset parts travel
in LDML's own extension point, `special`, under a kbdgen namespace. An
exported file is therefore a valid LDML keyboard, and reading it back loses
nothing.

> [spec:kbdgen:def:ldml.xml.special+1]
> Superset data lives in `special` elements in the namespace
> `https://divvun.no/ns/kbdgen-ldml/1`, bound on `keyboard3` to the prefix
> `kbdgen`. These elements carry attributes only, never text. Each row below
> names the element whose `special` child holds the kbdgen element.
>
> | Parent | Element | Attributes | Model field |
> |---|---|---|---|
> | `keyboard3` | `kbdgen:keyboard` | `tag`, `host`?, `decimal`?, `spaceLabel`?, `returnLabel`?, `impliedLayers`? | layout tag, `host`, `decimal`, `labels`; `impliedLayers` is authoring metadata for import |
> | `keyboard3` | `kbdgen:displayName` | `lang`, `name` | `display_names` |
> | `keyboard3` | `kbdgen:flush` | `marker`, `output` | `flush` |
> | `keyboard3` | `kbdgen:deadKeyName` | `marker`, `name` | `dead_key_names` |
> | `keyboard3` | `kbdgen:windows` | `shiftLock`?, `lrmRlm`? | `windows` |
> | `keyboard3` | `kbdgen:windowsKeyName` | `key`, `name` | `windows.key_names` |
> | `keyboard3` | `kbdgen:target` | `host`, `name`, `value` | per-layout target config (`ldml.yaml.targets`) |
> | `keyboard3` | `kbdgen:emojiKey` | `scanCode`, `modifiers` | `emoji.key` |
> | `keyboard3` | `kbdgen:emoji` | `emoji`, `name`, `keywords` (`|`-separated) | `emoji.annotations` |
> | `keyboard3` | `kbdgen:extraModifier` | `key` | `windows.extra_modifiers`, for a keyboard without hardware `layers` |
> | `keyboard3` | `kbdgen:deadKey`, with nested `kbdgen:compose` children | `identity`, `marker`, `display`, `standalone`, `name`?; a compose has `input` plus either `output` or (`marker`, `standalone`, children) | authoring metadata: the v4 `deadKeys` table (`ldml.yaml.import`) |
> | `transformGroup` | `kbdgen:generated` | `by` (`deadKeys-compose`, `deadKeys-fallback` or `deadKeys-backspace`) | marks a generated group; no model field |
> | `keys` | `kbdgen:role` | `keyId`, `role` | key `role` |
> | touch `layers` | `kbdgen:touchSet` | `name`, `bottomRow` | touch set `name`, `bottom_row` |
> | hardware `layers` | `kbdgen:extraModifier` | `key` | `windows.extra_modifiers`, in document order |
> | hardware `layers` | `kbdgen:layer`, with `kbdgen:row keys=` children | `id`?, `modifiers` | a hardware layer that LDML cannot express |
>
> `kbdgen:layer` holds every native-only layer (`ldml.model.native`), and
> every layer whose sets use `cmd` or `extra`*n*. Its rows reference ordinary
> `key` elements by id. Resolution stores the LDML `layer`s first, then the
> `kbdgen:layer`s, each in document order. A touch `layers` without
> `kbdgen:touchSet` gets no name and `bottomRow` `authored`; a
> `kbdgen:touchSet` without `bottomRow` means `host`. Authoring metadata
> never changes the model. Resolution ignores it, and import uses it only
> after checking it against the semantic elements.

> [spec:kbdgen:sem:ldml.xml.ldml-view+1]
> A consumer that knows only LDML ignores kbdgen's `special` content, as
> §Extensibility allows. It sees a keyboard that:
>
> - has every LDML layer, key, flick, display, variable and transform group,
>   including the groups generated for dead keys. So dead keys compose, fall
>   back and backspace as in kbdgen's engine.
> - draws role keys as gaps, or as layer-switch keys for the roles that
>   switch layers
> - has the `space` label as `<display keyId="space">` when the model
>   holds that display, which v4 lowering adds (`ldml.yaml.displays.auto`);
>   export itself writes the label only as `kbdgen:keyboard@spaceLabel`
> - lacks native-only and `extra`*n* layers, so it outputs nothing for those
>   modifier states
> - lacks the decimal key, flush outputs and the Windows options. A pending
>   dead key is therefore dropped, not committed, when the context changes.

> [spec:kbdgen:thm:ldml.xml.superset-roundtrip]
> For any keyboard K that kbdgen exports, resolving the exported document
> gives a model equal to K. Every model field is written either as an LDML
> element or attribute (the LDML subset) or as one `kbdgen` element of
> `ldml.xml.special` (the superset). Resolution reads both back
> (`ldml.xml.resolve` step 11), and `ldml.xml.roundtrip` preserves each one.

## Export

> [spec:kbdgen:req:ldml.xml.export+1]
> A generated keyboard document MUST:
>
> - order elements as the DTD sequence: `import`, `locales`, `version`,
>   `info`, `settings`, `displays`, `keys`, `flicks`, `forms`, `layers`,
>   `variables`, `transforms`, `special`
> - order attributes as the DTD declares them
> - set `xmlns` to `https://schemas.unicode.org/cldr/<conformsTo>/keyboard3`
> - set `conformsTo` to the greater of the model's `conforms_to` and the
>   minimum the content needs: 45, or 47 when `info@attribution` is present
> - omit implied keys and forms unless they are overridden
> - write modifier sets canonically (`ldml.model.modifiers`), with
>   components space-separated and sets separated by `", "`
> - write `width` with at most three decimals and no trailing zeros
>
> Output is deterministic.

> [spec:kbdgen:req:ldml.xml.export.escape]
> In generated attribute values the writer MUST use `\u{…}` (uppercase hex,
> at least four digits) for:
>
> - characters of general category M, Cc, Cf or Z other than U+0020
> - in `output` and `display`, a `\` that would start an escape, and a `$`
>   before `{` or `[`
> - in `from`, literal regex metacharacters (§Regex-like Syntax), escaped as
>   `\` plus the character instead
> - in `to`, `$` and `\`, written as `$$` and `\\`
>
> Everything else is literal. So `xmlem`'s own entity escaping only ever
> touches `& < > " '`.

> [spec:kbdgen:req:ldml.xml.ldml-ref+1]
> For a layout that references XML (`ldml.yaml.ldml-ref`), export MUST
> re-serialize the read document, which preserves its structure per
> `ldml.xml.roundtrip`. In `keyboard3`'s `special` it replaces only the
> layout-level elements `kbdgen:keyboard`, `kbdgen:displayName`,
> `kbdgen:target`, `kbdgen:emojiKey` and `kbdgen:emoji` with the layout
> file's data. They go into the `special` that held kbdgen elements, else
> the first `special`, else an appended one. The file's
> `kbdgen:keyboard@impliedLayers` is kept. Other kbdgen elements
> (`flush`, `deadKeyName`, `windows`, …), other `special` content and
> comments are kept.
