import { describe, it, expect } from 'vitest'
import { checkRigForm, blocks, type RigFormFacts } from './rigFormChecks'
import type { SerialPortInfo } from './api'
import type { AudioDeviceInfo } from './types'

// The real station this was written for: two Yaesus, each a CP2105 with two interfaces, plus a
// monitor that also presents a serial device. Interface 0 carries CAT; interface 1 is silent.
const PORTS: SerialPortInfo[] = [
  { name: '/dev/cu.usbserial-01AF7FED0', label: 'CP2105', interfaceIndex: 0, pairedAudio: 'USB Audio Device #2' },
  { name: '/dev/cu.usbserial-01AF7FED1', label: 'CP2105', interfaceIndex: 1, pairedAudio: 'USB Audio Device #2' },
  { name: '/dev/cu.usbserial-01A98F800', label: 'CP2105', interfaceIndex: 0, pairedAudio: 'USB Audio Device' },
  { name: '/dev/tty.usbserial-01A98F800', label: 'CP2105', interfaceIndex: 0, pairedAudio: 'USB Audio Device' },
  { name: '/dev/cu.usbmodem601NTGYJF9992', label: 'LG Monitor Controls', interfaceIndex: null, pairedAudio: null },
]

const base: RigFormFacts = {
  serialPort: '/dev/cu.usbserial-01AF7FED0',
  rigConn: 'serial',
  pttMethod: 'cat',
  rigModel: 1049,
}

// Two rigs, each an internal USB hub: the FT-710 at 0x110000, the FTX-1 at 0x120000. Both
// codecs report the same name and are told apart only by a positional " #2".
const AUDIO: { input: AudioDeviceInfo[]; output: AudioDeviceInfo[] } = {
  input: [
    { name: 'USB Audio Device', label: 'USB Audio Device', usbHub: 0x120000 },
    { name: 'USB Audio Device #2', label: 'USB Audio Device #2', usbHub: 0x110000 },
  ],
  output: [
    { name: 'USB Audio Device', label: 'USB Audio Device', usbHub: 0x120000 },
    { name: 'USB Audio Device #2', label: 'USB Audio Device #2', usbHub: 0x110000 },
  ],
}

// Stands in for `getPortlessRigModels()`: Hamlib's low range plus two software-CAT profiles.
// The real list comes from Rust, where rigmodels.rs pins it against the predicate it mirrors.
const PORTLESS = [0, 1, 2, 3, 4, 2054, 23005]

describe('rig form pre-save checks', () => {
  it('a correct configuration raises nothing', () => {
    expect(checkRigForm(base, PORTS)).toEqual([])
    expect(blocks(checkRigForm(base, PORTS))).toBe(false)
  })

  it('warns when the sound card is not inside the radio on this CAT port', () => {
    // THE swap. `…FED0` is the FT-710, whose codec is "USB Audio Device #2" — but the profile
    // names "USB Audio Device", which is the FTX-1's. Exactly what a rig moving USB port causes,
    // silently, to every saved profile.
    const c = checkRigForm(
      { ...base, serialPort: '/dev/cu.usbserial-01AF7FED0', audioIn: 'USB Audio Device' },
      PORTS,
      AUDIO,
    )
    expect(c.some((x) => /not inside the radio on/.test(x.message))).toBe(true)
    // A separate interface box is a legitimate setup, so this informs rather than refuses.
    expect(blocks(c)).toBe(false)
  })

  it('says nothing when the sound card IS the one inside that radio', () => {
    const c = checkRigForm(
      { ...base, serialPort: '/dev/cu.usbserial-01AF7FED0', audioIn: 'USB Audio Device #2' },
      PORTS,
      AUDIO,
    )
    expect(c).toEqual([])
  })

  it('stays silent when topology is unknown (non-macOS, or an unresolvable device)', () => {
    const noTopo = { input: [{ name: 'USB Audio Device', label: 'x' }], output: [] }
    const c = checkRigForm(
      { ...base, serialPort: '/dev/cu.usbserial-01AF7FED0', audioIn: 'USB Audio Device' },
      PORTS,
      noTopo,
    )
    expect(c.some((x) => /not inside the radio/.test(x.message))).toBe(false)
  })

  it('flags the silent second interface of a dual bridge', () => {
    // THE convincing failure: the rig is fine, the port answers nothing, and it looks dead.
    const c = checkRigForm({ ...base, serialPort: '/dev/cu.usbserial-01AF7FED1' }, PORTS)
    expect(c.some((x) => /port 2 of this device/.test(x.message))).toBe(true)
    // A warning, not a block — an unusual rig might genuinely use it.
    expect(blocks(c)).toBe(false)
  })

  it('blocks a dial-in tty device, which would just hang', () => {
    const c = checkRigForm({ ...base, serialPort: '/dev/tty.usbserial-01A98F800' }, PORTS)
    expect(blocks(c)).toBe(true)
    expect(c.some((x) => /dial-in/.test(x.message))).toBe(true)
  })

  // Port collisions belong to the backend (`settings::serial_port_conflicts`), which App.tsx
  // already surfaces as `radioConfigWarning`. The copy that used to live here dropped all four of
  // that rule's qualifiers (enabled / rig_model > 0 / serial conn / non-empty port), compared
  // case-sensitively, and BLOCKED where the real rule warns — so it refused to save a station
  // that shares one cable between two rigs on purpose.
  it('says nothing about two radios sharing a port — that rule lives in the backend', () => {
    const c = checkRigForm({ ...base, serialPort: '/dev/cu.usbserial-01A98F800' }, PORTS)
    expect(c.some((x) => /already uses|cannot share/i.test(x.message))).toBe(false)
  })

  it('warns when the chosen port is not connected', () => {
    const c = checkRigForm({ ...base, serialPort: '/dev/cu.usbserial-GONE' }, PORTS)
    expect(c.some((x) => /not connected right now/.test(x.message))).toBe(true)
    // Absent is not wrong — the rig may simply be switched off.
    expect(blocks(c)).toBe(false)
  })

  it('blocks CAT keying with no rig model', () => {
    const c = checkRigForm({ ...base, rigModel: 0 }, PORTS)
    expect(blocks(c)).toBe(true)
    expect(c.some((x) => /rig model is None\/VOX/.test(x.message))).toBe(true)
  })

  it('blocks a rig model with no port at all', () => {
    const c = checkRigForm({ ...base, serialPort: '  ' }, PORTS, undefined, PORTLESS)
    expect(blocks(c)).toBe(true)
  })

  // THE GATE. A whole class of models is served over TCP or a virtual COM pair by a program on
  // this machine, and is configured with NO port on purpose. Ungated, the check above called
  // every one of them an error and refused the save.
  it.each([
    [4, 'FLRig'],
    [2054, 'Thetis'],
    [23005, 'SmartSDR'],
  ])('allows model %i (%s) with no port — served by software, not a cable', (model) => {
    const c = checkRigForm(
      { ...base, serialPort: '', rigModel: model as number, pttMethod: 'rts' },
      PORTS,
      undefined,
      PORTLESS,
    )
    expect(c).toEqual([])
  })

  it('does not block when the portless rule could not be read', () => {
    // Empty list = the backend could not answer. Blocking then would make an unreadable rule the
    // reason an operator cannot save a configuration that is fine.
    const c = checkRigForm({ ...base, serialPort: '  ' }, PORTS, undefined, [])
    expect(blocks(c)).toBe(false)
  })

  it('survives a non-array where the rule should be, rather than throwing mid-save', () => {
    // This runs inside the save handler; a throw aborts the save with no message at all.
    const c = checkRigForm(
      { ...base, serialPort: '  ' },
      PORTS,
      undefined,
      null as unknown as number[],
    )
    expect(blocks(c)).toBe(false)
  })

  it('says nothing at all about a network rig', () => {
    expect(checkRigForm({ ...base, rigConn: 'network', serialPort: '' }, PORTS)).toEqual([])
  })

  it('a monitor is not silently blessed just because it enumerates as a serial device', () => {
    // The LG monitor WAS chosen as a CAT port, twice. It is a real serial device, so nothing can
    // honestly call it impossible — but pairing it with CAT keying and no rig model is caught,
    // and with a model set the operator at least gets the port-1/port-2 and clash checks.
    const c = checkRigForm(
      { ...base, serialPort: '/dev/cu.usbmodem601NTGYJF9992', rigModel: 0 },
      PORTS,
    )
    expect(blocks(c)).toBe(true)
  })
})
