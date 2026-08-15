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

## Status — 2026-08-14

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
