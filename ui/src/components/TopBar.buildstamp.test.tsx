// @vitest-environment jsdom
//
// The version chip names the build it came from. Identifying a running binary on
// 2026-08-13 needed a hunt for a string literal one commit had added and its parent
// had not — because the product version is identical across every branch built from
// the same baseline. These pin both directions: the stamp shows when the backend
// answers, and the chip is UNCHANGED when it does not (an older backend has no
// `build_id` command, and a missing build stamp must never cost the version).
import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, screen, cleanup, waitFor } from '@testing-library/react'
import { TopBar } from './TopBar'
import type { RadioStatus } from '../types'

vi.mock('../api', () => ({
  appVersion: vi.fn(() => Promise.resolve('1.2.9')),
  buildId: vi.fn(() => Promise.resolve('on8st/Nexus macos-support@abc1234')),
}))

import * as api from '../api'

function radio(): RadioStatus {
  return {
    dialMhz: 14.074,
    band: '20m',
    sideband: 'USB',
    slot: 0,
    source: 'native',
    sourceLabel: 'Native',
    nextSlotMs: 5000,
    rxOffsetHz: 1500,
    txOffsetHz: 1500,
    txLevel: 0.5,
    rxLevel: 0.3,
    txEven: true,
    txCycleAuto: true,
    txEnabled: false,
    txAllowed: true,
    transmitting: false,
    tuning: false,
    qsoRecording: false,
    catOk: true,
    dtSec: 0,
    clockOffsetMs: 0,
  } as unknown as RadioStatus
}

function renderBar() {
  const noop = () => {}
  return render(
    <TopBar
      mycall="KD9TAW"
      mygrid="EN61"
      radio={radio()}
      link={{ tier: 'FT8' } as never}
      bandPlan={[]}
      onSetFrequency={noop}
      onSetTxEnabled={noop}
      onSetTune={noop}
      onHaltTx={noop}
      onSetTxEven={noop}
      onSetTxCycleAuto={noop}
      onSetHoldTxFreq={noop}
      tier="FT8"
      onTierChange={noop}
      onOpenGuide={noop}
    />,
  )
}

afterEach(cleanup)

describe('TopBar build stamp', () => {
  it('names the fork, branch and commit in the version chip tooltip', async () => {
    const { container } = renderBar()
    await waitFor(() => {
      const chip = container.querySelector('.app-version')
      expect(chip?.getAttribute('title')).toContain('on8st/Nexus macos-support@abc1234')
    })
  })

  it('still shows the version when the backend has no build_id command', async () => {
    vi.mocked(api.buildId).mockRejectedValueOnce(new Error('unknown command: build_id'))
    const { container } = renderBar()
    // The version must arrive regardless — the stamp is additive, never a gate on it.
    await waitFor(() => expect(screen.getByText('v1.2.9')).toBeTruthy())
    // And no half-written tooltip: absent, not the string "undefined".
    expect(container.querySelector('.app-version')?.getAttribute('title')).toBeNull()
  })
})
