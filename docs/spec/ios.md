# iOS

The iOS target (`kbdgen target -b <bundle> -o <out> ios build`) clones the
`giellakbd-ios` app template into `<out>/repo`, writes a keyboard definitions
JSON file, rewrites the template's Xcode project, plists and icons, provisions
signing material with fastlane, and archives the app with `xcodebuild`.

## iOS layout list

> [spec:kbdgen:req:ios.layouts]
> The iOS layout list is every layout with an `iOS` section, sorted by
> `displayNames.en` lower-cased and then by language tag, both byte-wise.
> Producing it MUST fail when any such layout lacks `keyNames` or
> `displayNames.en`, with `Some iOS layouts are incomplete:` followed by one
> line per defect, indented two spaces and sorted byte-wise:
> ``<tag>: missing `keyNames` (space and return key labels)`` or
> ``<tag>: missing `displayNames.en` ``. `ios.keyboard-json` and
> `xcode.targets` MUST both iterate this list, because a keyboard finds its
> definition by its position in it (`xcode.targets.keyboard-plist`).

## Keyboard definitions JSON

> [spec:kbdgen:def:ios.keyboard-json+1]
> The keyboard definitions file is `<out>/repo/Keyboard/Models/KeyboardDefinitions.json`
> (its directory comes from the cloned template), a pretty-printed JSON array
> with one object per layout of `ios.layouts`, in that order; `[]` is written
> when the bundle has no iOS layout, and nothing is written when
> `ios.layouts` fails. Object keys, in order: `name` (the autonym),
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

> [spec:kbdgen:req:ios.pkg-ids+1]
> A layout's extension bundle identifier MUST be `{packageId}.{ext}`, with
> `ext` the layout's language tag and `packageId` that of the iOS target.
> When `packageId` is exactly `no.uit.giella.keyboards.Sami`, `ext` MUST
> instead come from this legacy table when the tag is listed: `se` and `sme`
> → `northern-sami-keyboard`, `sms` → `skolt-sami-keyboard`, `smn` →
> `inari-sami-keyboard`, `smj-SE` → `julev-sami-keyboard`, `smj-NO` →
> `julev-sami-keyboard-no`, `sma` → `south-sami-keyboard`. The identifier's
> last `.`-segment is the layout's "folder name", naming its Xcode target and
> `Keyboard/<folder>` directory.

> [spec:kbdgen:def:ios.pkg-ids.all+1]
> The package identifier list is the base `packageId` plus the identifier of
> every layout with an `iOS` section (with or without `keyNames`), sorted
> byte-wise with duplicates retained (`se` and `sme` both yield
> `…northern-sami-keyboard` under the legacy table). `ios print-pkg-ids`
> prints it one per line. Producing it MUST fail with an error when the
> bundle has no iOS target.

## Build chain

> [spec:kbdgen:req:ios.build-chain+2]
> `ios build` MUST run, per `[spec:kbdgen:sem:pipeline.steps]`: `git clone
> https://github.com/divvun/giellakbd-ios.git repo` in `<out>`;
> `ios.keyboard-json`; Xcode generation (`xcode.targets`); provisioning;
> `pod install --repo-update --project-directory=<out>/repo` (the refresh
> lets a stale spec index resolve the locked versions); and `xcodebuild
> archive` (workspace `GiellaKeyboard.xcworkspace`, scheme `HostingApp`,
> Release, to `<out>/<packageId>.xcarchive`, piped through `xcbeautify`)
> followed by `xcodebuild -exportArchive` to `<out>/ipa` with
> `repo/opts.plist`. A command that cannot be started or exits non-zero MUST
> fail the step with an error naming it: `git`, `fastlane` and `security`
> run captured, and the error quotes the tail of their stderr (of stdout
> when stderr is empty); `pod`, `xcodebuild` and `xcbeautify` stream their
> output, which the error points to. Provisioning requires `teamId` and
> `appStoreKeyJson`, and archiving `teamId` and `codeSignId`; a missing one
> is an error. Provisioning spawns, for every id of `ios.pkg-ids.all`, both
> `fastlane match appstore` and `fastlane sigh` (writing
> `<id>.mobileprovision`) as independent concurrent tasks (at most 12
> running), awaits them in that order and fails with the first failure.
> Each profile, decoded by `security cms -D -i <id>.mobileprovision`, then
> sets its UUID and name as `PROVISIONING_PROFILE`/
> `PROVISIONING_PROFILE_SPECIFIER` on target `HostingApp` (base id) or the
> id's last segment, and `repo/opts.plist` records `{teamID, method:
> app-store, provisioningProfiles: {id: UUID}}`.

> [spec:kbdgen:req:ios.build-chain.init+1]
> `ios init` MUST run in `<out>`, with the target's fastlane environment:
> `fastlane produce -a <packageId> --app_name <name>` (name = project
> `locales.en.name`), then `fastlane produce group -g group.<packageId> -n
> "<name> Group"`, then concurrently for every id of `ios.pkg-ids.all`: for
> non-base ids `fastlane produce -a <id> --app_name "<name>: <last segment>"
> --skip_itc`, followed by `fastlane produce enable_services -a <id>
> --app-group` and `fastlane produce associate_group -a <id>
> group.<packageId>`. It MUST fail with an error when the bundle has no iOS
> target or no `en` locale, or when a fastlane run cannot be started or
> exits non-zero; such an error names the command and quotes the tail of
> its stderr (of stdout when stderr is empty). A failed run ends its id's
> sequence, and the per-id tasks are awaited in id order, failing with the
> first failure.

## Xcode project generation

> [spec:kbdgen:req:xcode.targets+2]
> Xcode generation always reads and rewrites
> `repo/GiellaKeyboard.xcodeproj/project.pbxproj` (`pbxproj.model.read`,
> `pbxproj.serialize`), but only with an iOS target (`targets/ios.yaml`)
> does it, for each layout of `ios.layouts` in that order: add an `Info.plist`
> reference to group `Keyboard/<folder>` (created as needed); clone native
> target `Keyboard` as `<folder>` with fresh ids, product `<folder>.appex` in
> `Products`, shared build phases, and settings `INFOPLIST_FILE =
> Keyboard/<folder>/Info.plist`, `PRODUCT_NAME = <folder>`,
> `PRODUCT_BUNDLE_IDENTIFIER = <packageId>.<folder>`, `DEVELOPMENT_TEAM =
> <teamId>` (`Unknown` if unset); and embed the appex in `HostingApp`'s
> first `Embed App/Foundation Extensions` phase with `ATTRIBUTES =
> (RemoveHeadersOnCopy, )`. Afterwards `HostingApp` gets the base identifier
> and team, and target `Keyboard` is removed with its dependencies,
> `Products` entry and embed build file. These steps MUST fail with an error
> when the project lacks a target, configuration list, build configuration,
> product or embed phase they use, or the `Products` group.

> [spec:kbdgen:req:xcode.targets.keyboard-plist+2]
> With an iOS target, each such layout's `repo/Keyboard/<folder>/Info.plist`
> MUST be produced from template `repo/Keyboard/Info.plist` deserialized as
> the typed `KeyboardInfoPlist` (xcode_structures.rs; keys outside it are
> dropped; a template that does not deserialize, or has no
> `LSApplicationQueriesSchemes` entry, is an error) and written as XML with:
> `DivvunSpellerPackageKey`/`DivvunSpellerPath` from `iOS.config` when both
> are set, else both omitted; `DivvunContactEmail` from
> `<bundle>/projects/<tag>.yaml` when it exists and parses as a project
> (when it exists but cannot be read or parsed, a warning is logged), else
> unchanged;
> `CFBundleDisplayName` = `displayNames.en`;
> `CFBundleShortVersionString`/`CFBundleVersion` = target
> `version`/`build`; `LSApplicationQueriesSchemes[0]` = `packageId`;
> `SentryDSN` = `sentryDsn` (omitted if unset); `PrimaryLanguage` = the
> language tag; `DivvunKeyboardIndex` = the layout's 0-based position in
> `ios.layouts`, which is also its position in the keyboard definitions file.

> [spec:kbdgen:req:xcode.targets.hosting-app+2]
> With an iOS target, the generator MUST rewrite, via the typed structs of
> xcode_structures.rs (unknown keys dropped; a file that does not
> deserialize is an error): `repo/HostingApp/Info.plist` (`HostingPlist`)
> with `CFBundleDisplayName` = `bundleName`, version and build, `packageId`
> as first URL scheme and first queries scheme (an error when either list is
> empty), and `SentryDSN`; `Keyboard/Keyboard.entitlements` and
> `HostingApp/HostingApp.entitlements` (`EntitlementsDict`) with app groups
> `["group.<packageId>"]`; and `HostingApp/Settings.bundle/Root.plist`
> (`SettingsRootDict`) with `ApplicationGroupContainerIdentifier =
> group.<packageId>`. With or without the target, it MUST render the default
> icon of `resources/ios` (`icon.<ext>`, `bundle.resources`) for every entry of
> `HostingApp/Images.xcassets/AppIcon.appiconset/Contents.json` at N×N
> pixels, N = first `size` number × the `scale` number before its `x`, via
> `convert -resize NxN -background transparent -gravity center -extent NxN`
> into `<idiom>-<size>@<scale>.png`, recording it as `filename`. A missing
> `resources/ios` or icon, an unreadable `Contents.json`, `size` or `scale`,
> or a `convert` that cannot be started or exits non-zero MUST be an error;
> for `convert` it names the command and quotes the tail of its stderr.

> [spec:kbdgen:req:xcode.localization+1]
> With an iOS target, for each project locale (`en` written `Base`), the
> generator MUST write `repo/HostingApp/<locale>.lproj/InfoPlist.strings` as
> `"CFBundleName" = <name>;\n"CFBundleDisplayName" = <name>;` (`name`
> Rust-debug-quoted, no trailing newline). For each non-`Base` locale whose
> `<out>/repo/HostingApp/<locale>.lproj/About.txt` exists, it adds a
> `<locale>.lproj/About.txt` reference to variant group `About.txt` (an
> error if the project has none) unless the group already has a reference
> with that path. Finally the sorted locale set is added to `knownRegions`,
> and a new `InfoPlist.strings` variant group of
> `<locale>.lproj/InfoPlist.strings` references is added to `HostingApp`'s
> last resources phase (an error if it has none) and to group
> `HostingApp/Supporting Files`.
