#!/bin/bash
# Builds plasma-keyboard from the checkout mounted at /src, with its tests,
# and installs it to /usr. The Chinese, Japanese and Korean text composers
# are left out: the check needs none of them.
set -eo pipefail

dnf -y -q install dnf5-plugins
dnf -y -q install cmake ninja-build gcc-c++ extra-cmake-modules gettext python3 \
  qt6-qtbase-devel qt6-qtbase-private-devel qt6-qtdeclarative-devel qt6-qtwayland-devel \
  qt6-qtmultimedia-devel kf6-kcoreaddons-devel kf6-ki18n-devel kf6-kcmutils-devel \
  kf6-kpackage-devel kf6-kconfig-devel kf6-kcrash-devel kf6-kirigami-devel libplasma-devel \
  libxkbcommon-devel wayland-devel wayland-protocols-devel plasma-wayland-protocols-devel
dnf -y -q builddep plasma-keyboard

rm -rf /build && mkdir -p /build && cp -a /src /build/src
python3 -I /check/mock-test.py /build/src/autotests/mockinputmethodcompositor.cpp
cmake -S /build/src -B /build/out -G Ninja -DCMAKE_INSTALL_PREFIX=/usr \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON \
  -DPLASMA_KEYBOARD_LIBPINYIN_ENABLED=OFF -DPLASMA_KEYBOARD_LIBCHEWING_ENABLED=OFF \
  -DPLASMA_KEYBOARD_LIBHANGUL_ENABLED=OFF -DPLASMA_KEYBOARD_ANTHY_ENABLED=OFF
ninja -C /build/out
ninja -C /build/out install
