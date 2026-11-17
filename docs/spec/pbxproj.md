# Xcode project.pbxproj

kbdgen edits the iOS template's `GiellaKeyboard.xcodeproj/project.pbxproj` by
reading it into a typed model, mutating that model (see `xcode.targets`,
`xcode.localization`, `ios.build-chain`), and writing it back with its own
serializer. These rules pin the model and the serializer's exact output. The
developer tool `examples/pbxproj-roundtrip.rs` reads a file and prints the
re-serialized result.

## Model

> [spec:kbdgen:def:pbxproj.model+2]
> The pbxproj model is the top-level object with exactly the string fields
> `archiveVersion`, `objectVersion`, `rootObject`, an untyped `classes`, and
> `objects`, mapping ids to objects discriminated by `isa`. Exactly 18 kinds
> are modelled: `PBXBuildFile`, `PBXContainerItemProxy`,
> `PBXCopyFilesBuildPhase`, `PBXFileReference`, `PBXFrameworksBuildPhase`,
> `PBXGroup`, `PBXHeadersBuildPhase`, `PBXNativeTarget`, `PBXProject`,
> `PBXResourcesBuildPhase`, `PBXShellScriptBuildPhase`,
> `PBXSourcesBuildPhase`, `PBXTargetDependency`, `PBXVariantGroup`,
> `XCBuildConfiguration`, `XCConfigurationList`,
> `XCRemoteSwiftPackageReference`, `XCSwiftPackageProductDependency`. A
> `PBXBuildFile` names its subject by `fileRef` or, for a Swift Package
> product, by `productRef`; neither is required. `PBXProject.packageReferences`
> and `PBXNativeTarget.packageProductDependencies` are optional id lists. A
> package reference has `repositoryURL` and a string-to-string `requirement`;
> a product dependency has `productName` and an optional `package`. Scalars
> are strings, except the untyped JSON of `buildSettings` values and
> `PBXBuildFile.settings` values; `PBXProject.attributes` MUST be a
> dictionary of untyped JSON values. The top level and every kind except the
> six build phases reject unknown fields; build phases silently drop them.

> [spec:kbdgen:req:pbxproj.model.read+1]
> A pbxproj MUST be read by copying it to `tmp.pbxproj` in a fresh temporary
> directory, running `plutil -convert json` on the copy, and parsing the
> result as JSON into the model; the original file is untouched. Reading MUST
> fail with an error when the copy fails, `plutil` cannot be started, `plutil`
> exits non-zero (the error names the command and quotes the tail of its
> stderr), or the JSON does not fit the model (invalid JSON, unknown `isa`,
> unknown field on a strict kind, missing field, non-dictionary
> `attributes`). Operations needing the project MUST fail with an error when
> `rootObject` is not a `PBXProject`.

> [spec:kbdgen:sem:pbxproj.model.ordering+1]
> Reading normalizes order: `objects` is ordered by id, and every id list
> (`targets`, `children`, `buildPhases`, `dependencies`, `buildRules`, phase
> `files`, `buildConfigurations`, `packageReferences`,
> `packageProductDependencies`), `knownRegions` and the shell-script path
> lists are sets, deduplicated and sorted byte-wise, so a target's build
> phases are written in id order rather than authored order.
> `PBXBuildFile.settings`, `PBXProject.attributes` and package `requirement`
> keys are sorted.
> `buildSettings` keeps the key order of the JSON that `plutil` produced
> (which need not match the original file); setting an existing key replaces
> it in place and a new key is appended.

> [spec:kbdgen:req:pbxproj.ids]
> Every object kbdgen creates MUST get a new id of exactly 24 characters drawn
> uniformly at random from `0123456789ABCDEF` using a thread-local RNG. Ids
> are not checked against existing ones (a collision overwrites that
> object), and ids read from the input are not validated.

## Serialization

> [spec:kbdgen:syn:pbxproj.serialize+2]
> Output is `// !$*UTF8*$!`, then a `{ … }` block of tab-indented
> `archiveVersion`, an always-empty `classes = {` / `};`, `objectVersion`,
> `objects = {` + empty line + sections + `};`, and `rootObject`, ending with
> a newline. Every kind has a section, even when empty, in byte-wise `isa`
> order; the first three and the two Swift Package kinds open with
> `/* Begin <isa> section */`, the rest with
> `/* Start <isa> section */` plus an empty line, and all close with
> `/* End <isa> section */` plus an empty line. Objects appear in id order as
> `\t\t<id> /* <c> */ = {`, `\t\t\tisa = <isa>;`, one `\t\t\t<key> = <value>;`
> per field, `\t\t};`; lists as `(` + `\n\t\t\t\t<item> /* <p> */,` per item +
> `\n\t\t\t)`, where `<p>` is the fixed placeholder word T-O-D-O, except the
> shell-script path lists of `pbxproj.quoting`, whose items are
> `\n\t\t\t\t"<item>",` with no comment. `<c>` is the `name`, else `path`,
> for file references and groups (omitted if neither),
> `PBXContainerItemProxy` or `Project object` for those kinds, else that
> same placeholder word.

> [spec:kbdgen:def:pbxproj.serialize.fields+2]
> The field layout of a kind is the fields the serializer writes, in order
> (`?` = when present); others, notably `PBXFileReference.explicitFileType`
> and `PBXResourcesBuildPhase.name`, are dropped. ContainerItemProxy:
> `containerPortal proxyType remoteGlobalIDString remoteInfo`. CopyFiles:
> `buildActionMask dstPath dstSubfolderSpec files name? runOnly…`; other
> phases: `buildActionMask files runOnly…`, ShellScript adding
> `inputFileListPaths? inputPaths name outputFileListPaths? outputPaths`
> before and `shellPath shellScript showEnvVarsInLog?` after `runOnly…`.
> Group: `children path? name? sourceTree` (VariantGroup swaps `name?`
> and `path?`). NativeTarget: `buildConfigurationList buildPhases buildRules
> dependencies name packageProductDependencies? productName
> productReference? productType?`. Project: `attributes`, then its remaining
> fields alphabetically, `packageReferences?` among them. TargetDependency:
> `target targetProxy`. XCBuildConfiguration: `buildSettings name` (so
> `baseConfigurationReference` is dropped). ConfigurationList: its three
> fields alphabetically. RemoteSwiftPackageReference: `repositoryURL
> requirement`, the latter as `requirement = {`, one `<key> = <value>;` line
> per entry a tab deeper, and `};`. SwiftPackageProductDependency: `package?
> productName`. `PBXBuildFile` (`fileRef? productRef? settings?`) and
> `PBXFileReference` are single-line `{isa = …; key = value; …}`. Build-file
> settings are `settings = {` + `<key> = <value>;` per entry + `};`, an array
> value as `(a, b, )` and a string value Debug-quoted; any other value kind,
> or a non-string array item, MUST panic.

> [spec:kbdgen:req:pbxproj.quoting+2]
> "Debug-quoted" means Rust `{:?}` formatting (wrapped in `"`; `"`, `\`,
> control and non-printable characters backslash-escaped, the latter as
> `\u{…}`). The serializer MUST quote exactly so: `PBXFileReference.name`
> and `PBXFileReference.path` each iff it contains `-` or `+`; `PBXGroup`
> `path` and `name` iff they contain a space; `sourceTree` equal to `<group>`
> as `"<group>"`. Always Debug-quoted: `PBXVariantGroup.name`,
> `PBXNativeTarget.name`, CopyFiles `dstPath` and `name`, `PBXProject`
> `compatibilityVersion`/`projectDirPath`/`projectRoot`, shell-script `name`
> and `shellScript`, `XCBuildConfiguration.name`, string build-setting
> values, string build-file settings, a package reference's `repositoryURL`
> and `requirement` values, and a product dependency's `productName`.
> Shell-script `inputPaths`, `outputPaths`, `inputFileListPaths` and
> `outputFileListPaths` items are wrapped in `"` unescaped. Everything else is
> written raw.

> [spec:kbdgen:req:pbxproj.serialize.build-settings+1]
> `buildSettings` MUST be written as an empty line, `\t\t\tbuildSettings = {`,
> the entries in model order, then `\n\t\t\t};`. A string entry is
> `\n\t\t\t\t<key> = <Debug-quoted value>;`, where `<key>` is wrapped in `"`
> (unescaped) iff it contains `[`. An array entry is `\n\t\t\t<key> = (`
> (raw key, three tabs), `\n\t\t\t\t<element>,` per element rendered as JSON,
> then `\n\t\t\t);\n`. Any other value kind MUST panic
> (`build setting <key> has an unsupported value <value>`).

> [spec:kbdgen:req:pbxproj.serialize.attributes+1]
> `PBXProject.attributes` MUST be written between `\t\t\tattributes = {` and
> `\t\t\t};`. At depth d (starting at 4 tabs) each key, in sorted order, is
> written as d tabs + `<key> = ` and then either `{`, newline, its children
> at depth d+1, d tabs and `};` for an object; `(`, newline, each item as
> d+1 tabs, the raw string and `,`, then d tabs and `);` for an array; or the
> raw string and `;` for a string, each line newline-terminated. Null,
> boolean and number values, and non-string array items, MUST panic.
