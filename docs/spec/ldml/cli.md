# `kbdgen ldml` commands

These subcommands export, import, migrate, compile and test layouts. Each
loads the bundle as `cli.commands` does, and fails without writing
anything when loading fails.

Sources: `docs/spec/cli.md`, `docs/spec/bundle.md`.

> [spec:kbdgen:def:ldml.cli.commands]
> The commands are:
>
> - `kbdgen ldml export -b <BUNDLE> -o <OUT>`
> - `kbdgen ldml import -b <BUNDLE> [--force] <XML>…`
> - `kbdgen ldml migrate -b <BUNDLE> [--dry-run] [--report <FILE>]`
> - `kbdgen ldml compile -b <BUNDLE> -o <OUT>`
> - `kbdgen ldml test -b <BUNDLE> [--cldr]`
>
> `export` and `compile` also accept `[--layout <TAG>]… [--host <HOST>]…`,
> which restrict the run to the named layouts and hosts. Without them, the
> run covers every layout in bundle layout order and every host in
> `ldml.yaml.hosts` order. Errors end the process with a non-zero exit.

> [spec:kbdgen:req:ldml.cli.export]
> `export` MUST write `<OUT>/<tag>.<host>.xml` for each v4 layout and each
> host that has a document (`ldml.yaml.lowering`, `ldml.xml.write`).
> Layouts that use `ldml:` are exported per `ldml.xml.ldml-ref`. A v3
> layout is an error suggesting `kbdgen ldml migrate`. Hosts that share a
> model each get their own file, with identical bytes except for the
> `kbdgen:keyboard@host` attribute. The output directory is created with
> its parents.

> [spec:kbdgen:req:ldml.cli.import]
> `import` MUST read each XML file (`ldml.xml.read`) and group the files by
> layout tag. The tag is `kbdgen:keyboard@tag` if present, otherwise the
> `locale` normalised as a BCP 47 tag. For each group it writes
> `layouts/<tag>.yaml` in v4 (`ldml.yaml.import`), merging the host
> variants of one tag. An existing file is an error unless `--force` is
> given. Two files claiming the same tag and host are an error. It prints
> the count of dropped comments and every warning.

> [spec:kbdgen:req:ldml.cli.compile]
> `compile` MUST write `<OUT>/<tag>.<host>.dvkb` (`ldml.model.encoding`)
> for each v4 layout and host. Hosts that share a model get identical
> files. These are the files that host packaging embeds: the Windows
> resource of `ldml.kbdl.model-resource`, and the app bundles of the other
> hosts.

> [spec:kbdgen:req:ldml.cli.test]
> `test` MUST run every `tests/*.yaml` of the bundle (`ldml.test.bundle`).
> With `--cldr` it also runs the vendored CLDR vectors (`ldml.test.cldr`).
> It prints each failing step with the expected and the actual text,
> preedit and action. It exits non-zero when any step fails.
