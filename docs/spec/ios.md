# iOS

The iOS target (`kbdgen target -b <bundle> -o <out> ios build`) clones the
`giellakbd-ios` app template into `<out>/repo`, writes a keyboard definitions
JSON file, rewrites the template's Xcode project, plists and icons, provisions
signing material with fastlane, and archives the app with `xcodebuild`.

## Keyboard definitions JSON

> [spec:kbdgen:def:ios.keyboard-json]
> The keyboard definitions file is `<out>/repo/Keyboard/Models/KeyboardDefinitions.json`
> (its directory comes from the cloned template), a pretty-printed JSON array
> with one object per layout, in bundle layout order, that has both an `iOS`
> section and `keyNames`; other iOS layouts are silently skipped, and `[]` is
> written when none qualify. Object keys, in order: `name` (the autonym),
> `locale` (the language tag), `return` and `space` (from `keyNames`),
> `longPress` (key → whitespace-split alternatives, `{}` if absent),
> `deadKeys`, `transforms` (`ios.transforms`), and `iphone`, `ipad-9in`,
> `ipad-12in` built from platforms `primary`, `iPad-9in`, `iPad-12in`
> (`ios.rows`; `{}` when a platform is absent). `deadKeys` holds the layout's
> `iOS.deadKeys` copied unchanged under each of `iphone`, `ipad-9in` and
> `ipad-12in`, keyed by YAML layer ids (`default`, `shift`, …), not JSON
> layer names.

> [spec:kbdgen:syn:ios.rows]
> A platform object maps JSON layer names to rows, in YAML order: `default` →
> `normal`, `shift` → `shifted`; `caps`, `alt`, `alt+shift`, `symbols-1`,
> `symbols-2` are unchanged. A layer string is trimmed, split on `\n` into
> rows, and each row on whitespace into tokens. A token wholly matching
> `^\\s\{([^}:]+)(?::(\d+(?:\.\d+)?))?\}$` becomes `{"id": ID', "width": W}`,
> where ID' is ID with every `"` removed, prefixed with `_` unless ID starts
> with `"` (`\s{shift}` → `_shift`, `\s{"@":0.75}` → `@`), and W is the
> width as a 32-bit float, default `1.0`. Any other token, including
> near-misses such as `\s{shift:.5}`, is emitted as a verbatim string; no
> `\u{…}` decoding is done.

> [spec:kbdgen:req:ios.transforms]
> The JSON `transforms` object MUST have one entry per top-level transform
> key, in YAML order, each a flat object from following input to output,
> interpreted per `[spec:kbdgen:req:layout.transforms.dead-key-entries]`. A
> top-level leaf is emitted as `{}`. The `" "` child is copied like any other
> entry; none is synthesized when it is absent. All transforms are emitted,
> whether or not their keys occur in the iOS layers.

## Package identifiers

> [spec:kbdgen:req:ios.pkg-ids]
> A layout's extension bundle identifier MUST be `{packageId}.{ext}`, with
> `ext` the layout's language tag. When `packageId` is exactly
> `no.uit.giella.keyboards.Sami`, `ext` MUST instead come from this legacy
> table when the tag is listed: `se` and `sme` → `northern-sami-keyboard`,
> `sms` → `skolt-sami-keyboard`, `smn` → `inari-sami-keyboard`, `smj-SE` →
> `julev-sami-keyboard`, `smj-NO` → `julev-sami-keyboard-no`, `sma` →
> `south-sami-keyboard`. The identifier's last `.`-segment is the layout's
> "folder name", naming its Xcode target and `Keyboard/<folder>` directory.
> Computing an identifier without an iOS target panics.

> [spec:kbdgen:def:ios.pkg-ids.all]
> The package identifier list is the base `packageId` plus the identifier of
> every layout with an `iOS` section (with or without `keyNames`), sorted
> byte-wise with duplicates retained (`se` and `sme` both yield
> `…northern-sami-keyboard` under the legacy table). `ios print-pkg-ids`
> prints it one per line.

## Build chain

> [spec:kbdgen:req:ios.build-chain]
> `ios build` MUST run, per `[spec:kbdgen:sem:pipeline.steps]`: `git clone
> https://github.com/divvun/giellakbd-ios.git repo` in `<out>` (exit status
> ignored); `ios.keyboard-json`; Xcode generation (`xcode.targets`);
> provisioning; `pod install --project-directory=<out>/repo`; and
> `xcodebuild archive` (workspace `GiellaKeyboard.xcworkspace`, scheme
> `HostingApp`, Release, to `<out>/<packageId>.xcarchive`, piped through
> `xcbeautify`) followed by `xcodebuild -exportArchive` to `<out>/ipa` with
> `repo/opts.plist`. Exit statuses of pod and xcodebuild are ignored.
> Provisioning spawns, for every id of `ios.pkg-ids.all`, both `fastlane
> match appstore` and `fastlane sigh` (writing `<id>.mobileprovision`) as
> independent concurrent tasks (at most 12 running); any non-zero exit fails
> with `Failed to download profile '<id>'.`. Each profile's UUID and name
> then set `PROVISIONING_PROFILE`/`PROVISIONING_PROFILE_SPECIFIER` on target
> `HostingApp` (base id) or the id's last segment, and `repo/opts.plist`
> records `{teamID, method: app-store, provisioningProfiles: {id: UUID}}`.

> [spec:kbdgen:req:ios.build-chain.init]
> `ios init` MUST run in `<out>`, with the target's fastlane environment:
> `fastlane produce -a <packageId> --app_name <name>` (name = project
> `locales.en.name`; panic if absent), then `fastlane produce group -g
> group.<packageId> -n "<name> Group"`, then concurrently for every id of
> `ios.pkg-ids.all`: for non-base ids `fastlane produce -a <id> --app_name
> "<name>: <last segment>" --skip_itc`, followed by `fastlane produce
> enable_services -a <id> --app-group` and `fastlane produce associate_group
> -a <id> group.<packageId>`. Exit statuses are ignored; only launch
> failures are errors.

## Xcode project generation

> [spec:kbdgen:req:xcode.targets]
> Xcode generation always reads and rewrites
> `repo/GiellaKeyboard.xcodeproj/project.pbxproj` (`pbxproj.model.read`,
> `pbxproj.serialize`), but only with an iOS target (`targets/ios.yaml`)
> does it, for each iOS layout in bundle layout order: add an `Info.plist`
> reference to group `Keyboard/<folder>` (created as needed); clone native
> target `Keyboard` as `<folder>` with fresh ids, product `<folder>.appex` in
> `Products`, shared build phases, and settings `INFOPLIST_FILE =
> Keyboard/<folder>/Info.plist`, `PRODUCT_NAME = <folder>`,
> `PRODUCT_BUNDLE_IDENTIFIER = <packageId>.<folder>`, `DEVELOPMENT_TEAM =
> <codeSignId>` (`Unknown` if unset, not `teamId`); and embed the appex in
> `HostingApp`'s first `Embed App/Foundation Extensions` phase with
> `ATTRIBUTES = (RemoveHeadersOnCopy, )`. Afterwards `HostingApp` gets the
> base identifier and team, and target `Keyboard` is removed with its
> dependencies, `Products` entry and embed build file.

> [spec:kbdgen:req:xcode.targets.keyboard-plist]
> With an iOS target, each such layout's `repo/Keyboard/<folder>/Info.plist`
> MUST be produced from template `repo/Keyboard/Info.plist` deserialized as
> the typed `KeyboardInfoPlist` (xcode_structures.rs; missing required keys
> panic, keys outside it are dropped) and written as XML with:
> `DivvunSpellerPackageKey`/`DivvunSpellerPath` from `iOS.config` when both
> are set, else both omitted; `DivvunContactEmail` from
> `<bundle>/projects/<tag>.yaml` when readable, else unchanged;
> `CFBundleDisplayName` = `displayNames.en` (panic if absent);
> `CFBundleShortVersionString`/`CFBundleVersion` = target
> `version`/`build`; `LSApplicationQueriesSchemes[0]` = `packageId`;
> `SentryDSN` = `sentryDsn` (omitted if unset); `PrimaryLanguage` = the
> language tag; `DivvunKeyboardIndex` = the layout's 0-based position among
> iOS layouts (counting those without `keyNames`).

> [spec:kbdgen:req:xcode.targets.hosting-app]
> With an iOS target, the generator MUST rewrite, via the typed structs of
> xcode_structures.rs (unknown keys dropped): `repo/HostingApp/Info.plist`
> (`HostingPlist`) with `CFBundleDisplayName` = `bundleName`, version and
> build, `packageId` as first URL scheme and first queries scheme, and
> `SentryDSN`; `Keyboard/Keyboard.entitlements` and
> `HostingApp/HostingApp.entitlements` (`EntitlementsDict`) with app groups
> `["group.<packageId>"]`; and `HostingApp/Settings.bundle/Root.plist`
> (`SettingsRootDict`) with `ApplicationGroupContainerIdentifier =
> group.<packageId>`. With or without the target, it MUST render the iOS
> icon `icon.png` (panic if missing) for every entry of
> `HostingApp/Images.xcassets/AppIcon.appiconset/Contents.json` at N×N
> pixels, N = first `size` number × `scale`, via `convert -resize NxN
> -background transparent -gravity center -extent NxN` into
> `<idiom>-<size>@<scale>.png`, recording it as `filename`.

> [spec:kbdgen:req:xcode.localization]
> With an iOS target, for each iOS layout and each project locale (`en`
> written `Base`), the generator MUST write
> `repo/HostingApp/<locale>.lproj/InfoPlist.strings` as
> `"CFBundleName" = <name>;\n"CFBundleDisplayName" = <name>;` (`name`
> Rust-debug-quoted, no trailing newline). For non-`Base` locales whose
> `repo/HostingApp/<locale>.lproj/About.txt` exists relative to the process
> working directory (not `<out>`), it adds a `<locale>.lproj/About.txt`
> reference to variant group `About.txt` once per layout (so duplicates
> arise). Finally the sorted locale set is added to `knownRegions`, and a new
> `InfoPlist.strings` variant group of `<locale>.lproj/InfoPlist.strings`
> references is added to `HostingApp`'s last resources phase and to group
> `HostingApp/Supporting Files`.
