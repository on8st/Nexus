#!/usr/bin/env bash
# Nexus — native macOS build, producing a .app + .dmg for WHICHEVER architecture this script
# runs on (Apple Silicon or Intel). It does not cross-arch or lipo a universal binary — see
# MACOS.md for why, and run this once per arch (Apple Silicon + Intel runners in CI) if you
# want to ship both.
#
#   ./scripts/build-macos.sh            # UI + native modem + Tauri .app/.dmg
#   ./scripts/build-macos.sh --no-gui   # native modem test exes only (fast)
#   ./scripts/build-macos.sh --yaesu-wf # + the FT-710 FT4222 waterfall (FORK-LOCAL, see below)
#
# --yaesu-wf needs FTDI's LibFT4222 (which bundles D2XX) on this machine — a manual download from
# ftdichip.com, because their site is behind a JavaScript challenge and because the library is NOT
# redistributable, so it is never vendored into this repo. Point FT4222_LIB at the directory holding
# libft4222*.dylib, or drop it in ~/.local/lib/ft4222 which is checked by default:
#
#   FT4222_LIB=~/.local/lib/ft4222 ./scripts/build-macos.sh --yaesu-wf
#
# ⚠️ The resulting build links a closed-source library against a GPL-3.0-only app. That question is
# open (see FORK.md), so a build made this way is for THIS machine and is never shipped.
#
# One-time dev deps (Homebrew; the script checks and names anything missing):
#   xcode-select --install
#   brew install cmake ninja gcc fftw boost node hamlib   # `gcc` is what provides gfortran —
#                                                          # there is no standalone `gfortran`
#                                                          # formula, and macOS ships no system one.
#   + rustup (https://rustup.rs); cargo-tauri is auto-installed if absent.
#
# Native build uses Homebrew's FFTW3f via pkg-config — no cross FFTW needed, same as Linux.
# The Tauri shell uses the OS-provided WKWebView (no separate runtime to bundle, unlike
# Windows' WebView2). CAT uses the SYSTEM Hamlib (`brew install hamlib`) — same model as
# Linux: Nexus looks for `rigctld` on PATH, and nothing is bundled into the .app the way the
# Windows installer bundles rigctld.exe (there is no equivalent prebuilt Hamlib archive for
# macOS to stage the way fetch-hamlib.sh stages the Windows one).
set -euo pipefail

bold() { printf '\n\033[1m%s\033[0m\n' "$*"; }
ok()   { printf '  \033[32m✓\033[0m %s\n' "$*"; }
warn() { printf '  \033[33m!\033[0m %s\n' "$*"; }
die()  { printf '\n\033[31m✗ %s\033[0m\n' "$*" >&2; exit 1; }

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Official-build secrets (e.g. CLUBLOG_API_KEY, baked via option_env!) live OUTSIDE the repo.
# shellcheck disable=SC1091
[ -f "$HOME/.nexus-build.env" ] && source "$HOME/.nexus-build.env"

GUI=1
YAESU_WF=0
for a in "$@"; do
  case "$a" in
    --no-gui) GUI=0 ;;
    --yaesu-wf) YAESU_WF=1 ;;
    -h|--help) sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option: $a" ;;
  esac
done

ARCH="$(uname -m)" # arm64 (Apple Silicon) or x86_64 (Intel) — whatever this Mac actually is.

# Validate the FT4222 preconditions EARLY. They used to sit next to the cargo invocation, which
# meant a missing library was reported after the whole UI and modem build had already run — three
# minutes to learn something knowable in the first second.
FEATURES="radio,custom-protocol"
if [ "$YAESU_WF" = 1 ]; then
  # Fail HERE with a sentence the operator can act on, rather than 300 lines into a link error.
  FT4222_LIB="${FT4222_LIB:-$HOME/.local/lib/ft4222}"
  [ -d "$FT4222_LIB" ] || die "--yaesu-wf: no such directory: $FT4222_LIB
  FTDI's LibFT4222 is a manual download (ftdichip.com, FT4222H software examples) and is never
  vendored here. Unpack it and set FT4222_LIB, or put the dylibs in ~/.local/lib/ft4222."
  lib="$(ls "$FT4222_LIB"/libft4222*.dylib 2>/dev/null | head -1 || true)"
  [ -n "$lib" ] || die "--yaesu-wf: no libft4222*.dylib in $FT4222_LIB (found: $(ls "$FT4222_LIB" 2>/dev/null | tr '\n' ' '))"
  # An x86_64-only library cannot link against an arm64 build, and the linker's own message for that
  # is famously unhelpful — so check the architecture we are actually building for.
  if ! lipo -archs "$lib" 2>/dev/null | tr ' ' '\n' | grep -qx "$ARCH"; then
    die "--yaesu-wf: $(basename "$lib") is $(lipo -archs "$lib" 2>/dev/null) but this build is $ARCH.
  FTDI ship per-architecture builds; you need the one matching $ARCH (or build Nexus for the arch you have)."
  fi
  FEATURES="$FEATURES,yaesu-wf"
  export RUSTFLAGS="${RUSTFLAGS:-} -L $FT4222_LIB"
  # The .app finds the dylib at RUN time too — an unsigned local build gets it from the environment.
  export DYLD_LIBRARY_PATH="${DYLD_LIBRARY_PATH:+$DYLD_LIBRARY_PATH:}$FT4222_LIB"
  bold "FT-710 waterfall: ON (linking $(basename "$lib") from $FT4222_LIB)"
  warn "This build links FTDI's closed-source library — local use only, never shipped. See FORK.md."
fi

# 1 — toolchain + Homebrew libraries -----------------------------------------------------------
bold "1/4  Toolchain + Homebrew libraries ($ARCH)"
xcode-select -p >/dev/null 2>&1 || die "Xcode Command Line Tools missing — run: xcode-select --install"
miss=()
for t in cc gfortran cmake node npm; do command -v "$t" >/dev/null || miss+=("$t"); done
command -v ninja >/dev/null || command -v make >/dev/null || miss+=("ninja-or-make")
[ "${#miss[@]}" -eq 0 ] || die "missing tools: ${miss[*]}
  Homebrew: brew install cmake ninja gcc node   (gcc provides gfortran)"
command -v cargo >/dev/null || die "Rust not found — install from https://rustup.rs"

# Homebrew's prefix is arch-dependent (/opt/homebrew on Apple Silicon, /usr/local on Intel), and
# whichever pkg-config comes first on PATH may know about neither — MacPorts' /opt/local one
# doesn't, and a shell that never ran `brew shellenv` leaves PKG_CONFIG_PATH unset entirely.
# Point pkg-config and CMake at the brew prefix explicitly instead of trusting the environment;
# an unset PKG_CONFIG_PATH is what MACOS.md's "can't find fftw3f/Boost" note is really about, and
# without this the check below reports "fftw3f missing" at a machine where fftw is installed.
if command -v brew >/dev/null 2>&1; then
  BREW_PREFIX="$(brew --prefix)"
  export PKG_CONFIG_PATH="$BREW_PREFIX/lib/pkgconfig:$BREW_PREFIX/opt/fftw/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
  export CMAKE_PREFIX_PATH="$BREW_PREFIX${CMAKE_PREFIX_PATH:+:$CMAKE_PREFIX_PATH}"
fi
pkg-config --exists fftw3f 2>/dev/null || die "fftw3f not found by pkg-config ($(command -v pkg-config))
  Install it with 'brew install fftw'. If it IS installed, pkg-config is looking in the wrong
  place — check 'brew --prefix'/lib/pkgconfig is on PKG_CONFIG_PATH (currently: ${PKG_CONFIG_PATH:-<unset>})."
GEN=Ninja; command -v ninja >/dev/null || GEN="Unix Makefiles"
ok "cc/gfortran/cmake ($GEN)/node, system FFTW3f ($ARCH)"

# The DeepCW AI CW model (AGPL-3.0, (c) e04) is NOT committed — same gate as build-linux.sh /
# build-windows.sh. Fail loudly rather than silently ship a build with no AI CW decoder.
if [ "$GUI" = 1 ]; then
  dcw="$REPO/src-tauri/resources/deepcw"
  for f in model.onnx model.onnx.json; do
    [ -s "$dcw/$f" ] || die "missing $dcw/$f — the DeepCW model is gitignored and must be staged
  before bundling, or the build silently ships without the AI CW decoder.
  See src-tauri/resources/deepcw/README.md for provenance and how to fetch/fold it."
  done
  ok "DeepCW model staged ($(du -h "$dcw/model.onnx" | cut -f1))"
fi

# 2 — libtempo native modem test exes (system FFTW3f via pkg-config, same as Linux) -------------
bold "2/4  libtempo native modem test exes"
cmake -S "$REPO/libtempo" -B "$REPO/libtempo/build-macos" -G "$GEN" -DCMAKE_BUILD_TYPE=Release \
  ${WX:+-DWX="$WX"} >/dev/null
cmake --build "$REPO/libtempo/build-macos" >/dev/null
for e in tempodeep_test_standalone roundtrip tempofast_test_standalone acquire; do
  [ -f "$REPO/libtempo/build-macos/$e" ] && ok "$e" || warn "$e not produced"
done

if [ "$GUI" = 0 ]; then bold "Modem exes done (--no-gui)."; exit 0; fi

# 3 — UI build deps ------------------------------------------------------------------------------
bold "3/4  Web UI dependencies"
# `npm ci`, NOT `npm install` — the same command every release.yml/ci.yml job already uses.
# `npm install` REWRITES package-lock.json to whatever the local npm believes: an npm older than
# the one that wrote the lock silently drops the `libc` ("glibc"/"musl") fields from the optional
# Linux rollup/esbuild binaries (69 deletions with npm 11.9.0 on 2026-08-13). That is npm's own
# selector for which native binary a glibc-vs-musl Linux box installs, so a macOS build could
# quietly degrade Linux installs if the rewrite were ever committed — and short of that it just
# leaves every macOS builder with a dirty tree after an ordinary build. `npm ci` installs the
# lockfile exactly as committed and never writes to it, so the build cannot change the source
# tree. It also fails loudly when package.json and the lock disagree, which is the right outcome
# for a release build rather than silently resolving something new.
( cd "$REPO/ui" && npm ci >/dev/null )
ok "ui/node_modules"

# 4 — the GUI app + .app + .dmg ------------------------------------------------------------------
bold "4/4  Nexus GUI app + .app + .dmg"
cargo tauri --version >/dev/null 2>&1 || { warn "installing tauri-cli…"; cargo install tauri-cli --version "^2" --locked; }
[ -f "$REPO/src-tauri/icons/128x128.png" ] || python3 "$REPO/scripts/gen-icons.py"
# macOS uses the SYSTEM Hamlib (rigctld on PATH via `brew install hamlib`), so don't ship the
# Windows-only hamlib binaries in the .app bundle. The Windows build re-stages the real
# binaries via fetch-hamlib.sh, so removing them here is safe — see build-linux.sh for why
# this deletes individual files rather than the directory (the tracked LGPL license texts
# live in the same folder and must survive this step).
find "$REPO/src-tauri/resources/hamlib" -type f \
  \( -name '*.dll' -o -name '*.exe' -o -name '*.lib' -o -name '*.def' \) -delete
if git -C "$REPO" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git -C "$REPO" diff --quiet -- src-tauri/resources/hamlib || \
    die "build-macos.sh modified TRACKED files under src-tauri/resources/hamlib — refusing to
  continue. Those are the LGPL license texts Hamlib requires us to distribute; restore with
  'git checkout -- src-tauri/resources/hamlib/'."
fi
# `createUpdaterArtifacts` is on and tauri.conf.json carries the updater PUBLIC key, so Tauri
# emits a .app.tar.gz self-update payload and then fails the whole build signing it unless
# TAURI_SIGNING_PRIVATE_KEY is set — which it is in CI (a secret) and is not for anyone building
# from source. Without this the .app and .dmg both build fine and the script still exits 1 on
# the very last step. A local build has no use for an updater payload it cannot sign, so drop
# it when there's no key; a real build failure still fails.
updater_cfg=()
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  updater_cfg=(--config '{"bundle":{"createUpdaterArtifacts":false}}')
  warn "no TAURI_SIGNING_PRIVATE_KEY — skipping the unsignable updater payload (.app/.dmg unaffected)"
fi
( cd "$REPO/src-tauri" && cargo tauri build --features "$FEATURES" --bundles app,dmg "${updater_cfg[@]}" )
ok "Nexus .app + .dmg ($ARCH)"

bold "Done ✓  macOS artifacts ($ARCH):"
echo "  .dmg  : src-tauri/target/release/bundle/dmg/*.dmg"
echo "  .app  : src-tauri/target/release/bundle/macos/Nexus.app"
echo
warn "CAT needs Hamlib: 'brew install hamlib' puts rigctld on PATH. FT8/FT4 audio decode works"
warn "without it (VOX)."
warn "Unsigned/unnotarized builds are blocked by Gatekeeper on double-click — right-click the"
warn ".app and choose Open once, or set APPLE_SIGNING_IDENTITY/APPLE_ID/APPLE_PASSWORD/"
warn "APPLE_TEAM_ID for a signed + notarized build. See MACOS.md."
