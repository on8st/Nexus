/**
 * Pre-save checks for the rig form.
 *
 * The form had exactly one check — the callsign — so every way of getting the RADIO wrong saved
 * silently and then behaved like broken hardware. Observed on one station in one afternoon
 * (ON8ST, 2026-08-13): a monitor's `usbmodem` device selected as a CAT port twice, and a profile
 * left on the second interface of a dual bridge, which answers nothing and looks exactly like a
 * dead rig.
 *
 * Pure on purpose. Everything here is decided from the form plus the enumerated ports, so it is
 * unit-testable without hardware, a running app, or a rig — which is also why the facts it needs
 * are structured fields on `SerialPortInfo` rather than patterns matched against display text.
 *
 * A `blocking` problem stops the save; a warning is stated and the operator proceeds. The split
 * matters: an operator with an unusual-but-correct setup must never be locked out of their own
 * configuration by a heuristic, so only things that CANNOT be right block.
 */
import type { SerialPortInfo } from './api'
import type { AudioDeviceInfo } from './types'

export type RigCheck = { level: 'error' | 'warning'; message: string }

/** The subset of the settings form these checks read. */
export interface RigFormFacts {
  serialPort: string
  audioIn?: string
  audioOut?: string
  rigConn: string
  pttMethod: string
  rigModel: number
}

export function checkRigForm(
  form: RigFormFacts,
  ports: SerialPortInfo[],
  audio?: { input: AudioDeviceInfo[]; output: AudioDeviceInfo[] },
  /**
   * Models that need no serial port, from `getPortlessRigModels()` — the backend's own
   * `model <= 4 || is_software_cat_profile(model)` (crates/tempo-audio/src/usbrig.rs). Empty
   * means the rule could not be read, and then the port check does not BLOCK: an unreadable
   * rule must never be why a correct configuration cannot be saved.
   */
  portlessModels: number[] = [],
): RigCheck[] {
  const out: RigCheck[] = []
  // A network rig has no serial port at all; none of this applies.
  if (form.rigConn === 'network') return out

  const port = form.serialPort.trim()
  if (!port) {
    // CAT is wanted and there is nowhere to send it.
    //
    // Gated on the portless set, because a whole class of models is served over TCP or a virtual
    // COM pair by a program on this machine: Dummy, NET rigctl, FLRig, Thetis, PowerSDR,
    // SmartSDR, SDR Console. Those are configured with no port ON PURPOSE, and ungated this
    // called every one of them an error and refused the save. Model 0 (None/VOX) is in that set
    // too, so "no model chosen" needs no separate case.
    //
    // `Array.isArray` rather than a bare `.length`: this runs inside the save handler, and a
    // throw here would abort the save with no message — the exact failure mode the rest of this
    // file exists to prevent.
    const ruleKnown = Array.isArray(portlessModels) && portlessModels.length > 0
    if (ruleKnown && !portlessModels.includes(form.rigModel)) {
      out.push({
        level: 'error',
        message: 'No serial port chosen — a rig model is set, so CAT needs a port.',
      })
    }
    return out
  }

  const info = ports.find((p) => p.name === port)

  // 1. Present at all. A saved port can legitimately be absent (rig switched off), so this is a
  //    warning — but the operator should know before wondering why CAT never connects.
  if (!info) {
    out.push({
      level: 'warning',
      message: `${port} is not connected right now — check the rig is powered on, or pick another port.`,
    })
  }

  // 2. macOS callout vs dial-in. `/dev/tty.*` blocks on carrier detect and will simply hang;
  //    `/dev/cu.*` is the one to use. They are offered as a pair and look interchangeable.
  if (port.startsWith('/dev/tty.')) {
    out.push({
      level: 'error',
      message: `${port} is a dial-in device and will hang waiting for carrier. Use the matching /dev/cu.… port instead.`,
    })
  }

  // 3. The silent half of a dual bridge. A CP2105 exposes two interfaces and only the first
  //    carries CAT on these rigs — choosing the second is the single most convincing way to make
  //    a working radio look dead.
  if (info?.interfaceIndex != null && info.interfaceIndex > 0) {
    out.push({
      level: 'warning',
      message: `${port} is port ${info.interfaceIndex + 1} of this device. CAT is normally on port 1 — the other one answers nothing.`,
    })
  }

  // 4. NOT HERE — two radios on one port. `settings::serial_port_conflicts` already decides it
  //    and App.tsx already puts the verdict in the status lane as `radioConfigWarning`. That rule
  //    carries four qualifiers a form-side copy loses on sight — the other profile must be
  //    `enabled`, have `rig_model > 0`, be on `rig_conn == "serial"` and have a non-empty port —
  //    and it compares case-insensitively, so the copy here fired on disabled profiles and missed
  //    `COM3` vs `com3`. It is a WARNING there, correctly: a station that swaps one cable between
  //    two rigs has both profiles on one port on purpose and must still be able to save. The copy
  //    BLOCKED. One rule, one place; this file does not get a second opinion on it.

  // 5. The sound card must be INSIDE the radio on this CAT port.
  //
  // This is the check that catches the failure names cannot: audio devices are stored by NAME,
  // two rigs with the same codec chip both enumerate as "USB Audio Device", and the positional
  // " #2" that separates them is assigned by enumeration order. Moving a rig to a different USB
  // port therefore SWAPS which rig each name means — observed on 2026-08-13, where every saved
  // profile silently began pointing at the other radio and nothing warned.
  //
  // Topology does not move when names do: a rig carrying CAT and audio down one cable is
  // internally a hub, so its codec shares the CAT port's parent. Comparing those two catches the
  // swap the instant it happens. Only ever a warning — a rig whose audio genuinely is not on its
  // own CAT device (a separate interface box, an analogue card) is a legitimate setup.
  const portHub = info?.pairedAudio != null ? info : undefined
  if (audio && portHub) {
    const catDevice = ports.find((p) => p.name === port)
    for (const [field, list] of [
      ['Input', audio.input],
      ['Output', audio.output],
    ] as const) {
      const chosen = field === 'Input' ? form.audioIn : form.audioOut
      if (!chosen) continue
      const dev = list.find((d) => d.name === chosen)
      // Only speak when BOTH sides are known and they disagree.
      if (dev?.usbHub == null || catDevice?.pairedAudio == null) continue
      const expected = list.find((d) => d.name === catDevice.pairedAudio)
      if (expected?.usbHub != null && dev.usbHub !== expected.usbHub) {
        out.push({
          level: 'warning',
          message: `${field} device “${chosen}” is not inside the radio on ${port} — “${catDevice.pairedAudio}” is. Device names can swap when a rig moves USB port.`,
        })
      }
    }
  }

  // 6. CAT keying with no rig model. `pttMethod: 'cat'` and model 0 (None/VOX) cannot both be
  //    true — there is nothing to send the keying command to.
  if (form.pttMethod === 'cat' && form.rigModel === 0) {
    out.push({
      level: 'error',
      message: 'PTT is set to CAT but the rig model is None/VOX — pick your rig model, or choose a different PTT method.',
    })
  }

  return out
}

/** Convenience: does anything here stop a save? */
export function blocks(checks: RigCheck[]): boolean {
  return checks.some((c) => c.level === 'error')
}
