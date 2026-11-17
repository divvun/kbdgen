# Android

The Android target patches a clone of the `giellakbd-android` app with one
keyboard for each bundle layout that has an `android` section. Paths are
relative to `{out}/repo/app/src/main` unless noted. `{out}` is the
canonicalised `-o` directory.

## Toolchain

> [spec:kbdgen:req:android.dependencies+1]
> The `target android build` steps are listed in
> `[spec:kbdgen:sem:pipeline.steps]`. The clone step MUST run
> `git clone https://github.com/divvun/giellakbd-android repo` with `{out}`
> as the working directory. If `git` cannot be started, the step fails. A
> non-zero exit is not an error when `{out}/repo` is a directory, for
> example a checkout left by an earlier run, and later steps reuse whatever
> checkout is there. Otherwise a non-zero exit is an error naming the
> command and including its stderr.

> [spec:kbdgen:req:android.dependencies.jnilibs+1]
> The dependency step MUST extract two release assets into
> `{out}/repo/app/src/main`, in this order:
> 1. from `divvun/divvunspell`, the asset whose name contains
>    `android-jnilibs`;
> 2. from `divvun/pahkat`, the asset whose name contains `android`.
>
> Matching is a case-insensitive substring test. For each repo, the step
> sends an unauthenticated
> `GET https://api.github.com/repos/{org}/{repo}/releases?per_page=100` with
> `User-Agent: kbdgen-rust-client`. It skips draft and prerelease releases
> and takes the first matching asset of the newest release that has one.
>
> The step fails with an error in these cases:
> - a request fails or answers with a non-success HTTP status, for
>   example a rate-limit refusal;
> - the release list is not a JSON array:
>   `the release list of {org}/{repo} is not a JSON array`;
> - no release matches: `no {filter} asset found in any release for {org}/{repo}`;
> - the matching asset has no `browser_download_url`;
> - `tar -xzf <tmp> -C <main>` cannot be started or exits non-zero. The
>   error names the asset and includes tar's stderr.

## Layout XML

> [spec:kbdgen:def:android.layout-xml]
> The Android layout XML is the resource set written per layout, named by
> the sanitised keyboard name `{name}`:
> - `rows_{name}.xml` and `rowkeys_{name}{n}.xml`, in both `res/xml` (from
>   `primary`) and `res/xml-sw600dp` (from `tablet-600`);
> - `res/xml/kbd_{name}.xml`, which contains one
>   `<include latin:keyboardLayout="@xml/rows_{name}"/>`;
> - `res/xml/keyboard_layout_set_{name}.xml`.
>
> The layout set contains these `<Element>` entries, in order:
> - `alphabet` (with `latin:enableProximityCharsCorrection="true"`),
>   `alphabetAutomaticShifted`, `alphabetManualShifted`,
>   `alphabetShiftLocked` and `alphabetShiftLockShifted`, all pointing to
>   `@xml/kbd_{name}`;
> - `symbols`, `symbolsShifted`, `phone`, `phoneSymbols` and `number`,
>   pointing to `@xml/kbd_symbols`, `kbd_symbols_shift`, `kbd_phone`,
>   `kbd_phone_symbols` and `kbd_number`.
>
> Tokens are whitespace-split from each layer line and emitted undecoded.

> [spec:kbdgen:def:android.layout-xml.name+1]
> The sanitised keyboard name is derived from `displayNames.en`; generation
> fails with `{tag}: displayNames has no 'en' entry` if that entry is
> missing. The steps are:
> 1. Lowercase it.
> 2. Replace each space and `-` with `_`.
> 3. Delete `(` and `)`.
> 4. Replace every character outside `[a-z0-9]` with `x`.
>
> Step 4 also replaces the underscores added in step 2. For example,
> `Northern Sami (Norway)` becomes `northernxsamixnorway`. Layouts whose
> names collide overwrite each other's files.

> [spec:kbdgen:req:android.layout-xml.rows]
> An `android` section MUST have both `primary` and `tablet-600` platforms,
> and each platform MUST have a `default` layer. For each platform, line `i`
> of the `default` layer goes into the `<default>` element of
> `rowkeys_{name}{i+1}.xml`, and line `i` of the `shift` layer goes into its
> `<case latin:keyboardLayoutSetElement="alphabetManualShifted|alphabetShiftLocked|alphabetShiftLockShifted">`
> element. The key width `w` is `100/L` on phone and `90/L` on tablet, where
> `L` is the longest token count of any `default` line, formatted as the
> shortest round-trip `f64` (for example `10` or `9.090909090909092`). The
> rows file inserts one
> `<Row><include latin:keyboardLayout="@xml/rowkeys_{name}{i+1}" latin:keyWidth="{w}%p"/></Row>`
> per line, in line order, between the template's `key_styles_common` and
> `row_qwerty4` includes.

## Keys

> [spec:kbdgen:req:android.keys+1]
> Each token MUST become a `<Key>` element.
>
> On lines 1 and later:
> - `\s{shift}` gets `latin:keyStyle="shiftKeyStyle"` and
>   `latin:keyWidth="{f:.2}%"`, where `f = (100 − w·(N−S))/S`. `N` is the
>   number of tokens on the line and `S` the number of tokens starting with
>   `\s`. The base is 100 even on tablet.
> - `\s{backspace}` gets `deleteKeyStyle` and `keyWidth="fillRight"`.
> - Every other token, including other `\s{…}` tokens, gets
>   `latin:keySpec` set to the raw token. A lone `\` becomes `\\`.
>   - `latin:keyLabelFlags="preserveCase"` is added only when the language
>     tag is exactly `lut`.
>   - A `longpress` entry for the exact token adds `latin:keyHintLabel`, set
>     to its first value, and `latin:moreKeys`, set to the values joined
>     with `,`. An empty list is an error naming the layout, the platform,
>     the layer, the 1-based line and the token.
>
> On every line, a token equal to a top-level `transforms` key also gets
> `latin:deadKey="True"`.

> [spec:kbdgen:req:android.keys.number-row]
> On line 0 of every layer and platform:
> - `\s{shift}` and `\s{backspace}` get only their `keyStyle`.
> - Every other token at 0-based position `k` (special tokens included in
>   the count) gets `keySpec` and, where it applies, `preserveCase`.
>   - For `k ≤ 9`, it also gets `latin:keyHintLabel` and
>     `latin:additionalMoreKeys`, both set to the digit `(k+1) mod 10`.
>   - A longpress entry adds `latin:moreKeys` only; it never sets the hint
>     label.
>
> No line-0 key has a `keyWidth`.

## Transforms

> [spec:kbdgen:req:android.transforms+1]
> Transforms are interpreted per `[spec:kbdgen:req:layout.transforms.dead-key-entries]`.
> Flattening runs for every layout in the bundle before the `android` check,
> so a nested transform in a layout that has no `android` section still
> panics `target android generate`. A top-level leaf still counts as a dead
> key and gets an empty inner map. For each Android layout, kbdgen MUST
> write `assets/layouts/{languageTag}.json` as pretty JSON:
> - with `android.config`:
>   `{"transforms": {deadKey: {next: output}}, "speller": {"path": spellerPath, "package_url": spellerPackageKey}}`.
>   A missing `spellerPath` or an unparseable `spellerPackageKey` is an
>   error naming the layout. When `spellerPackageKey` is absent,
>   `package_url` is `null`.
> - without `android.config`: `{"transforms": {}, "speller": null}`. The
>   transforms are dropped, but `deadKey` marking still applies.

## Metadata

> [spec:kbdgen:req:android.metadata+1]
> `res/xml/method.xml` and `res/xml/spellchecker.xml` MUST already exist,
> each with at least one `<subtype>`; otherwise generation fails with an
> error naming the file. The first `<subtype>` of each file is removed.
> Then, in bundle layout order, each Android layout appends one subtype to
> each file. `{sub}` is the language tag with `-`→`_`, lowercased.
> - `method.xml`:
>   `<subtype android:icon="@drawable/ic_ime_switcher_dark" android:imeSubtypeMode="keyboard" android:label="@string/subtype_{sub}" android:imeSubtypeLocale="{tag, -→_}" android:imeSubtypeExtraValue="KeyboardLayoutSet={name},AsciiCapable,EmojiCapable" android:isAsciiCapable="true"/>`
> - `spellchecker.xml`:
>   `<subtype android:label="@string/subtype_{sub}" android:subtypeLocale="{tag}"/>`
>
> Each run removes one subtype but appends one per layout, so re-running on
> the same clone duplicates subtypes. If no layout has an `android` section,
> kbdgen only logs a warning and every step still runs.

> [spec:kbdgen:req:android.metadata.strings]
> For each Android layout, kbdgen MUST:
> - set the `string[name="english_ime_name"]` element of
>   `res/values/strings-appname.xml` to `{displayNames.en} Keyboard`. The
>   last layout wins.
> - append `<string name="subtype_{sub}">{displayNames.en}</string>` to
>   `res/values/strings.xml`.
> - for each `displayNames` entry `(t, n)` where `res/values-{t}` existed
>   before generation, append `<string name="subtype_{sub}">{n}</string>` to
>   `res/values-{t}/strings.xml`. The file is created if missing, and the
>   string is skipped if one with that name already exists.
>
> Afterwards, for each `project.yaml` locale `l` whose `res/values-{l}`
> existed, kbdgen appends
> `<string name="english_ime_name">{locales[l].name}</string>` to
> `res/values-{l}/strings-appname.xml`, creating the file if missing. Only
> the per-locale `strings.xml` entries are deduplicated; the other appends
> repeat on every run.

> [spec:kbdgen:req:android.metadata.icons+1]
> If `resources/android/icon.png` exists, kbdgen MUST run the following
> command for each density `(mdpi 48, hdpi 72, xhdpi 96, xxhdpi 144, xxxhdpi 192)`:
> `convert -resize {d}x{d} <icon> res/drawable-{density}/ic_launcher_keyboard.png`.
> If `convert` cannot be started or exits non-zero, generation fails with
> an error naming the icon and the size and including convert's stderr.
> Without the icon, kbdgen logs
> `No resources/android/icon.png; keeping the app's launcher icon` and
> leaves the app's icons unchanged.

## Gradle

> [spec:kbdgen:req:android.gradle+1]
> When `targets/android.yaml` exists, kbdgen MUST write
> `{out}/repo/app/local.gradle` containing
> `ext.app = [ storeFile, keyAlias, storePassword, keyPassword, packageName, versionCode, versionName, playEmail, playCredentials ]`,
> one `key: value,` per line with four-space indent. Every value is
> double-quoted except `versionCode` (`build`).
> - `storeFile` is the canonicalised `keyStore` path with `\` doubled. It is
>   empty if `keyStore` is absent or cannot be canonicalised. A canonical
>   path that is not valid UTF-8 is an error.
> - The key alias, the passwords, `playStoreAccount` and `playStoreP12` have
>   `"`→`\"` and are empty if absent.
> - `packageId` and `version` are inserted verbatim.
> - Every `$` in the final text becomes `\$`.
>
> kbdgen then runs `gradlew assembleRelease -Dorg.gradle.jvmargs=-Xmx4096M --info --stacktrace`
> in `{out}/repo`: `cmd /C gradlew …` on Windows, elsewhere the
> canonicalised `{out}/repo/gradlew`, which MUST exist. The output is
> captured and logged only at trace level. If gradlew cannot be started or
> exits non-zero, generation fails with an error naming the command and
> including its stderr. This also happens on `target android generate`.
> Without the target file, kbdgen logs
> `No targets/android.yaml; skipping local.gradle and the Gradle build` and
> skips both.
