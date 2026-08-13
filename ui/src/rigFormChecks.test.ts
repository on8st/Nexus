import { describe, it, expect } from 'vitest'
import { checkRigForm, blocks, type RigFormFacts } from './rigFormChecks'
import type { SerialPortInfo } from './api'

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
  activeRadio: 0,
  radios: [
    { id: 0, name: 'FT-710', serialPort: '/dev/cu.usbserial-01AF7FED0' },
    { id: 1, name: 'FTX-1', serialPort: '/dev/cu.usbserial-01A98F800' },
  ],
}

describe('rig form pre-save checks', () => {
  it('a correct configuration raises nothing', () => {
    expect(checkRigForm(base, PORTS, 0)).toEqual([])
    expect(blocks(checkRigForm(base, PORTS, 0))).toBe(false)
  })

  it('flags the silent second interface of a dual bridge', () => {
    // THE convincing failure: the rig is fine, the port answers nothing, and it looks dead.
    const c = checkRigForm({ ...base, serialPort: '/dev/cu.usbserial-01AF7FED1' }, PORTS, 0)
    expect(c.some((x) => /port 2 of this device/.test(x.message))).toBe(true)
    // A warning, not a block — an unusual rig might genuinely use it.
    expect(blocks(c)).toBe(false)
  })

  it('blocks a dial-in tty device, which would just hang', () => {
    const c = checkRigForm({ ...base, serialPort: '/dev/tty.usbserial-01A98F800' }, PORTS, 0)
    expect(blocks(c)).toBe(true)
    expect(c.some((x) => /dial-in/.test(x.message))).toBe(true)
  })

  it('blocks two radios sharing one CAT port, and names the other radio', () => {
    // Editing radio 0, but pointing it at radio 1's port.
    const c = checkRigForm({ ...base, serialPort: '/dev/cu.usbserial-01A98F800' }, PORTS, 0)
    expect(blocks(c)).toBe(true)
    expect(c.some((x) => x.message.includes('FTX-1'))).toBe(true)
  })

  it('does not call a radio a clash with ITSELF', () => {
    // Editing radio 1, on radio 1's own port — the commonest re-save there is.
    const c = checkRigForm({ ...base, serialPort: '/dev/cu.usbserial-01A98F800', activeRadio: 1 }, PORTS, 1)
    expect(blocks(c)).toBe(false)
  })

  it('warns when the chosen port is not connected', () => {
    const c = checkRigForm({ ...base, serialPort: '/dev/cu.usbserial-GONE' }, PORTS, 0)
    expect(c.some((x) => /not connected right now/.test(x.message))).toBe(true)
    // Absent is not wrong — the rig may simply be switched off.
    expect(blocks(c)).toBe(false)
  })

  it('blocks CAT keying with no rig model', () => {
    const c = checkRigForm({ ...base, rigModel: 0 }, PORTS, 0)
    expect(blocks(c)).toBe(true)
    expect(c.some((x) => /rig model is None\/VOX/.test(x.message))).toBe(true)
  })

  it('blocks a rig model with no port at all', () => {
    const c = checkRigForm({ ...base, serialPort: '  ' }, PORTS, 0)
    expect(blocks(c)).toBe(true)
  })

  it('says nothing at all about a network rig', () => {
    expect(checkRigForm({ ...base, rigConn: 'network', serialPort: '' }, PORTS, 0)).toEqual([])
  })

  it('a monitor is not silently blessed just because it enumerates as a serial device', () => {
    // The LG monitor WAS chosen as a CAT port, twice. It is a real serial device, so nothing can
    // honestly call it impossible — but pairing it with CAT keying and no rig model is caught,
    // and with a model set the operator at least gets the port-1/port-2 and clash checks.
    const c = checkRigForm(
      { ...base, serialPort: '/dev/cu.usbmodem601NTGYJF9992', rigModel: 0 },
      PORTS,
      0,
    )
    expect(blocks(c)).toBe(true)
  })
})
