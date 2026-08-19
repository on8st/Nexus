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

### The updater endpoint is a PERMANENT fork delta — check it after every merge

`src-tauri/tauri.conf.json` → `plugins.updater.endpoints` points at **`on8st/Nexus`**, not
`kd9taw/Nexus`. One line, and it must survive every upstream merge. `fork-radar` will flag
upstream commits touching this file; when it does, re-read that line before trusting the merge.

**Why, and it is new as of upstream v1.6.0.** The updater is active in fork builds
(`useSelfUpdate` checks at startup and hourly, then downloads SILENTLY and banners; installing
takes one operator click). Until v1.6.0 upstream's `latest.json` carried no `darwin-aarch64`
entry, so on macOS the check found nothing for this platform and the whole thing was inert.
It is there now — so a fork build on the 1.6.0 manifests sees upstream's 1.6.1, fetches it, and
stands there offering an install that would **overwrite the fork build with upstream's**: the
FT-710 waterfall and every unmerged macOS delta gone, by one absent-minded click on a banner
that looks exactly like routine housekeeping.

Pointed at the fork it is both safe and honest: `on8st/Nexus` publishes no releases, so
`releases/latest/download/latest.json` 404s, `check()` throws, and the hook swallows it in
silence exactly as it does for any unreachable endpoint (measured 2026-08-18: fork 404,
upstream 200). If the fork ever does publish, it then updates from the right place.

**Deliberately NOT done: changing `identifier`.** It stays `com.kd9taw.tempo`. That would also
separate the two apps, but it would invalidate the microphone TCC grant, and it buys nothing the
endpoint change does not — `config_base()` is `$HOME/.config`, so settings live in
`~/.config/tempo/` and are not identifier-derived either way. The shared identifier does mean a
side-by-side install of upstream's dmg shares this build's TCC grant and config directory; that
is a testing hazard, noted in `tasks/STATION-TODO.md`, not a reason to renumber the app.

---

## Status — 2026-08-18

Upstream is **1.6.1**. Five releases in four days — 1.3.0 (14 Aug), 1.4.0, 1.5.0, 1.6.0, 1.6.1
(17 Aug). The pace has not slowed and it is still editing the same audio files this fork does;
assume divergence, not stability.

`macos-support` carries the **1.6.0** manifests: **13 commits behind** `upstream/main`, 93 ahead.
Merge forward before starting anything new — `scripts/fork-radar` first, so the shortlist is the
upstream commits that touch files these branches touch rather than the whole log.

### The contribution question is answered — in practice, by 14 merges

**14 of 16 fork PRs are merged. Two are open.** Counted from the API on 2026-08-18, not from this
file's memory, which had all of the merged ones still listed as awaiting review.

| Open | What | State |
|---|---|---|
| [#85](https://github.com/kd9taw/Nexus/pull/85) | Restore actually restores — route it through the ordinary save path | three review points addressed; awaiting re-review |
| [#109](https://github.com/kd9taw/Nexus/pull/109) | `usbtopo` — tell two identical radios apart by USB topology (closes [#93](https://github.com/kd9taw/Nexus/issues/93)) | 9/9 CI green; awaiting review |

Merged: #69, #70, #71, #72, #73, #74, #77, #78, #79, #88, #89, #90, #91, #92.

**What that means for the standing-policy question this file kept waiting on.** It was never
answered as a written policy, and it no longer needs to be: generic fixes, macOS fixes and
test-quality work have all gone in, and on [#93](https://github.com/kd9taw/Nexus/issues/93) the
maintainer answered a *design* question with a concrete shape to build to. Treat the categories as
settled by behaviour and stop gating work on a policy statement. **Still genuinely open:** the dual
radio-config representation (`radios[]` versus the ~19 flat `Settings` fields) — do not "fix" that
until it is answered.

Issue [#76](https://github.com/kd9taw/Nexus/issues/76) (dark-mode placeholder contrast) is closed.
Issue [#110](https://github.com/kd9taw/Nexus/issues/110) — the FT-710 waterfall — was **withdrawn
and scrubbed** on 2026-08-18 as filed too early; the draft is kept machine-local at
`tasks/drafts/ft710-waterfall-issue.md`. Re-file when the FTDI licence question is settled.

---

## Status — 2026-08-15 (previous)

Upstream is **1.3.0**; it released 1.2.10 → 1.2.14 tester builds → 1.3.0 in about a day. It moves
fast and it is actively editing the same audio files this fork does. Assume divergence, not
stability. `macos-support` is level with 1.3.0 as of 2026-08-14.

### Nine PRs open at the time — ⚠️ ALL NINE HAVE SINCE MERGED; see the 2026-08-18 status above

[#69](https://github.com/kd9taw/Nexus/pull/69) macOS link paths · [#70](https://github.com/kd9taw/Nexus/pull/70) rigctld resolve ·
[#71](https://github.com/kd9taw/Nexus/pull/71) stale rigctld test · [#72](https://github.com/kd9taw/Nexus/pull/72) `npm ci` ·
[#73](https://github.com/kd9taw/Nexus/pull/73) audio recovery · [#74](https://github.com/kd9taw/Nexus/pull/74) banner hold ·
[#77](https://github.com/kd9taw/Nexus/pull/77) build stamp · [#78](https://github.com/kd9taw/Nexus/pull/78) Node 25 localStorage ·
[#79](https://github.com/kd9taw/Nexus/pull/79) api-mock derivation. Plus issue
[#76](https://github.com/kd9taw/Nexus/issues/76) (dark-mode placeholder contrast).

Seth said yes to PRs on [#6](https://github.com/kd9taw/Nexus/issues/6) and offered credit. The
standing-policy question (generic fixes / macOS fixes / enhancements / new features /
architectural changes) was open at this point — ⚠️ superseded: answered in practice by 14 merges,
see the 2026-08-18 status. The dual radio-config question is still genuinely open.

---

## Upstreamable from this session — assessment

All four fixes above were made on `macos-support`, **not** off `upstream/main`. That is the rule
in this file being broken for the third time, and the cost is the same each time: extraction.

**⚠️ All four have since been offered, and the assessment below was consumed rather than
abandoned — keep it for the reasoning, not the status.** Restore persistence is
[#85](https://github.com/kd9taw/Nexus/pull/85) (open, awaiting re-review); `window.confirm` merged
as [#89](https://github.com/kd9taw/Nexus/pull/89); add-radio hijack merged as
[#91](https://github.com/kd9taw/Nexus/pull/91); serial dedup went in two halves, PR #92 and
[#109](https://github.com/kd9taw/Nexus/pull/109). The "three separate PRs, not one" call below was
right: the confirm change did take the longest review, and bundling it would have held the
two-line restore fix hostage to it.

| Fix | Upstreamable? | Shape |
|---|---|---|
| **Restore persistence** (3) | **Yes — strongest** | Universal bug, small, self-contained, and the fix is "do what `reset_settings` already does". Needs a test, which it has. |
| **`window.confirm` inert** (1) | **Yes — macOS, high value** | Exactly the category Seth said gets maintenance. But it is 11 files and introduces a UI primitive, so it is the biggest ask. Frame it as the BUG (thirteen destructive actions silently cancel on macOS) with the dialog as the remedy. |
| **Add-radio hijack** (2) | **Yes, with care** | Universal bug, but it reverses an invariant pinned by a test with a documented rationale. The PR must explain why the clobber scaffolding is obsolete, not merely delete it. Touches 7 tests. |
| **Serial dedup** (4) | **Done — both halves** | Resolved the way this row suggested, in that order: the name-based half landed upstream as `collapse_tty_twins` via PR #92, and the topology half went in on top of it as `collapse_usb_siblings` in [#109]. |

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
| `usbtopo` label rewriting — `label_by_rig`, `label_serial_ports`, `devices_sharing_usb_device` | rewrite what the PICKERS DISPLAY, e.g. "USB Audio Device (FT-710)" instead of a bare `" #2"` | held back on purpose: it changes the text of every picker and wants its own review of what happens when the topology reading is wrong. The rest of `usbtopo` went upstream in [#109] — see below. |
| `config-ui` (work, no longer a branch) | probe UX, validation, Config tab, device cross-checks | product decisions — needs buy-in first; see the PR-candidate table above |
| `MACOS.md`, `scripts/build-macos.sh` | macOS build path | **reason falsified 2026-08-18 — re-decide.** See "Upstream ships macOS" below. |

### Open questions put to upstream

- **Standing contribution policy** across the five categories — asked, unanswered.
- **The dual radio-config representation.** Config is held twice: a profile in `radios[]` and
  ~19 flat fields on `Settings`, reconciled by hand at ~50 call sites. The comment on
  `sync_flat_from_active` reads as a compatibility shim so existing consumers keep working. Asked
  whether that was a step toward `radios[]` as the single source of truth or is deliberate.
  **Do not "fix" it until that is answered.**

---

## FT-710 waterfall over USB — feasibility, measured 2026-08-17

**Verdict: technically proven on this station. The remaining blocker is licensing, not protocol.**

### What it is

The FT-710 contains an **FT4222 USB→SPI bridge**. Enable *SCU-LAN10* in the radio's menu and it
appears as a third USB function alongside the CAT bridge and the codec:

```
IOUSBHostDevice@02400000                 ← the FT-710's internal hub
  ├─ CP2105 Dual USB to UART @02410000   ← CAT (/dev/cu.usbserial-01AF7FED0 and …1)
  ├─ USB Audio Device        @02420000   ← the codec
  └─ FT4222                  @02430000   ← the waterfall bridge (FTDI, VID 0x0403, PID 0x601C)
```

It is NOT a serial port and NOT a CAT command. Two dead ends ruled out by measurement first:
`SS` (SPECTRUM SCOPE) only reads/writes scope SETTINGS — verified against Yaesu's own FT-710 CAT
manual, and confirmed live (`SS05;` → `SS0570000;`, span = 200 kHz). And nothing arrives
unsolicited: 0 bytes in 3 s on `…FED1` at 38400 and 115200, and 0 bytes on the CAT port itself
with no daemon holding it.

### The measurement

`libft4222` on Apple Silicon, SPI master per `ratmandu/YaesuWFTesting`
(`Mode.SINGLE, Clock.DIV_16, Cpol.IDLE_HIGH, Cpha.CLK_TRAILING, SlaveSelect.SS0`, 48 MHz clock):

```
40 frames of 4096 B in 0.48 s          → 84 reads/s (12 ms per frame)
waterfall RX1, 852 bins                → min 0, max 248, mean 184
bins 0-15                              → 6c c4 c9 b4 b5 bc bd b7 bb ca d8 cb ca bf b4 b7
unique frames                          → 17/40  (LIVE, not a static buffer)
bins changed, first vs last frame       → 821/852
```

No init command is needed — open, configure SPI master, read. CAT kept working throughout on
`…FED0`: this is a separate USB function, not a shared bus.

### Frame layout (from `ratmandu/YaesuWFTesting`, originally via wfview)

4096-byte frame: waterfall RX1 `0..851`, RX2 `852..1703` (reserved on this model), AF-FFT RX1
192 B, AF oscilloscope RX1 400 B (128 = zero), then a 144-byte parameter block.

⚠️ **The parameter block did not reproduce.** At the documented offset the 144 bytes are 128 zero
bytes followed by a repeating `ff 01 ee 01` — an idle/padding pattern, not frequencies or meter
data. (An earlier note here said "all zeroes"; that was read off the first 24 bytes only. The
distinction matters: the block is not absent, it carries nothing useful.) So on the FT-710 those
fields sit at a different offset, or are only populated under conditions we did not hit.
Irrelevant for a waterfall — the 852 bins are solid — but it is exactly the part that project was
still working on. Do not build on it without re-deriving it. A real frame is committed as
`crates/tempo-audio/tests/fixtures/ft710_wf_frame.bin` so the next attempt starts from evidence.

### Verified against a known signal — and three faults that only that could find

A strong broadcast carrier on 9.410 MHz, 50 kHz span, W/F CENTER mode. This is the part no unit
test can do, and it changed the code three times:

| Question | Answer | How |
|---|---|---|
| Polarity | **inverted** — low bytes are strong | dial centred, 20 frames averaged: centre bin **109** against a 185-190 floor |
| Usable width | **850 bins**, not 852 | bins 850/851 zero in all 30 frames sampled; 844-849 vary normally |
| Bin→frequency | **correct** | `peak 9.4100 MHz` on a dial of 9.410120, stable frame after frame (~59 Hz per bin) |
| Mirrored? | **no** — low→high, left to right | dial moved to 9.395: peak reported 9.4090, i.e. RIGHT of centre. Mirrored would read 9.3800 |

Publishing the bytes as-is would have drawn every band upside down — signals as holes in a bright
ceiling — and the display AGC would have stretched that into something plausible. Including the two
padding bins would have put a permanent phantom carrier at the top edge of every span, moving
whenever the operator retuned. Neither is mentioned in `ratmandu/YaesuWFTesting` or the
wfview-derived layout, and neither would have errored.

⚠️ **One residual, quantified rather than assumed.** Dead centre the peak reads 9.4100 (exact);
15 kHz off centre it reads 9.4090 — about **1.1 kHz low**. Too large for the padding question
(~35 Hz) and too small for wrong offsets or mirroring. Candidates: the strongest bin of an AM
signal is not necessarily its carrier, the span may not be exactly 50.000 kHz, or the
peak-to-frequency interpolation is a few bins out. Irrelevant for drawing a waterfall; it matters
the moment a click on a signal is supposed to tune to it.

### Why this fits Nexus with little new machinery

The app side already exists: a native-scope path (Icom CI-V, Flex), the `rigscope` pane, and a
`SpectrumFeed` that takes bins. 852 uint8 bins per frame drop into that without new UI or DSP.

And **radio attribution is already solved** by the USB-topology code this fork carries: the
FT4222 sits on the same parent hub (`0x2400000`) as the FT-710's CAT port and codec, which is the
same evidence `usbtopo` already uses to label the codec "USB Audio Device #2 — FT-710". With two
scope-capable radios that is not a nicety — the scope must bind to the right rig.

### What actually blocks upstreaming

**FTDI's D2XX / LibFT4222 is a closed-source binary library, and Nexus is GPL-3.0-only.** That is
a per-file licence question to answer BEFORE vendoring anything, and it is the one open item. The
fork's implementation therefore expects the library to be present on the system and is gated
behind an off-by-default feature; no FTDI binary is vendored into this repo.

One operational note: the first `open` of the FT4222 hung for ten minutes, and every run after it
was instant. A one-off claim rather than a protocol problem, but an implementation needs a timeout
around open rather than trusting it.

## `usbtopo` HAS been offered — #93 / #109, 2026-08-18

Two rows in this file said it had not, and a reader acting on either would have redone work that
is sitting in an open PR. Recorded here rather than only in the rows, because the split between
what went and what stayed is the part worth knowing.

[Issue #93](https://github.com/kd9taw/Nexus/issues/93) asked the question; the maintainer answered
with a shape — *optional topology fields on `SerialPortInfo`, string matching stays the first pass,
macOS-gated with empty-map stubs, and the two checks held out of #88 can ride in behind it.*
[PR #109](https://github.com/kd9taw/Nexus/pull/109) is that shape, off `upstream/main`, 9/9 CI
green, awaiting review.

**Went upstream in #109:** `parent_hub`, `location_from_audio_uid`, the three registry/CoreAudio
readers and `serial_topology`; `ports::collapse_usb_siblings` (behind upstream's own
`collapse_tty_twins` — itself the fork's, via PR #92 — and keeping anything it cannot key);
`interfaceIndex` / `siblingPorts` /
`pairedAudio` / `usbHub` as optional DTO fields; and the two `checkRigForm` diagnostics.

**Stayed fork-only:** the three label-rewriting helpers. They were REMOVED from the PR rather than
shipped unwired — offering code with no caller invites the reviewer to design it for you.

**Two findings from the PR that belong in this file, because neither was in the design:**

1. `siblingPorts` exists because live data falsified the first version of the dual-bridge check. It
   fired on `interfaceIndex > 0` alone, and the LG monitor on this desk is interface **2** of a
   device with exactly **one** interface — it would have told the operator to pick "port 1" of a
   device that has no port 1. The count is taken over ports sharing the EXACT `locationID`.
2. **The two relations are not equally strong, and the difference bounds what each may claim.** Two
   serial interfaces of one bridge share the same `locationID`, so counting them is exact. A rig's
   CAT bridge and its codec are SEPARATE USB devices behind the rig's internal hub, so only
   `parent_hub` relates them — and two unrelated things in one EXTERNAL hub share a parent too. A
   USB headset beside a rig's CAT adapter can read as "inside" it. Hence the interface advice can
   be precise while the paired-audio reading may only ever raise a doubt.

Also worth keeping: `cargo clippy --workspace` FEATURELESS caught `collapse_usb_siblings` as dead
code without `serial`, and the first local clippy run reported clean because `-D warnings` aborted
on pre-existing findings in `propagation` before reaching `tempo-audio`. Use `--no-deps` when
linting one crate, and do not read a clean sweep as clean until a positive control says the sweep
reached your file.

## What stays LOCAL, always — and what is a PR candidate (2026-08-19)

Reviewed after the 1.7.0 merge. **Measured against `upstream/main`, not against a checked-out
tree** — that mistake produced two near-miss duplicate PRs on 2026-08-18.

### Always local — never offer these

| Item | Why it can never go upstream |
|---|---|
| `FORK.md`, `scripts/fork-radar` | Fork infrastructure by definition |
| `.githooks/contributors` fork entry | The identity the fork's own pre-push gate accepts |
| `src-tauri/tauri.conf.json` → `plugins.updater.endpoints` | Points at the fork so upstream's release cannot overwrite a fork build. **Re-check after every merge** — see the section on it above |
| `.gitignore` fork line (`tasks/`) | Machine-local scratch |
| Fork-only changelog entries | Kept out of upstream's `CHANGELOG.md` so that file merges clean forever |

### PR candidates, in the order they are worth doing

| Item | Shape | Blocker |
|---|---|---|
| **AGC AUTO/OFF** | The FT-710 has both and Nexus offered neither; `Engine::AGC_SPEEDS` plus the two cockpit selectors | none — smallest, cleanest win |
| **USB audio/serial pickers say WHICH RADIO** | `label_by_rig`, `label_serial_ports`, `devices_sharing_usb_device` + `examples/usb_topology.rs`. Rewrites what the pickers DISPLAY | stacks on [#109]; that PR carries the module and the structured fields |
| `MACOS.md`, `scripts/build-macos.sh` | Local macOS dev-build path | needs rework: upstream now has its own macOS build in `release.yml`, so these must reconcile with it rather than sit beside it |
| `config-ui` — auto-test PROPOSES a port, a Config tab (Backup/Restore/Reset), probe candidates excluded by USB device, the sound-card-not-in-this-rig warning | Lives on **`macos-support`**; the `config-ui` branch was deleted 2026-08-19 as a redundant second copy (archived locally as tag `archive/config-ui`). A PR branches off `upstream/main` and ports it | **ask first**, and **split it**: ~650 lines across four separable features, and one PR for all of them would land far harder than three small ones |

### Held, not dropped

| Item | Why |
|---|---|
| FT-710 waterfall — `yaesu_wf.rs`, its probe, the frame fixture, the `'yaesu'` scope label | FTDI D2XX/LibFT4222 is closed-source against GPL-3.0-only. Issue #110 was withdrawn as premature; the draft is machine-local in `tasks/drafts/`. Re-file when the licence question is answered |

### Dropped

`up/rigctld-orphans` — see the next section. The branch stays pushed on the fork as a dead
record; do not build on it.

[#109]: https://github.com/kd9taw/Nexus/pull/109

## The rigctld-orphan work is OBSOLETE — upstream got there first (2026-08-18)

**Do not re-propose it.** `up/rigctld-orphans` (3 commits: port-already-held guard, which-radio
identification, the `ps` drain fix) is superseded by upstream **`291bb662` "fix(unix):
rigctld/rotctld can no longer outlive Nexus on macOS/Linux"**, which landed after v1.6.1 and is in
1.7.0. Upstream's version is BETTER than the fork's: an on-disk PID ledger, `kill_leftovers` on the
quit path, AND `init_orphan_ledger` sweeping at startup what a crash or force-quit left behind — so
it survives the cases a Rust `Drop` cannot. `AddrInUse` handling is in 1.7.0 too.

**How this was nearly missed, which is the reusable part.** The leak is real and was watched three
times on 2026-08-18 — but on the shipped **1.6.1** dmg, which predates the fix. A fresh
`stop_cat_daemons` fix was written, tested and about to be PR'd before anyone read upstream's
current `rigctld_proc.rs`. Worse, an earlier check "upstream has none of these guards" was run
against a WORKING TREE at 1.6.1, not against `upstream/main`, and came back clean — a false
negative that would have produced a PR reimplementing a landed fix.

**The rule this yields:** measure upstream with `git show upstream/main:<path>`, never against a
checked-out tree, and never against a branch that is behind. `scripts/fork-radar` exists for this;
use it before writing, not after.

## Upstream ships macOS — checked 2026-08-18, and it overturns an assumption in this file

**macOS is a first-class upstream platform.** This file said the opposite, and that claim was
load-bearing: it is the stated reason `MACOS.md` and `scripts/build-macos.sh` were never offered.
Two distinct milestones, both read off the published artifacts rather than off a job name:

| | v1.4.0 | v1.5.0 (16 Aug) | v1.6.0 (16 Aug) onward |
|---|---|---|---|
| `.dmg` + signed `.app.tar.gz` | — | **yes** | yes |
| `latest.json` platforms | win, linux | win, linux | win, linux, **`darwin-aarch64`** |

- **v1.5.0: macOS enters the formal build**, and it *gates* the release — `release.yml`'s publish
  step declares `needs: [linux-x86, windows, smoke-windows, macos, smoke-macos]`, so a broken
  macOS build blocks the whole publish. `smoke-macos` mounts the dmg.
- **v1.6.0: macOS joins the signed auto-updater.**
- Build: `cargo tauri build --target aarch64-apple-darwin --features radio,custom-protocol
  --bundles app,dmg` on `macos-14`. Apple Silicon only; no Intel bundle.

**⚠️ How this was got wrong, because the same trap is still in place.** `ci.yml` carries a
separate, deliberately cheap job named *"macOS compile check (not a shipped platform)"*. It
builds no bundle **on purpose** and its name is stale. Reading that job title as the project's
platform policy is how this file came to assert something the release page disproves in one
query. **The published artifacts are the authority on platform support; a CI job name is not.**

**Consequences to act on, not yet acted on:**

1. `MACOS.md` and `scripts/build-macos.sh` still do not exist upstream (checked 2026-08-18), and
   the reason for holding them back is gone. That is an upstreaming opportunity, not a closed
   door — but upstream now has its own macOS build path in `release.yml`, so anything offered
   must be reconciled with it rather than layered beside it.
2. The `ci.yml` job name is misleading to every future reader. A one-line doc fix worth offering.
3. Stop framing macOS work upstream as a second-class ask. It is a gating platform.

## Station notes

- The FT-710 must be plugged **directly into the Mac**, not through a monitor hub — two stacked
  hardware faults, diagnosed 2026-08-13.
- Hamlib: keep the self-built `~/.local` copy (4.7.0~rc); it is the only one with the FTX-1 driver.
- UI tests need **Node ≤ 24**. Node 25 ships a built-in `localStorage` whose `.clear` is undefined
  and every jsdom suite fails in `beforeEach`. CI pins Node 24, so this is local-only.
- A release build from a **worktree** fails on the gitignored DeepCW model unless the model is
  staged or `NEXUS_ALLOW_MISSING_AICW=1` is set.

[#109]: https://github.com/kd9taw/Nexus/pull/109
