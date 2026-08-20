// @vitest-environment jsdom
//
// THE PANADAPTER CONTROLS MUST OUTLIVE THE FEED (2026-08-20).
//
// Two operator-reported faults in a row came from cockpit JSX gates, and both walked past a suite of
// 3400 tests because nothing here ever rendered a cockpit with an FT-710 RF scope and asked which
// controls exist. This file does exactly that, and only that.
//
// The fault it pins: the panadapter block was gated on RF rows ARRIVING. In FIX with no start
// stated no rows arrive BY DESIGN, so the block unmounted and took the "FIX starts here" button with
// it — the one control that would have brought the rows back. `PhoneScope` is stubbed here, so no
// feed is ever reported, which IS that state: the radio has a scope, Nexus cannot place the sweep.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, screen, cleanup } from '@testing-library/react'
import type { AppSnapshot } from '../types'
import type { PanelLayoutApi } from '../features/paneLayout'
import { PhoneCockpit } from './PhoneCockpit'
import { CwCockpit } from './CwCockpit'

vi.mock('../api', () => ({
  setPtt: vi.fn(async () => {}),
  setRfPower: vi.fn(async () => {}),
  setMicGain: vi.fn(async () => {}),
  setNrLevel: vi.fn(async () => {}),
  setAgc: vi.fn(async () => ({})),
  setScopeSpan: vi.fn(async () => ({})),
  setScopeRef: vi.fn(async () => {}),
  setFlexPanSpan: vi.fn(async () => ({})),
  setFlexPanRef: vi.fn(async () => ({})),
  startQsoRecording: vi.fn(async () => ({})),
  stopQsoRecording: vi.fn(async () => ({})),
  setTune: vi.fn(async () => ({})),
  haltTx: vi.fn(async () => ({})),
  setFrequency: vi.fn(async () => ({})),
  setSplit: vi.fn(async () => ({})),
  setRigFunc: vi.fn(async () => ({})),
  setSidebandOverride: vi.fn(async () => ({})),
  setFilterWidth: vi.fn(async () => ({})),
  openPanelWindow: vi.fn(async () => {}),
  getVoiceMessages: vi.fn(async () => []),
  playVoiceMessage: vi.fn(async () => ({})),
  stopVoice: vi.fn(async () => ({})),
  startVoiceRecording: vi.fn(async () => ({})),
  stopVoiceRecording: vi.fn(async () => []),
  cancelVoiceRecording: vi.fn(async () => ({})),
  clearVoiceMessage: vi.fn(async () => []),
  importVoiceMessage: vi.fn(async () => []),
  getSettings: vi.fn(async () => ({ macros: { cwProfiles: [], activeCwProfile: 0 } })),
  setSettings: vi.fn(async () => ({})),
  sendCw: vi.fn(async () => {}),
  setCwKeyer: vi.fn(async () => null),
  setCwWpm: vi.fn(async () => {}),
  stopCw: vi.fn(async () => {}),
  cwDecode: vi.fn(async () => decodeState),
  cwClear: vi.fn(async () => {}),
  setAiCw: vi.fn(async () => {}),
  selectPeer: vi.fn(async () => null),
  previewCw: vi.fn(async (t: string) => t),
  pointRotatorAtCall: vi.fn(async () => 0),
  // The real CockpitHeader hosts RotorStrip, which polls these on mount.
  readRotator: vi.fn(async () => null),
  stopRotator: vi.fn(async () => ({})),
  getDeclination: vi.fn(async () => 0),
  getSatTrackStatus: vi.fn(async () => null),
  getSatTransponder: vi.fn(async () => null),
  setSatTransponder: vi.fn(async () => {}),
  stopSatTrack: vi.fn(async () => ({})),
  getRttyState: vi.fn(async () => rttyState),
  getLicensedBandPlan: vi.fn(async () => []),
  rttyArm: vi.fn(async () => rttyState),
  rttySend: vi.fn(async () => rttyState),
  rttyStop: vi.fn(async () => rttyState),
  rttyClear: vi.fn(async () => rttyState),
  rttyAfcReset: vi.fn(async () => rttyState),
  rttyNet: vi.fn(async () => rttyState),
  rttySetAuto: vi.fn(async () => rttyState),
  rttyAutoCq: vi.fn(async () => rttyState),
  rttyAutoAnswer: vi.fn(async () => rttyState),
  rttyAutoAbort: vi.fn(async () => rttyState),
  getPskState: vi.fn(async () => pskState),
  pskArm: vi.fn(async () => pskState),
  pskAutoArm: vi.fn(async () => pskState),
  pskClear: vi.fn(async () => pskState),
  pskAfcReset: vi.fn(async () => pskState),
  pskNet: vi.fn(async () => pskState),
  pskSend: vi.fn(async () => pskState),
  pskSetLatched: vi.fn(async () => pskState),
  pskType: vi.fn(async () => pskState),
  pskStop: vi.fn(async () => pskState),
  getSstvState: vi.fn(async () => sstvState),
  sstvArm: vi.fn(async () => sstvState),
  sstvAutoArm: vi.fn(async () => sstvState),
  sstvSend: vi.fn(async () => sstvState),
  sstvStop: vi.fn(async () => sstvState),
  setOperatingMode: vi.fn(async () => ({})),
}))
vi.mock('../toast', () => ({
  pushToast: vi.fn(),
  withErrorToast: vi.fn(async (action: () => Promise<unknown>) => action()),
}))
// Canvas/scope children only. CockpitHeader is DELIBERATELY REAL — see the file header.
vi.mock('./PhoneScope', () => ({ PhoneScope: () => <div data-testid="scope-stub" /> }))
vi.mock('./BandStrip', () => ({ BandStrip: () => <div data-testid="bandstrip-stub" /> }))
vi.mock('./LogEntry', () => ({ LogEntry: () => <div data-testid="log-stub" /> }))
vi.mock('./SpotDialog', () => ({ SpotDialog: () => null }))
vi.mock('./Waterfall', () => ({ Waterfall: () => <div className="waterfall-wrap" /> }))
const radio = {
  dialMhz: 14.2,
  band: '20m',
  catOk: true,
  sideband: 'USB',
  sidebandOverride: null,
  rigMode: 'USB',
  transmitting: false,
  tuning: false,
  txEnabled: true,
  txAllowed: true,
  qsoRecording: false,
  rfPower: null,
  micGain: null,
  nrLevel: 0.3,
  agc: 'fast',
  nb: true,
  nr: true,
  notch: null,
  comp: null,
  vox: null,
  filterWidthHz: 500,
  splitTxMhz: null,
  smeterDb: null,
  cwWpm: 22,
  cwKeyer: 'cat',
  phoneSegLo: null,
  phoneSegHi: null,
}
const snap = { mycall: 'KD9TAW', radio } as unknown as AppSnapshot


function panelsWith<P extends string>(removed: readonly P[]): PanelLayoutApi<P> {
  return {
    layout: { v: 1, state: {}, share: {} },
    stateOf: (id) => (removed.includes(id) ? 'removed' : 'docked'),
    setPanelState: () => {},
    shareOf: () => 1,
    setShare: () => {},
    setShares: () => {},
    undo: () => {},
    canUndo: false,
    undoRemoves: [],
    reset: () => {},
  }
}

const panels = panelsWith<string>([])

/** A snapshot whose radio reports the rig's scope MODE — the `SS` P3 byte, widened for JSON. */
function snapWithMode(code: number): AppSnapshot {
  return { mycall: 'KD9TAW', radio: { ...radio, scopeModeCode: code } } as unknown as AppSnapshot
}

const FIX_NORMAL = 0x41 // 'A' — W/F FIX (NORMAL)
const CENTER_NORMAL = 0x34 // '4' — W/F CENTER (NORMAL)
const CURSOR_NORMAL = 0x37 // '7' — W/F CURSOR (NORMAL)

globalThis.ResizeObserver = class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver

afterEach(() => cleanup())

const cockpits = [
  { name: 'Phone', render: (s: AppSnapshot) => render(<PhoneCockpit snap={s} theme="dark" onWorkSpot={() => {}} spots={[]} panels={panels} />) },
  { name: 'CW', render: (s: AppSnapshot) => render(<CwCockpit snap={s} theme="dark" onWorkSpot={() => {}} spots={[]} panels={panels} />) },
]

describe.each(cockpits)('$name cockpit panadapter controls', ({ render: mount }) => {
  it('are present while the sweep cannot be placed — no feed, FIX, no start stated', () => {
    mount(snapWithMode(FIX_NORMAL))
    // The controls, found the way the operator finds them.
    expect(screen.getByLabelText('Panadapter span (sets the radio)')).toBeTruthy()
    expect(screen.getByLabelText('Panadapter position (sets the radio)')).toBeTruthy()
    // And the way OUT of the unplaceable state. This is the assertion the shipped bug failed.
    expect(screen.getByRole('button', { name: 'FIX starts here' })).toBeTruthy()
  })

  it('offer no FIX-start action when the sweep is not in FIX', () => {
    // The button means nothing in CENTER or CURSOR: a start would be a value with no window to
    // apply it to. Its absence is as much a requirement as its presence above.
    for (const code of [CENTER_NORMAL, CURSOR_NORMAL]) {
      mount(snapWithMode(code))
      expect(screen.queryByRole('button', { name: 'FIX starts here' })).toBeNull()
      cleanup()
    }
  })

  it('show the position the RADIO reports, not a local default', () => {
    mount(snapWithMode(CURSOR_NORMAL))
    const sel = screen.getByLabelText('Panadapter position (sets the radio)') as HTMLSelectElement
    expect(sel.value).toBe('cursor')
  })

  it('stay away entirely when the radio has no scope Nexus has read', () => {
    // No mode code = not an FT-710 with the bridge running. The audio cockpit must be unchanged.
    render(<PhoneCockpit snap={{ mycall: 'KD9TAW', radio } as unknown as AppSnapshot} theme="dark" onWorkSpot={() => {}} spots={[]} panels={panels} />)
    expect(screen.queryByLabelText('Panadapter span (sets the radio)')).toBeNull()
  })
})
