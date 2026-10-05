# Windows

The Windows target generates, for each layout's `windows` section, the Rust
crate and resources of a keyboard layout DLL ([kbdl.md](kbdl.md)). The rules
below describe the MSKLC-based DLL build, which the Rust build of
[kbdl.md](kbdl.md) replaces.

## DLL build

> [spec:kbdgen:req:windows.dll]
> The Windows build runs KLC generation, then the DLL step only when kbdgen
> is compiled for Windows (otherwise two warnings are logged).
> The DLL step MUST:
> 1. Fail if any of the `x64`, `x86` or `arm64` MSVC environments is invalid.
> 2. Install the pahkat `msklc` package (`nightly`, from
>    `https://pahkat.uit.no/devtools/`) into
>    `<app-data>/kbdgen/prefix/windows`; a download error exits with status 1.
> 3. For every `*.klc` in the output directory (sorted, including stale files)
>    and each architecture in that order, in `<out>/<arch>/build/<stem>/` run
>    `kbdutool.exe -n -s -u`, then `cl.exe`, `rc.exe` and `link.exe` under
>    that architecture's environment, and move `<stem>.dll` to
>    `<out>/<arch>/<stem>.dll`.
>
> The first command that fails to start or exits unsuccessfully MUST abort
> the build with an error naming the stage and the exit status.

> [spec:kbdgen:req:windows.dll.link]
> The link step MUST produce a native-subsystem, entry-less keyboard DLL whose
> descriptor, code and tables all live in one read-only executable section,
> to match Microsoft's keyboard-layout samples. To do so it MUST pass
> `-nologo -SECTION:INIT,D -OPT:REF -OPT:ICF -IGNORE:4039,4078 -noentry -dll
> -subsystem:native,5.0 -merge:.edata=.data -merge:.rdata=.data
> -merge:.text=.data -merge:.bss=.data -section:.data,re -STACK:0x40000,0x1000
> -osversion:4.0 -version:4.0 /release`. The compile step MUST pass `/MD /c
> /Zp8 /Gy /W3 /WX /Gz /Gm- /EHs-c- /GR- /GF -Z7 /Oxs` together with the
> fixed `-D` define set in src/build/windows/build_klc.rs `cl_command`.
