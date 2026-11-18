# CLI and build pipeline

`kbdgen` is a single binary whose subcommands load a bundle and run one or
more build steps against it.

## Commands

> [spec:kbdgen:def:cli.commands+3]
> The commands are `kbdgen fetch -b|--bundle-path <BUNDLE>` (see
> `bundle.fetch`), `kbdgen tsf -w|--workspace <WORKSPACE> -o|--output-path
> <OUT>`, both options required, which builds the Windows text service from
> the kbdgen workspace without a bundle (`tsf.arch.builds`,
> `tsf.security.signing`), and `kbdgen target -b|--bundle-path <BUNDLE>
> -o|--output-path <OUT> <PLATFORM>`, both options required and given before
> `<PLATFORM>`: `windows`, `chromeos`, `linux`,
> `macos {build [--no-installer] | generate | installer}`,
> `android {build | clone | generate}` or `ios {build | print-pkg-ids | init}`.
> `target` loads the whole bundle first (a load error creates nothing), then
> creates the output directory with its parents and canonicalises it.
> `ios print-pkg-ids` prints each package id on its own line to stdout and
> runs no steps; `ios init` runs the App Store Connect initialisation
> directly. Returned errors end the process with a non-zero exit.

## Pipeline

> [spec:kbdgen:sem:pipeline.steps+3]
> A build step receives the loaded bundle and the canonical output path. A
> full build runs its target's steps strictly in order, awaiting each and
> stopping at the first error; each target's rule lists its sequence.
> `windows`, `chromeos`, `linux` (one step, `linux.output`), `android build`
> and `ios build` run full builds;
> `android clone`/`generate` and `macos generate`/`installer` run that single
> step. `macos build` runs
> the generate step, then the installer step only when `kbdgen` was compiled
> for macOS and `--no-installer` is absent; on a binary compiled for any
> other OS, `macos build` without `--no-installer` logs a warning that the
> installer step is omitted. That is the only host-gated step: the Windows
> DLL build runs on any host, and `macos installer` is not gated.
