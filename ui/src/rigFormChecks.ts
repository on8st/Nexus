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
  radios?: { id: number; name: string; serialPort: string }[]
  /** Optional on `Settings` (a pre-roster config has none); treated as radio 0. */
  activeRadio?: number
}

/** Which radio the form is describing — the one being edited, else the active one. */
function targetRadioId(form: RigFormFacts, editingRadioId: number | null | undefined): number {
  return editingRadioId ?? form.activeRadio ?? 0
}

export function checkRigForm(
  form: RigFormFacts,
  ports: SerialPortInfo[],
  editingRadioId: number | null | undefined,
  audio?: { input: AudioDeviceInfo[]; output: AudioDeviceInfo[] },
): RigCheck[] {
  const out: RigCheck[] = []
  // A network rig has no serial port at all; none of this applies.
  if (form.rigConn === 'network') return out

  const port = form.serialPort.trim()
  const usesSerial = form.rigModel !== 0 || form.pttMethod !== 'vox'
  if (!port) {
    if (usesSerial && form.rigModel !== 0) {
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

  // 4. Another radio already uses it. Two profiles on one port means two daemons fighting for it;
  //    the loser dies and its radio silently stops responding.
  const me = targetRadioId(form, editingRadioId)
  const clash = (form.radios ?? []).find(
    (r) => r.id !== me && r.serialPort.trim() === port && port !== '',
  )
  if (clash) {
    out.push({
      level: 'error',
      message: `${clash.name} already uses ${port}. Two radios cannot share one CAT port.`,
    })
  }

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
