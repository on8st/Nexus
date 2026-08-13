// @vitest-environment jsdom
//
// The per-radio Edit flow (0.17.0) decoupled "which radio the rig form edits" from "which radio
// is active", but only Save was taught to route accordingly. Test CAT and Auto-test still sent
// the whole flat form, and the backend folds a flat payload into the profile named by its
// `activeRadio` — so configuring radio 2 and pressing either button stamped radio 2's COM port,
// model and audio devices onto radio 1's profile, persisted. Operator report, 2026-07-25: with
// two radios configured, both ended up on one set of comm ports.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/react'
import { SettingsPanel } from './SettingsPanel'
import type { FeaturesApi } from '../useFeatures'
import defaultSettings from './__fixtures__/defaultSettings.json'

const api = vi.hoisted(() => {
  // SettingsPanel pulls ~50 verbs from ../api. They all resolve null, which is enough for a
  // mount — the assertions here are about WHICH save verb gets called, not what it returns.
  const VERBS = [
    'clearCloudlogKey', 'clearClublogPassword', 'clearEqslPassword', 'clearHamqthPassword',
    'clearHrdlogCode', 'clearLotwPassword', 'clearQrzLogbookKey', 'clearQrzPassword', 'detectRigs',
    'downloadEqslReport', 'downloadLotwReport', 'getAllRigModels', 'getAudioDevices', 'audioDevicesForPort', 'getBandPlan',
    'getRigModels', 'getSerialPortsDetailed', 'getSettings', 'setCloudlogKey', 'setClublogPassword',
    'setEqslPassword', 'setHamqthPassword', 'setHrdlogCode', 'setLotwPassword', 'setQrzLogbookKey',
    'setQrzPassword', 'setRepeaterbookToken', 'setRxGain', 'setSettings', 'setTxLevel', 'addRadio',
    'removeRadio', 'renameRadio', 'setActiveRadio', 'setRadioBands', 'updateRadioProfile', 'testCat',
    'probeCatPorts', 'qrzTestConnection', 'syncQrz', 'n3fjpTestConnection', 'getConnectionLog',
    'getCredentialsStatus', 'fetchLotwUsers', 'getLotwUsersStatus', 'fetchFccStates',
    'getFccStatesStatus', 'getTleStatus', 'fetchTlesNow', 'importTles', 'discoverFlex', 'civDiagnosticLog', 'civDiagnosticStatus',
    'allTxtLocation', 'revealAllTxt', 'recordingsLocation', 'revealRecordings', 'appVersion', 'getSpectrumRow', 'setFrequency',
    'getWatchlist', 'setWatchlist', 'openPanelWindow', 'getAssistanceJournal',
    'setUnassistedMode',
  ]
  const spies: Record<string, ReturnType<typeof vi.fn>> = {}
  const get = (name: string) => {
    if (!spies[name]) spies[name] = vi.fn(() => Promise.resolve(null))
    return spies[name]
  }
  return { spies, get, VERBS }
})

vi.mock('../api', () => {
  const mod: Record<string, unknown> = {}
  for (const v of api.VERBS) mod[v] = api.get(v)
  return mod
})
vi.mock('../toast', () => ({
  pushToast: vi.fn(),
  withErrorToast: vi.fn(async (fn: () => Promise<unknown>) => fn()),
}))

const FTDX10 = {
  id: 0,
  name: 'FTDX10',
  enabled: true,
  serialPort: 'COM3',
  baud: 38400,
  rigModel: 1042,
  rigModelName: 'Yaesu FTDX10',
  rigConn: 'serial',
  rigAddr: '',
  rigctldPort: 4532,
  rotctldPort: 4533,
  icomNativeCat: false,
  audioIn: 'in-0',
  audioOut: 'out-0',
  txLevel: 1,
  rxGain: 1,
  pttMethod: 'cat',
  rotatorModel: 0,
  rotatorPort: '',
  rotatorBaud: 9600,
  rotatorHost: '',
  nativeScope: 'auto',
  bands: [],
}
const IC9700 = {
  ...FTDX10,
  id: 1,
  name: 'IC-9700',
  serialPort: 'COM7',
  baud: 115200,
  rigModel: 3081,
  rigModelName: 'Icom IC-9700',
  rigctldPort: 4534,
  rotctldPort: 4535,
  icomNativeCat: true,
  audioIn: 'in-1',
  audioOut: 'out-1',
}

/** Settings with two radios, FTDX10 active, flat mirror = the FTDX10.
 * Built on the real `Settings::default()` (dumped from Rust) so the panel renders for real
 * rather than against a hand-guessed subset that drifts. */
function twoRadioSettings() {
  return {
    ...defaultSettings,
    ...FTDX10,
    mycall: 'KD9TAW',
    mygrid: 'EN52',
    activeRadio: 0,
    radios: [FTDX10, IC9700],
    band: '20m',
    dialMhz: 14.074,
    sideband: 'USB',
  } as never
}

const features: FeaturesApi = {
  enabled: () => true,
  setEnabled: vi.fn(),
  all: () => [],
  profile: 'full',
  setProfile: vi.fn(),
} as unknown as FeaturesApi

function renderPanel() {
  return render(
    <SettingsPanel
      activeRadioId={0}
      scale={1 as never}
      scaleMode={'auto' as never}
      scaleCap={1 as never}
      onScaleModeChange={() => {}}
      onScaleCapChange={() => {}}
      density={'comfortable' as never}
      onDensityChange={() => {}}
      onResetLayout={() => {}}
      features={features}
    />,
  )
}

beforeEach(() => {
  // Reset in place — the mocked module captured these exact spy objects at import time, so
  // replacing them here would leave the component calling the old ones.
  for (const spy of Object.values(api.spies)) {
    spy.mockClear()
    spy.mockImplementation(() => Promise.resolve(null))
  }
  api.get('getRigModels').mockImplementation(() => Promise.resolve([]))
  api.get('getAllRigModels').mockImplementation(() => Promise.resolve([]))
  api.get('getSerialPortsDetailed').mockImplementation(() => Promise.resolve([]))
  api.get('getBandPlan').mockImplementation(() => Promise.resolve([]))
  api.get('getAudioDevices').mockImplementation(() => Promise.resolve({ input: [], output: [] }))
  api.get('getCredentialsStatus').mockImplementation(() => Promise.resolve({}))
  api.get('detectRigs').mockImplementation(() => Promise.resolve([]))
  api.get('appVersion').mockImplementation(() => Promise.resolve('0.17.12'))
  api.get('getSettings').mockImplementation(() => Promise.resolve(twoRadioSettings()))
  api.get('probeCatPorts').mockImplementation(() =>
    Promise.resolve({
      found: true,
      detail: 'Icom IC-9700 on COM7 @ 115200 baud',
      portName: 'COM7',
      baud: 115200,
      model: 3081,
      modelName: 'Icom IC-9700',
      freqMhz: 144.174,
      modelSeeded: false,
    }),
  )
  api.get('testCat').mockImplementation(() => Promise.resolve({ ok: true, detail: 'ok' }))
})
afterEach(cleanup)


describe('Auto-test proposes; it never applies or saves on its own', () => {
  // THE 2026-08-13 INCIDENT. `handleAutoTestPorts` used to write the probe result into the form
  // and PERSIST it immediately. A port sweep answers with whichever rig replies first, and
  // `probe_cat_ports` only excludes ports ALREADY CONFIGURED on another profile — so on a station
  // with two radios and one profile it reached the other radio, and an FTX-1's profile was saved
  // pointing at an FT-710's CAT port, with the FT-710's model. Silent, persisted, and it looks
  // exactly like a dead radio afterwards.
  //
  // The probe here reports an IC-9700 on COM7 while the form describes an FTDX10 — i.e. it found
  // the wrong rig, which is precisely the case that must not be written.

  it('a found port is NOT saved — no settings write of any kind', async () => {
    renderPanel()
    fireEvent.click(await screen.findByRole('tab', { name: 'Radio' }))
    fireEvent.click(await screen.findByRole('button', { name: /auto-test/i }))

    await waitFor(() => expect(api.get('probeCatPorts')).toHaveBeenCalled())
    // The whole point: nothing reached disk.
    expect(api.get('setSettings')).not.toHaveBeenCalled()
    expect(api.get('updateRadioProfile')).not.toHaveBeenCalled()
  })

  it('it asks, naming the rig that actually answered and the radio it would change', async () => {
    renderPanel()
    fireEvent.click(await screen.findByRole('tab', { name: 'Radio' }))
    fireEvent.click(await screen.findByRole('button', { name: /auto-test/i }))

    // The operator must be able to see BOTH facts: what answered, and what it would change.
    await waitFor(() => expect(document.body.textContent).toContain('COM7'))
    expect(document.body.textContent).toContain('IC-9700')
    expect(document.body.textContent).toContain('FTDX10') // the radio being configured
    expect(screen.getByRole('button', { name: 'Apply' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Dismiss' })).toBeTruthy()
  })

  it('Apply fills the form but STILL does not save', async () => {
    renderPanel()
    fireEvent.click(await screen.findByRole('tab', { name: 'Radio' }))
    fireEvent.click(await screen.findByRole('button', { name: /auto-test/i }))
    fireEvent.click(await screen.findByRole('button', { name: 'Apply' }))

    await waitFor(() => expect(screen.queryByRole('button', { name: 'Apply' })).toBeNull())
    // Applying is a form edit. Persisting stays the operator's Save — a wrong guess costs a
    // glance, not a silent rewrite of a working profile.
    expect(api.get('setSettings')).not.toHaveBeenCalled()
    expect(api.get('updateRadioProfile')).not.toHaveBeenCalled()
  })

  it('Dismiss drops it, changing nothing', async () => {
    renderPanel()
    fireEvent.click(await screen.findByRole('tab', { name: 'Radio' }))
    fireEvent.click(await screen.findByRole('button', { name: /auto-test/i }))
    fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }))

    await waitFor(() => expect(screen.queryByRole('button', { name: 'Apply' })).toBeNull())
    expect(api.get('setSettings')).not.toHaveBeenCalled()
    expect(api.get('updateRadioProfile')).not.toHaveBeenCalled()
  })
})
