# FORK.md — how this fork relates to upstream

**Fork-local file. Never upstream it.** `kd9taw/Nexus` is upstream; this is `on8st/Nexus`.

Its job is to stop the same decisions being re-made. If you are about to work out *again*
whether something is upstreamable, or whether upstream already fixed it, the answer belongs here.

Run `scripts/fork-radar` before merging upstream forward. It reports only the upstream commits
that touch files your branches touch — the shortlist, not the log.

---

## Branch topology

| Branch | Role | Rule |
|---|---|---|
| `main` | pristine mirror of upstream | **never commit** — `fetch` + `merge --ff-only` only |
| `macos-support` | the station branch: what gets built and run | merge upstream forward often |
| `up/*`, `pr/*` | intended for upstream | branch off **`upstream/main`**, never off the station |
| `fork/*` | fork-only | branch off the station |

### The rule that pays for itself

**Anything that might be upstreamable starts on a branch off `upstream/main`.**

This was learned the expensive way. Tier 1 had to be surgically extracted from `macos-support`
— cherry-picks, generalising comments, and a station identifier that existed only in an
intermediate state that the stacked branch never had. Branching off upstream in the first place
costs nothing; extracting later costs an afternoon.

`up/*` and `pr/*` branches additionally must touch **zero** version manifests
(`src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `ui/package.json`) and **zero** CHANGELOG
release headers. Versioning is the maintainer's call, not a contributor's.

## Versioning

The version number belongs to upstream. Never mint a value in its namespace — two different
artifacts both called `1.3.0` is the whole failure mode. Manifests carry whatever upstream says,
inherited by merging, never chosen here. `scripts/release-prep` is upstream's tool; do not run it
on the fork.

Fork identity lives in git instead: tag builds `on8st-<upstream version>-<n>` (never `v*` —
`release.yml` fires on `v*` tags and would publish a public GitHub Release), and the build stamps
`owner/repo branch@sha[-dirty]` into itself so any artifact can name its own origin.

Fork-only changelog entries go in a separate file, not upstream's `CHANGELOG.md`, so that file
merges clean forever.

---

## Status — 2026-08-15

Upstream is **1.3.0**; it released 1.2.10 → 1.2.14 tester builds → 1.3.0 in about a day. It moves
fast and it is actively editing the same audio files this fork does. Assume divergence, not
stability. `macos-support` is level with 1.3.0 as of 2026-08-14.

### Nine PRs open upstream, awaiting review

[#69](https://github.com/kd9taw/Nexus/pull/69) macOS link paths · [#70](https://github.com/kd9taw/Nexus/pull/70) rigctld resolve ·
[#71](https://github.com/kd9taw/Nexus/pull/71) stale rigctld test · [#72](https://github.com/kd9taw/Nexus/pull/72) `npm ci` ·
[#73](https://github.com/kd9taw/Nexus/pull/73) audio recovery · [#74](https://github.com/kd9taw/Nexus/pull/74) banner hold ·
[#77](https://github.com/kd9taw/Nexus/pull/77) build stamp · [#78](https://github.com/kd9taw/Nexus/pull/78) Node 25 localStorage ·
[#79](https://github.com/kd9taw/Nexus/pull/79) api-mock derivation. Plus issue
[#76](https://github.com/kd9taw/Nexus/issues/76) (dark-mode placeholder contrast).

Seth said yes to PRs on [#6](https://github.com/kd9taw/Nexus/issues/6) and offered credit. The
standing-policy question (generic fixes / macOS fixes / enhancements / new features /
architectural changes) is **still unanswered**, as is the dual radio-config question.

---

## Upstreamable from this session — assessment

All four fixes above were made on `macos-support`, **not** off `upstream/main`. That is the rule
in this file being broken for the third time, and the cost is the same each time: extraction.

| Fix | Upstreamable? | Shape |
|---|---|---|
| **Restore persistence** (3) | **Yes — strongest** | Universal bug, small, self-contained, and the fix is "do what `reset_settings` already does". Needs a test, which it has. |
| **`window.confirm` inert** (1) | **Yes — macOS, high value** | Exactly the category Seth said gets maintenance. But it is 11 files and introduces a UI primitive, so it is the biggest ask. Frame it as the BUG (thirteen destructive actions silently cancel on macOS) with the dialog as the remedy. |
| **Add-radio hijack** (2) | **Yes, with care** | Universal bug, but it reverses an invariant pinned by a test with a documented rationale. The PR must explain why the clobber scaffolding is obsolete, not merely delete it. Touches 7 tests. |
| **Serial dedup** (4) | Probably — macOS-only | Depends on `usbtopo`, which has not been offered. Either propose `usbtopo` first or ship the name-based half alone. |

**Three separate PRs, not one.** They share no code, they carry very different review burdens, and
Seth merges small ones fastest. Do NOT bundle them: the confirm change would hold the two-line
restore fix hostage to a UI-primitive discussion.

---

## Config-UI session, 2026-08-14/15 — four bugs, and what they teach

All four were found by an operator using the app, not by a test. Each one is recorded with the
symptom first, because the symptom is what the next person will have.

### 1. `window.confirm` is INERT on macOS — thirteen destructive actions did nothing

**Symptom:** Remove radio did nothing at all — no dialog, no error, no effect. Then Reset the same.

**Cause:** WKWebView shows a JS confirm only if the host implements
`runJavaScriptConfirmPanelWithMessage`. wry 0.55.1 implements exactly three `WKUIDelegate`
methods — `runOpenPanelWithParameters`, `requestMediaCapturePermission`,
`createWebViewWithConfiguration` — and that is not one of them. So `confirm()` returns `false`
immediately, showing nothing, and every `if (!window.confirm(…)) return` guard cancels silently.

**It is macOS-only.** `webview2` and `webkitgtk` contain *no* script-dialog handling at all, so
they fall through to those engines' defaults, and both show dialogs. Checked in the wry source,
not assumed.

**This one cause produced four separate-looking bugs:** Remove radio, Reset, Restore (which ALSO
had a real persistence bug — see 3), and "cannot select any radio", because both radio-switch
paths are guarded by `if (dirty && !confirm('Discard unsaved changes?')) return` and a dead
confirm reads as the operator saying no.

**Fixed** by `src/confirm.tsx`, promise-based on the existing Radix Dialog, **failing closed**
(no host mounted ⇒ resolves false; a missing dialog that answered *yes* would be silent data
loss). Thirteen of fifteen sites converted.

**TWO ARE DELIBERATELY NOT CONVERTED** — both transmit path, both currently failing SAFE:
`SetupHealth`'s "Prove the transmit path?" (keys TX ~2 s) and `SstvView`'s ISS 145.800 downlink
guard. Converting them restores the intended ask-then-allow, but that ENABLES TRANSMISSION where
it is presently blocked. That needs the maintainer's explicit sign-off, never a side effect of a
UI fix.

### 2. Add radio hijacked the station and froze the UI

`Engine::add_radio` called `set_active_radio` one line after `add_radio_profile`, whose own doc
comment says it does not change the active radio. That is a LIVE RIG SWITCH, not a roster edit: it
tears down the working radio's CAT and brings up one with no port and no model, under the engine
lock — hence the freeze, and an operator left on an empty profile with a blank settings pane.

The switch was scaffolding for the *clobber bug* (the flat form edits whatever is ACTIVE). The
panel now routes non-active edits through `update_radio_profile(editingRadioId)`, so the
scaffolding outlived its problem. Seven tests used `add_radio` as shorthand for "make a radio and
switch to it"; their setups now switch explicitly.

### 3. Restore never persisted, and never refreshed

`import_settings_bundle` was the one write that bypassed `set_settings`. It hand-rolled
`eng.apply_settings()` and returned `()`, so nothing persisted and the panel had no snapshot to
re-render from. Worse, the stale form stayed live: the next Save wrote the OLD values back over
the restored ones. It now routes through `set_settings` exactly as `reset_settings` does.

**There was not one reference to `importSettingsBundle` in the whole suite.**

### 4. Twenty-two serial rows for two radios

macOS lists every port two to four times: `/dev/tty.X` twins of `/dev/cu.X`, and
`cu.SLAB_USBtoUART<n>` twins of `cu.usbserial-<serial><iface>` — same silicon, names sharing
nothing, so only USB topology can see it. Collapsed to 7 on this station. **Nothing is dropped
that cannot be PROVED a duplicate**: no topology, or no better twin, means it survives.

### What these four have in common

Every one was invisible to a green test suite, and three were invisible *because* of how they
were tested: `window.confirm` was mocked to return `true` (a dialog the app never shows);
`importSettingsBundle` had no test at all; the duplicate rows only exist against real hardware.
**Mocking the thing you are trying to verify is how all three survived.**

---

## Status — 2026-08-14 (previous)

Upstream is **1.3.0**; it released 1.2.10 → 1.2.14 tester builds → 1.3.0 in about a day. It moves
fast and it is actively editing the same audio files this fork does. Assume divergence, not
stability.

### Offered upstream — awaiting policy answer

Seth said yes to PRs on [#6](https://github.com/kd9taw/Nexus/issues/6) and offered credit. A
follow-up asks what standing policy he wants across generic fixes / macOS fixes / enhancements /
new features / architectural changes. **Nothing has been PR'd yet.**

| Branch | What | vs upstream |
|---|---|---|
| `pr/macos-link-paths` | `tempo-fast-sys/build.rs` emits macOS link search paths | clean |
| `pr/rigctld-resolve` | verify a resolved `rigctld` actually runs (`cfg(unix)`) | clean |
| `pr/audio-recovery` | retry the fallback open; surface OS-killed streams | **collides — see below** |
| `pr/audio-banner-hold` | hold the banner across a flapping card | **collides — see below** |
| `pr/rigctld-test-premise` | fix a test asserting a premise Hamlib does not honour | clean |
| `pr/npm-ci-linux` | `npm ci` in `build-linux.sh` | clean |

### ⚠ Open conflict — upstream fixed audio independently

`7e88d120 fix(audio): release the old sound card BEFORE probing the new one (#2, #8)`, dated
2026-08-14, touches `backend.rs`, `device.rs`, `service.rs` **and** `monitor.rs` — the same
surface as `pr/audio-recovery` and `pr/audio-banner-hold`.

**Do not open those two PRs before reading that commit.** Some or all of the work may be done, or
done differently, and offering a patch that fights a fix the maintainer just landed is worse than
offering nothing. The other four are unaffected and can go whenever policy allows.

### Fork-only, not offered

| Branch / area | What | Why not upstream |
|---|---|---|
| `fork/build-stamp` | stamp fork/branch/commit into the build, show in the version chip tooltip | generic enough to offer later; branched off the station, so it needs extraction first. **Collides** with upstream's `2a942d41`, which also edits `src-tauri/build.rs`. |
| `fork/tooling` | this file + `scripts/fork-radar` | fork infrastructure by definition |
| `usbtopo` (in `macos-support`) | identify which radio a sound card / serial port belongs to, from USB topology | macOS-only, no-ops elsewhere; a design decision, described on #6 but not proposed |
| `config-ui` | probe UX, validation, Config tab, device cross-checks | product decisions — needs buy-in first |
| `MACOS.md`, `scripts/build-macos.sh` | macOS build path | upstream has not accepted macOS as a supported platform |

### Open questions put to upstream

- **Standing contribution policy** across the five categories — asked, unanswered.
- **The dual radio-config representation.** Config is held twice: a profile in `radios[]` and
  ~19 flat fields on `Settings`, reconciled by hand at ~50 call sites. The comment on
  `sync_flat_from_active` reads as a compatibility shim so existing consumers keep working. Asked
  whether that was a step toward `radios[]` as the single source of truth or is deliberate.
  **Do not "fix" it until that is answered.**

---

## Station notes

- The FT-710 must be plugged **directly into the Mac**, not through a monitor hub — two stacked
  hardware faults, diagnosed 2026-08-13.
- Hamlib: keep the self-built `~/.local` copy (4.7.0~rc); it is the only one with the FTX-1 driver.
- UI tests need **Node ≤ 24**. Node 25 ships a built-in `localStorage` whose `.clear` is undefined
  and every jsdom suite fails in `beforeEach`. CI pins Node 24, so this is local-only.
- A release build from a **worktree** fails on the gitignored DeepCW model unless the model is
  staged or `NEXUS_ALLOW_MISSING_AICW=1` is set.
