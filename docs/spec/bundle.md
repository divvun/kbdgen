# Bundle model

A `.kbdgen` bundle is the directory every `kbdgen` command reads. This
document pins how it is loaded and how `kbdgen fetch` pulls layouts in from
dependency bundles.

## Structure

> [spec:kbdgen:def:bundle.structure]
> A bundle is a directory, conventionally `<name>.kbdgen`, holding
> `project.yaml` and the directories `layouts/`, `targets/` and `resources/`.
> Loading canonicalises the path, then reads those four in that order; any
> missing or unreadable one aborts with an I/O error (for `project.yaml` the
> error names the bundle path). The bundle name is the file stem of the
> canonical path (`sme.kbdgen` → `sme`). In `layouts/` and `targets/` only
> regular files with extension exactly `yaml` are read; others (including
> `.yml`) are skipped. An optional `projects/` directory is ignored by loading;
> `kbdgen fetch` fills it and the iOS generator reads it.

> [spec:kbdgen:def:bundle.project]
> `project.yaml` is a mapping with required `locales`, `author`, `copyright`,
> `email`, `organisation` and optional `dependencies` (default empty); unknown
> fields are ignored and any scalar is accepted as a string. `locales` is an
> ordered map from locale string to {`name`, `description`}, both required.
> `dependencies` is an ordered map from dependency id to {`url` (GitHub
> `owner/repo`), `layouts` (list of tags), optional `branch`}.

### Target files

> [spec:kbdgen:req:bundle.structure.targets]
> A file in `targets/` is recognised by its stem, exactly `windows`, `ios`,
> `macos`, `chromeos` or `android`; other stems are warned about and ignored.
> An absent file leaves the target unconfigured; a parse failure aborts
> loading; unknown fields are ignored.
>
> | File | Required | Optional |
> |---|---|---|
> | `windows` | `appName` `version` `url` `uuid` `build` (unused) | — |
> | `macos` | `codeSignId` `packageId` `bundleName` `version` `build` | — |
> | `ios` | `packageId` `bundleName` `version`, `build` (unsigned int) | `codeSignId` `teamId` `provisioningProfileId` `sentryDsn` `appStoreKeyJson` `matchGitUrl` `matchPassword` `fastlaneUser` `fastlanePassword` |
> | `chromeos` | `appId` `build` `version` | — |
> | `android` | `packageId` `version`, `build` (unsigned int) | `keyStore` `keyAlias` `playStoreAccount` `playStoreP12` `storePassword` `keyPassword` |
>
> Other fields are strings. `windows`, `macos`, `chromeos` accept any scalar
> as a string (`version: 1.0` → `"1.0"`); `ios` and `android` are parsed via a
> YAML value, so their strings MUST be YAML strings and their top level MUST
> be a mapping (else a panic).

> [spec:kbdgen:req:bundle.structure.targets.env]
> When `ios.yaml` or `android.yaml` exists, each variable below that is set
> to valid Unicode (empty included) MUST replace the named top-level field
> with its value as a YAML string; unset variables leave the file value.
>
> | Target | Variable → field |
> |---|---|
> | iOS | `MATCH_GIT_URL`→`matchGitUrl`, `MATCH_PASSWORD`→`matchPassword`, `FASTLANE_USER`→`fastlaneUser`, `PRODUCE_USERNAME`→`fastlaneUser`, `FASTLANE_PASSWORD`→`fastlanePassword`, `APP_STORE_KEY_JSON`→`appStoreKeyJson`, `TEAM_ID`→`teamId`, `CODE_SIGN_ID`→`codeSignId` |
> | Android | `ANDROID_KEYSTORE`→`keyStore`, `ANDROID_KEYALIAS`→`keyAlias`, `PLAY_STORE_ACCOUNT`→`playStoreAccount`, `PLAY_STORE_P12`→`playStoreP12`, `STORE_PW`→`storePassword`, `KEY_PW`→`keyPassword` |
>
> Overrides apply in hash-map order: with both `FASTLANE_USER` and
> `PRODUCE_USERNAME` set, the winner is unspecified.

## Layouts

> [spec:kbdgen:req:bundle.layouts+1]
> Each `layouts/*.yaml` stem MUST be a well-formed BCP 47 tag (normalised,
> `se-fi` → `se-FI`), else loading fails with `InvalidLanguageTag`. The file's
> top level MUST be a mapping (else, empty files included, a panic: "top level
> yaml type must be a mapping"); the tag is inserted as `languageTag` and the
> value deserialised as a layout. A `decimal` other than `.` or `,` is logged
> as an error and replaced by `.`. Files are loaded in ascending byte-wise
> file-name order; the first failing layout aborts loading, and when two
> stems normalise to the same tag the later file replaces the earlier.
> Layouts MUST be iterated in "bundle layout order": ascending by the
> normalised tag's string form, compared byte-wise (`se` < `se-FI` < `sma` <
> `sme`), identically on every run.

> [spec:kbdgen:req:bundle.layouts.autonym]
> A layout's `displayNames` MUST contain the autonym: an entry keyed by the
> primary language subtag of its tag (`se` for `se-FI`); an entry under the
> full tag alone does not count. Keys compare as normalised tags. Otherwise
> loading fails with `MissingMandatoryDisplayName` naming the tag.

## Resources

> [spec:kbdgen:def:bundle.resources]
> `resources/` holds per-target subdirectories named exactly `macos`, `ios`,
> `chromeos`, `android`; other subdirectories are warned about and ignored,
> plain files ignored, and an unreadable recognised directory leaves that
> target without resources. For `macos`, `ios`, `chromeos`, each entry named
> `icon.*` is an icon keyed by the language tag in the second `.`-separated
> segment (`icon.se.png` → `se`; `icon.png` → `png`, the key iOS looks up).
> A segment that is not a well-formed tag (`icon.se_FI.png`) panics; among
> entries sharing a tag the survivor is unspecified. For `android` the only
> resource is `icon.png`, if present.

## Fetch

> [spec:kbdgen:req:bundle.fetch]
> `kbdgen fetch` loads the bundle, creates `<bundle>/layouts/` and
> `<bundle>/projects/`, then for each dependency in order (branch defaulting
> to `main`) MUST download `https://github.com/{url}/archive/{branch}.zip`
> (status unchecked; a non-zip body fails as a zip error), extract it to a
> temporary directory and use `{repo}-{branch'}/{id}.kbdgen` as the source,
> where `repo` is the second `/`-segment of `url` (no `/` panics) and
> `branch'` has `/` replaced by `-`. For each listed layout `L` it copies
> `layouts/L.yaml` to `<bundle>/layouts/L.yaml` and the source `project.yaml`
> to `<bundle>/projects/L.yaml`, overwriting. Any error aborts; earlier copies
> remain.
