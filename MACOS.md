# Building & running Nexus on macOS

**Status: not yet shipped.** Nexus does not publish macOS installers today — see the FAQ and
Roadmap. This page is for building from source. Most of the codebase is already macOS-aware
(cpal for audio, `keyring`'s Apple-native credential store, `Info.plist` with the microphone
usage string, `icon.icns` already in the icon set); what's missing is a signed/notarized
release pipeline, not a port. If you build this and it works for you, say so on the issue
tracker — that's what prioritizes an official release.

Nexus's modem (`libtempo`) is **Fortran + C/C++ + FFTW**, built through CMake the same way as
on Linux: the **native** (host == target) CMake path, FFTW3 single precision and Boost found
via `pkg-config`/`find_package`. The Tauri shell uses the OS-provided **WKWebView** (no extra
runtime to install, unlike Windows' WebView2), audio uses **CoreAudio** (via `cpal`, no extra
libs), and PTT/CAT uses Hamlib's **`rigctld`** over TCP, the same as Linux.

Nexus has two waveform tiers (the Tempo chat layer protocols), switched with the **Fast · Robust** toggle (never
silently — the active tier is always shown):
- **Fast = TempoFast** — 4 s T/R, coherent; regional NVIS / good-condition national /
  Field Day / conversational.
- **Robust = TempoDeep** — 15 s T/R, non-coherent 8-FSK; fading-resilient national
  reach (≈3.7 dB fading penalty in simulation vs FT8's 10+ dB collapse). Both
  tiers carry the same messages, so Chat / QSO / Field Day work on either.

## 1. Install the toolchain (Homebrew)

Install [Homebrew](https://brew.sh), then:

```bash
xcode-select --install     # Command Line Tools — provides clang, and WKWebView ships with macOS
brew install cmake ninja gcc fftw boost node hamlib
```

(`gcc` is what provides `gfortran` — there is no standalone `gfortran` formula, and macOS
ships no system Fortran compiler. `fftw` ships the single-precision `fftw3f` this modem needs.
`hamlib` gives you `rigctld`/`rigctl` for CAT — see §4.)

## 2. Install Rust + the Tauri CLI

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # rustup, if you don't have it
cargo install tauri-cli --version "^2"
```

No `-gnu`-vs-`-msvc` toolchain choice like Windows — the default `rustup` host toolchain
(`aarch64-apple-darwin` on Apple Silicon, `x86_64-apple-darwin` on Intel) links against the
same `clang`/`gfortran` this script uses.

## 3. Build & run

**One-shot:** `./scripts/build-macos.sh` does steps 1–3 (toolchain check, icons, UI deps) and
the release build for you. The manual steps below are what that script automates.

```bash
cd nexus
# Web UI deps (once):
npm --prefix ui install

# Run the desktop app with the live radio loop (sound card + rig):
cargo tauri dev   --features radio       # from src-tauri/, or: cargo tauri dev -d src-tauri ...
# Production build:
cargo tauri build --features radio --bundles app,dmg
```

Without `--features radio` the app builds and runs the full UI but does not key the radio
(useful for trying the interface). The headless workspace (`cargo test` in `tempo/`) does
**not** need a WebView and runs the modem/engine tests.

Artifacts land under `src-tauri/target/release/bundle/`: `Nexus.app` under `macos/`, the
installer `.dmg` under `dmg/`.

`build-macos.sh` builds for **whichever architecture it runs on** (Apple Silicon or Intel) —
it does not lipo a universal binary. `libtempo`'s Fortran/C/C++ modem would need to be built
twice (once per arch) and combined with `lipo` to ship one, and nobody has done that work yet.
Run the script natively on each arch you want to support — this is also how CI builds both
(see `.github/workflows/release.yml`'s `macos` job).

## 4. Configure your station (in-app Settings)

Settings persist to **`~/.config/tempo/settings.json`** — **not**
`~/Library/Application Support`. Nexus resolves its config directory from `XDG_CONFIG_HOME`
(falling back to `$HOME/.config`) on every non-Windows platform, macOS included, so it follows
the Linux convention here rather than the macOS one. If you're scripting around the config
file (backups, profile switching), this is the path to use.

- **Callsign / grid** — your identity (used for CQ/beacons and exchanges).
- **Band / dial frequency / sideband** — e.g. 14.074 MHz USB.
- **Field Day class / section** — e.g. `1D` / `WI`.
- **Rig / PTT** — pick a **PTT method** and (for CAT) a **rig model** + **serial port** +
  **baud**. See below.
- **Network** — WSJT-X UDP API + PSK Reporter (see §6).

### Rig control (CAT / PTT)

Nexus handles rig control in-app — **you do not run rigctld yourself**:

- **CAT (recommended):** choose your **rig model** (dropdown) and **serial port** (rigs show
  up as `/dev/tty.usbserial-*` or `/dev/cu.usbserial-*`, not `/dev/ttyUSB*` like Linux) +
  **baud** in Settings. Nexus launches Hamlib's `rigctld` for you and keys/tunes the rig.
  Unlike Windows, **the .app does not bundle `rigctld`** — install it with
  `brew install hamlib` and Nexus finds it on `PATH`, the same model as the Linux AppImage.
  The curated model list is best-effort — confirm your exact model number with `rigctl -l` if
  needed.
- **Serial RTS / DTR:** PTT-only rigs/interfaces — pick the port and the control line.
  (Enabled by the `serial` feature, which the `radio` build turns on.)
- **VOX:** no CAT; the rig keys on transmit audio.

Most ham rigs use a USB-serial bridge chip (Silicon Labs CP210x, FTDI, WCH CH340, Prolific).
macOS needs the vendor's driver for some of these (notably CH340/CH341 and older Prolific
chips) before the port shows up at all — see the driver hint Nexus shows for your rig's chip
if the port doesn't appear.

## 5. Audio wiring

Point macOS' input device (System Settings → Sound) at the rig's receive audio and the output
device at the rig's data/mic input (a USB CODEC such as a SignaLink or the rig's built-in USB
audio). `cpal` uses the system default devices; Tempo resamples to/from the modem's 12 kHz
automatically. The first launch prompts for microphone access — that's the
`NSMicrophoneUsageDescription` in `src-tauri/Info.plist`; if you dismiss it, re-enable under
System Settings → Privacy & Security → Microphone.

## 6. Network: WSJT-X UDP API + PSK Reporter

Enable these in Settings → Network for ecosystem compatibility:

- **WSJT-X UDP API** — Nexus emits the WSJT-X-compatible UDP protocol
  (Heartbeat / Status / Decode / QSO-Logged) so **JTAlert, GridTracker, N1MM and
  loggers** can consume Nexus's decodes/QSOs and (where supported) control it.
  Default target `127.0.0.1:2237` (same as WSJT-X). For another machine on the
  LAN, set its address (and allow the UDP port through the macOS firewall if it's on). It also
  accepts inbound Reply / HaltTx / FreeText.
- **PSK Reporter** — uploads your heard stations (call / freq / mode / SNR) to
  `report.pskreporter.info:4739` so your reception shows on the global maps.
  Outbound UDP — allow it through the firewall if it's on.

## Gatekeeper: running an unsigned build

A build you produce yourself with `cargo tauri build` is not code-signed with an Apple
Developer ID or notarized, so **Gatekeeper blocks it on a normal double-click** ("Nexus.app is
damaged and can't be opened" or "cannot be opened because the developer cannot be verified,"
depending on macOS version). This is expected for a source build, not a bug in the app. Two
ways around it for a build you made yourself:

- **Right-click (or Control-click) `Nexus.app` → Open**, then confirm in the dialog. This
  only needs doing once per build.
- Or clear the quarantine attribute directly: `xattr -cr /path/to/Nexus.app`.

**This is not a workaround anyone should tell end users to do** — it only makes sense for a
build you compiled yourself and trust. A real release needs an Apple Developer ID
($99/yr), `codesign`, and notarization via `notarytool`. Tauri v2 picks these up automatically
from environment variables at build time — `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (an
app-specific password, not your Apple ID password), and `APPLE_TEAM_ID` — nothing in
`tauri.conf.json` needs to change to support this. Without them, `cargo tauri build` still
succeeds; it just produces the unsigned build described above.

CI enforces this distinction rather than trusting a human to remember it: the `macos` job in
`.github/workflows/release.yml` asks `spctl` — the same authority Gatekeeper itself
consults — whether the built `.app` passes assessment. On a `workflow_dispatch` dry run an
unsigned build is fine (it's how the pipeline stays provably green before an Apple Developer
ID exists); on an actual tag push it's a hard failure, not a warning, because the alternative
is a signed-looking release that fails "can't be opened" for every operator who downloads it.
This mirrors the three-gates framing in `ci.yml`'s `macos-check` job (see issue #6): an Apple
Developer ID names a legal entity in every signed binary, the signing secrets are credential
handling, and a published artifact is a publish. `ci.yml` deliberately stays compile-only;
this file and the `macos` release job are that separate decision, made.

## Notes / troubleshooting

- If CMake can't find `fftw3f` or Boost, confirm `pkg-config --exists fftw3f` succeeds and
  that Homebrew's prefix (`brew --prefix`) is on `PKG_CONFIG_PATH` — this is usually automatic
  from a Homebrew-managed shell, but custom shells or a non-default Homebrew prefix (Intel's
  `/usr/local` vs Apple Silicon's `/opt/homebrew`) can need it set explicitly.
- Link errors about `gfortran`/`quadmath`/`fftw3f` mean the Homebrew packages above aren't
  where `pkg-config`/CMake expect them — re-check `brew list gfortran fftw boost`.
- No universal binary yet (§3) — an Intel-built `.dmg` will not run on Apple Silicon without
  Rosetta, and vice versa is not applicable (Apple Silicon binaries never run on Intel).
- Time sync matters for decoding: keep the Mac's clock accurate (it defaults to network time;
  don't disable it).
