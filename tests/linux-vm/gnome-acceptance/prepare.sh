#!/bin/sh
# Run on the host from the repository root: writes build/ with the Linux
# output of a fixture bundle (the layouts of linux.test.golden) and
# ibus-engine-divvun built for aarch64 Linux in Docker.
set -e
A=$(dirname "$(readlink -f "$0")")
B=$A/build
rm -rf "$B"
mkdir -p "$B/bundle.kbdgen/layouts" "$B/bundle.kbdgen/targets" "$B/bundle.kbdgen/resources"
cp crates/kbd-engine/tests/golden/v3/se-NO.yaml "$B/bundle.kbdgen/layouts/se-NO.yaml"
cp src/build/linux/testdata/sme.yaml "$B/bundle.kbdgen/layouts/sme.yaml"
cp src/build/chromeos/testdata/vro-v3.yaml "$B/bundle.kbdgen/layouts/vro.yaml"
printf 'locales:\n  en:\n    name: Divvun keyboards\n    description: Keyboards for the Sámi languages\nauthor: Divvun\ncopyright: Divvun\nemail: test@example.com\norganisation: Divvun\n' \
  > "$B/bundle.kbdgen/project.yaml"
printf 'packageId: no.divvun\nlayoutPrefix: divvun\nversion: 1.0.0\n' > "$B/bundle.kbdgen/targets/linux.yaml"
cargo run -q --bin kbdgen -- target -b "$B/bundle.kbdgen" -o "$B/out" linux
cp crates/kbd-ibus/data/divvun.xml.in "$B/"
docker run --rm -v "$PWD":/src:ro -v kbdgen-acceptance-target:/target \
  -v kbdgen-acceptance-cargo:/usr/local/cargo/registry -v "$B":/build -w /src rust:slim \
  sh -c 'cargo build -q --locked -p kbd-ibus --release --target-dir /target && cp /target/release/ibus-engine-divvun /build/'
