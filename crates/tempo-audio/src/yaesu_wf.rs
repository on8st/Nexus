//! FT-710 waterfall over the radio's internal FT4222 USB→SPI bridge.
//!
//! WHAT THIS IS. The FT-710 contains an FT4222 USB→SPI bridge. With *SCU-LAN10* enabled in the
//! radio's menu it appears as a THIRD USB function beside the CAT bridge and the codec, on the
//! same internal hub:
//!
//! ```text
//! IOUSBHostDevice@02400000                 ← the FT-710's internal hub
//!   ├─ CP2105 Dual USB to UART @02410000   ← CAT
//!   ├─ USB Audio Device        @02420000   ← the codec
//!   └─ FT4222                  @02430000   ← this  (FTDI, VID 0x0403, PID 0x601C)
//! ```
//!
//! It is neither a serial port nor a CAT command, and both of those were ruled out by measurement
//! before this module existed: `SS` (SPECTRUM SCOPE) reads and writes scope SETTINGS only — the
//! official FT-710 CAT manual has no waveform command at all — and nothing arrives unsolicited on
//! either virtual COM port. Measured on ON8ST's station 2026-08-17: 40 frames of 4096 B in 0.48 s
//! (84/s), 852 live bins, 821 of 852 changing between the first and last frame. See FORK.md for
//! the full feasibility record.
//!
//! WHY THE SPLIT IN THIS FILE. Everything that decides what the bins MEAN is pure and tested here
//! ([`parse_wf1`], [`span_hz`], [`sweep_edges`]). The only part that needs FTDI's closed-source
//! library sits behind the off-by-default `yaesu-wf` feature and behind [`WaterfallSource`], so
//! the parsing is exercised by the suite on every platform and the FFI is the one thing a
//! licensing decision gates. **No FTDI binary is vendored into this repo** — see FORK.md: D2XX /
//! LibFT4222 is closed source and Nexus is GPL-3.0-only, which is an open question, not a settled
//! one. Fork-local until it is answered.
//!
//! HOW IT MEETS THE REST OF THE APP. The output shape is deliberately the same as the CI-V
//! scope's [`crate::civ::scope::ScopeSweep`]: bins normalised 0..1 plus the absolute RF span, so
//! the existing `rigscope` pane and `SpectrumFeed` take it with no new UI or DSP. Which RADIO a
//! bridge belongs to is answered by [`crate::usbtopo`] — the FT4222 shares its parent hub with
//! that rig's CAT port and codec, the same evidence that already labels the codec.

/// One SPI read from the bridge. Fixed size; the radio does not frame or delimit.
pub const FRAME_BYTES: usize = 4096;
/// Receiver 1's waterfall line: `uint8` per bin, at the start of the frame.
pub const WF1_OFFSET: usize = 0;
/// Bin count for receiver 1 — the width of one waterfall line.
pub const WF1_BINS: usize = 852;
/// Receiver 2's line. Present in the layout for the FTDX101 series; unused on the FT-710.
pub const WF2_OFFSET: usize = 852;
/// AF-FFT for receiver 1 (`uint8`), after both waterfall lines.
pub const AF1_FFT_OFFSET: usize = 1704;
/// AF-FFT bin count.
pub const AF1_FFT_BINS: usize = 192;
/// AF oscilloscope for receiver 1 (`uint8`, 128 = zero line).
pub const AF1_SCOPE_OFFSET: usize = 1896;
/// AF oscilloscope sample count.
pub const AF1_SCOPE_SAMPLES: usize = 400;

/// Receiver 1's waterfall line, normalised to 0..1 the way [`ScopeSweep::row`] is.
///
/// `None` for a short frame rather than a padded one: a truncated SPI read is a transport fault,
/// and half a line rendered as if it were a full one is worse than a dropped frame — the operator
/// would be reading a spectrum whose right-hand side is silence that is not on the air.
///
/// [`ScopeSweep::row`]: crate::civ::scope::ScopeSweep::row
pub fn parse_wf1(raw: &[u8]) -> Option<Vec<f32>> {
    if raw.len() < WF1_OFFSET + WF1_BINS {
        return None;
    }
    Some(
        raw[WF1_OFFSET..WF1_OFFSET + WF1_BINS]
            .iter()
            .map(|&b| f32::from(b) / 255.0)
            .collect(),
    )
}

/// The scope span in Hz for the `P3` code of `SS<P1>5;` (SPECTRUM SCOPE ▸ SPAN).
///
/// Straight from Yaesu's FT-710 CAT manual, and read off the radio rather than assumed — the app
/// asks `SS05;` and gets e.g. `SS0570000;` = code 7 = 200 kHz. `None` for a code the manual does
/// not define: guessing a span silently mis-scales the whole waterfall, which looks like a
/// mistuned radio rather than a software fault.
pub fn span_hz(code: u8) -> Option<f64> {
    Some(match code {
        b'0' => 1_000.0,
        b'1' => 2_000.0,
        b'2' => 5_000.0,
        b'3' => 10_000.0,
        b'4' => 20_000.0,
        b'5' => 50_000.0,
        b'6' => 100_000.0,
        b'7' => 200_000.0,
        b'8' => 500_000.0,
        b'9' => 1_000_000.0,
        _ => return None,
    })
}

/// Is the scope's `P3` MODE code one of the CENTER modes — i.e. is the span centred on the dial?
///
/// `SS<P1>6;` reports it. CENTER (`3`, `4` for W/F; `0` for 3DSS) means the row is symmetric about
/// the dial. CURSOR and FIX are NOT, and this module refuses to guess their edges: in FIX the
/// window is pinned to a band edge the CAT protocol does not report, so a centred assumption would
/// place every signal at the wrong frequency — a wrong answer that looks authoritative.
pub fn mode_is_centered(code: u8) -> bool {
    matches!(code, b'0' | b'3' | b'4')
}

/// Absolute row edges `(lo_hz, hi_hz)` for a dial and span, or `None` when they cannot be known.
///
/// `None` for a non-CENTER mode, for an undefined span code, and for a dial that would put the row
/// below 0 Hz. Every one of those is "we do not know where this row sits", and the honest response
/// is to render no row rather than a mislabelled one.
pub fn sweep_edges(dial_hz: f64, span_code: u8, mode_code: u8) -> Option<(f64, f64)> {
    if !mode_is_centered(mode_code) {
        return None;
    }
    let span = span_hz(span_code)?;
    let half = span / 2.0;
    (dial_hz - half >= 0.0).then_some((dial_hz - half, dial_hz + half))
}

/// A source of raw 4096-byte frames.
///
/// A trait so the parsing above is testable without hardware AND so the FTDI dependency has one
/// place to live. `Err` is a transport fault; the caller drops the frame and keeps the previous
/// picture rather than blanking the pane on a single hiccup.
pub trait WaterfallSource {
    fn read_frame(&mut self) -> std::io::Result<Vec<u8>>;
}

/// A canned source for tests: cycles the frames it was given.
#[derive(Debug)]
pub struct MockWaterfall {
    frames: Vec<Vec<u8>>,
    next: usize,
}

impl MockWaterfall {
    pub fn new(frames: Vec<Vec<u8>>) -> Self {
        Self { frames, next: 0 }
    }
    /// A frame whose receiver-1 line ramps 0..255 across the bins — a shape whose normalisation
    /// is checkable at both ends and in the middle.
    pub fn ramp() -> Self {
        let mut f = vec![0u8; FRAME_BYTES];
        for (i, b) in f[WF1_OFFSET..WF1_OFFSET + WF1_BINS].iter_mut().enumerate() {
            *b = ((i * 255) / (WF1_BINS - 1)) as u8;
        }
        Self::new(vec![f])
    }
}

impl WaterfallSource for MockWaterfall {
    fn read_frame(&mut self) -> std::io::Result<Vec<u8>> {
        if self.frames.is_empty() {
            return Err(std::io::Error::other("no frames"));
        }
        let f = self.frames[self.next % self.frames.len()].clone();
        self.next += 1;
        Ok(f)
    }
}

/// What one pump attempt did — three outcomes, because two of them are NOT the same failure.
///
/// A dropped frame is transient (a short SPI read, a hiccup) and the caller must KEEP the last
/// picture: blanking the pane on one bad read is a flicker the operator reads as a dying radio.
/// `Unavailable` is persistent — the scope is in a mode whose edges CAT does not report — and
/// there the stale row must be CLEARED, or the pane keeps showing a band that is no longer what
/// the rig is looking at. The CI-V path draws the same line with `clear_rf`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pumped {
    /// A row reached the feed.
    Published,
    /// Transient read fault — keep whatever is on screen.
    Dropped,
    /// The row cannot be placed on the band at all — the caller should clear the RF feed.
    Unavailable,
}

/// What the radio has to tell us over CAT before a row can be placed on the band.
///
/// The bins arrive over SPI with no metadata at all — no centre, no span, no mode. Those three
/// come from `SS<P1>5;` / `SS<P1>6;` and the dial, which is why this transport is useless on its
/// own and pairs with the CAT link rather than replacing it.
#[derive(Debug, Clone, Copy)]
pub struct SweepMeta {
    pub dial_hz: f64,
    /// `P3` of `SS<P1>5;` — the SPAN code, as the ASCII byte the radio sent.
    pub span_code: u8,
    /// `P3` of `SS<P1>6;` — the MODE code, as the ASCII byte the radio sent.
    pub mode_code: u8,
}

/// The feed label for rows from this bridge.
///
/// ⚠️ MUST match `isRfScopeSource` in `ui/src/waterfall.ts`. That predicate decides whether a row
/// spans ABSOLUTE RF Hz or demodulated audio Hz, and an unknown label falls through to the audio
/// reading — so a row would be drawn against a 0–4000 Hz axis with dB-scaled thresholds. Nothing
/// errors; the waterfall is simply wrong, which is the failure this constant exists to prevent.
pub const SOURCE: &str = "yaesu";

/// Read one frame and publish it as an RF row.
///
/// Takes the feed rather than the engine: the CI-V scope learned this the hard way — going through
/// the engine mutex starved the panadapter on the same lock that starved the audio row.
pub fn pump(
    src: &mut dyn WaterfallSource,
    feed: &tempo_app::engine::SpectrumFeed,
    meta: SweepMeta,
) -> Pumped {
    let Some((lo_hz, hi_hz)) = sweep_edges(meta.dial_hz, meta.span_code, meta.mode_code) else {
        return Pumped::Unavailable;
    };
    let Ok(raw) = src.read_frame() else {
        return Pumped::Dropped;
    };
    let Some(row) = parse_wf1(&raw) else {
        return Pumped::Dropped;
    };
    feed.publish_rf(tempo_app::dto::Spectrum {
        row,
        lo_hz,
        hi_hz,
        source: SOURCE.to_string(),
    });
    Pumped::Published
}

/// The FT4222 transport — the ONE part that needs FTDI's closed-source library.
///
/// ⚠️ **FORK-LOCAL, AND OFF BY DEFAULT.** `LibFT4222`/`D2XX` are closed-source binaries and Nexus
/// is GPL-3.0-only; that question is open (FORK.md), so nothing is vendored here and the feature
/// is not enabled in any build. Everything above this module is licence-clean and tested.
///
/// The SPI configuration is not written from memory: the constants below were READ OUT of the
/// `ft4222` Python wrapper that was proven against the radio on 2026-08-17, because a wrong CPOL
/// or clock divider yields plausible-looking garbage rather than an error — the worst failure mode
/// available here.
#[cfg(feature = "yaesu-wf")]
pub mod ft4222 {
    use super::{WaterfallSource, FRAME_BYTES};
    use std::os::raw::{c_int, c_void};

    // Verified against the working wrapper, not the header from memory.
    const SPI_IO_SINGLE: u8 = 1;
    const CLK_DIV_16: u8 = 4;
    const CPOL_IDLE_HIGH: u8 = 1;
    const CPHA_CLK_TRAILING: u8 = 1;
    const SS0: u8 = 1;
    const SYS_CLK_48: u8 = 2;

    type Handle = *mut c_void;

    #[link(name = "ft4222")]
    unsafe extern "C" {
        fn FT_Open(device: c_int, handle: *mut Handle) -> u32;
        fn FT_Close(handle: Handle) -> u32;
        fn FT4222_SetClock(handle: Handle, rate: u8) -> u32;
        fn FT4222_SPIMaster_Init(
            handle: Handle,
            io_line: u8,
            clock: u8,
            cpol: u8,
            cpha: u8,
            sso_map: u8,
        ) -> u32;
        fn FT4222_SPIMaster_SingleRead(
            handle: Handle,
            buffer: *mut u8,
            bytes_to_read: u16,
            size_transferred: *mut u16,
            is_end_transaction: bool,
        ) -> u32;
    }

    /// An opened bridge. Closes on drop.
    pub struct Ft4222Waterfall {
        handle: Handle,
    }

    impl Ft4222Waterfall {
        /// Open interface A of the bridge and put it in SPI-master mode.
        ///
        /// `device` is the index in FTDI's device list; interface A is the data interface (B
        /// exists and carries nothing we want). The first open on a freshly-appeared bridge was
        /// observed to hang for minutes once and be instant every time after, so a caller must
        /// treat this as a blocking call worth a timeout — that is the caller's job, because
        /// D2XX offers no timeout of its own here.
        pub fn open(device: i32) -> std::io::Result<Self> {
            let mut handle: Handle = std::ptr::null_mut();
            let st = unsafe { FT_Open(device, &mut handle) };
            if st != 0 || handle.is_null() {
                return Err(std::io::Error::other(format!("FT_Open failed: {st}")));
            }
            let me = Self { handle };
            let st = unsafe {
                FT4222_SPIMaster_Init(
                    handle,
                    SPI_IO_SINGLE,
                    CLK_DIV_16,
                    CPOL_IDLE_HIGH,
                    CPHA_CLK_TRAILING,
                    SS0,
                )
            };
            if st != 0 {
                return Err(std::io::Error::other(format!(
                    "SPIMaster_Init failed: {st}"
                )));
            }
            let st = unsafe { FT4222_SetClock(handle, SYS_CLK_48) };
            if st != 0 {
                return Err(std::io::Error::other(format!("SetClock failed: {st}")));
            }
            Ok(me)
        }
    }

    impl WaterfallSource for Ft4222Waterfall {
        fn read_frame(&mut self) -> std::io::Result<Vec<u8>> {
            let mut buf = vec![0u8; FRAME_BYTES];
            let mut got: u16 = 0;
            let st = unsafe {
                FT4222_SPIMaster_SingleRead(
                    self.handle,
                    buf.as_mut_ptr(),
                    FRAME_BYTES as u16,
                    &mut got,
                    false,
                )
            };
            if st != 0 {
                return Err(std::io::Error::other(format!("SingleRead failed: {st}")));
            }
            // A short read is handed up as-is; `parse_wf1` refuses it rather than padding.
            buf.truncate(got as usize);
            Ok(buf)
        }
    }

    impl Drop for Ft4222Waterfall {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe { FT_Close(self.handle) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_frame_yields_one_normalised_line_per_bin() {
        let mut src = MockWaterfall::ramp();
        let raw = src.read_frame().unwrap();
        let row = parse_wf1(&raw).expect("a full frame parses");
        assert_eq!(row.len(), WF1_BINS);
        assert!((row[0] - 0.0).abs() < 1e-6, "first bin is the floor");
        assert!(
            (row[WF1_BINS - 1] - 1.0).abs() < 1e-6,
            "last bin is full scale"
        );
        assert!(
            row.iter().all(|v| (0.0..=1.0).contains(v)),
            "row stays in 0..1"
        );
    }

    /// THE PARSER AGAINST A REAL RADIO, not a shape I invented. Captured from an FT-710 on
    /// 2026-08-17 with the SCU-LAN10 option on; the synthetic ramp above proves the arithmetic,
    /// this proves the OFFSETS — a layout error would pass the ramp and fail here.
    #[test]
    fn a_real_ft710_frame_parses_into_a_plausible_spectrum() {
        let raw = include_bytes!("../tests/fixtures/ft710_wf_frame.bin");
        assert_eq!(
            raw.len(),
            FRAME_BYTES,
            "the radio sends fixed 4096-byte frames"
        );
        let row = parse_wf1(raw).expect("a real frame parses");
        assert_eq!(row.len(), WF1_BINS);

        // A live band is neither flat nor saturated: there is a noise floor well above zero and
        // peaks below full scale. Flat would mean we are reading a dead region of the frame.
        let mean = row.iter().sum::<f32>() / row.len() as f32;
        let max = row.iter().cloned().fold(f32::MIN, f32::max);
        assert!(
            (0.3..0.9).contains(&mean),
            "noise floor out of range: mean {mean}"
        );
        assert!(max > mean, "no peaks above the floor — wrong offset?");
        assert!(row.iter().all(|v| (0.0..=1.0).contains(v)));

        // The 144-byte parameter block is NOT usable on this model: 128 zeroes then a repeating
        // `ff 01 ee 01` idle pattern. Pinned so that if a future frame does carry frequencies,
        // this test fails and says so rather than the block being quietly assumed empty forever.
        let params = &raw[3952..FRAME_BYTES];
        assert!(
            params[..128].iter().all(|&b| b == 0)
                && params[128..] == [0xff, 0x01, 0xee, 0x01].repeat(4),
            "the parameter block changed shape — re-derive it before relying on it"
        );
    }

    /// THE ENGINE CONNECTION: a frame in, an RF row out, with the edges CAT supplied.
    #[test]
    fn a_frame_reaches_the_spectrum_feed_as_an_absolute_rf_row() {
        let feed = tempo_app::engine::SpectrumFeed::default();
        let mut src = MockWaterfall::ramp();
        let meta = SweepMeta {
            dial_hz: 14_100_000.0,
            span_code: b'7', // 200 kHz — what the radio reported
            mode_code: b'4', // W/F CENTER (NORMAL) — likewise
        };
        assert_eq!(pump(&mut src, &feed, meta), Pumped::Published);

        let row = feed.row().expect("a row reached the feed");
        assert_eq!(row.row.len(), WF1_BINS);
        assert_eq!(row.lo_hz, 14_000_000.0);
        assert_eq!(row.hi_hz, 14_200_000.0);
        assert_eq!(
            row.source, SOURCE,
            "the label the UI's isRfScopeSource must recognise"
        );
    }

    /// The two failures are NOT interchangeable, and the distinction is what the caller acts on:
    /// a transient read fault keeps the last picture, an unplaceable row clears it.
    #[test]
    fn a_transient_fault_is_dropped_but_an_unplaceable_row_is_unavailable() {
        let feed = tempo_app::engine::SpectrumFeed::default();
        let meta = SweepMeta {
            dial_hz: 14_100_000.0,
            span_code: b'7',
            mode_code: b'4',
        };

        // A source with nothing to give = transient.
        let mut empty = MockWaterfall::new(vec![]);
        assert_eq!(pump(&mut empty, &feed, meta), Pumped::Dropped);
        // A short frame is the same class.
        let mut short = MockWaterfall::new(vec![vec![0u8; 10]]);
        assert_eq!(pump(&mut short, &feed, meta), Pumped::Dropped);

        // A CURSOR/FIX scope mode cannot be placed at all — and the read must not even be
        // attempted, since the answer would be discarded.
        let mut src = MockWaterfall::ramp();
        let cursor = SweepMeta {
            mode_code: b'7',
            ..meta
        };
        assert_eq!(pump(&mut src, &feed, cursor), Pumped::Unavailable);
    }

    /// A short read must be DROPPED, not padded. Half a line rendered as a whole one shows
    /// silence where the band is, which reads as a dead radio rather than a lost frame.
    #[test]
    fn a_truncated_frame_is_refused_rather_than_padded() {
        assert!(parse_wf1(&vec![0u8; WF1_BINS - 1]).is_none());
        // Exactly enough for the line is fine even if the rest of the frame is missing.
        assert!(parse_wf1(&vec![0u8; WF1_BINS]).is_some());
    }

    /// The span table is Yaesu's, and every code the manual defines must resolve — a missing
    /// entry would silently mis-scale the whole row.
    #[test]
    fn every_documented_span_code_resolves_and_others_do_not() {
        let expect = [
            (b'0', 1_000.0),
            (b'1', 2_000.0),
            (b'2', 5_000.0),
            (b'3', 10_000.0),
            (b'4', 20_000.0),
            (b'5', 50_000.0),
            (b'6', 100_000.0),
            (b'7', 200_000.0),
            (b'8', 500_000.0),
            (b'9', 1_000_000.0),
        ];
        for (code, hz) in expect {
            assert_eq!(span_hz(code), Some(hz), "code {}", code as char);
        }
        // The radio answered `SS0570000;` on 2026-08-17 — code 7, 200 kHz.
        assert_eq!(span_hz(b'7'), Some(200_000.0));
        for bad in [b'A', b'x', 0u8] {
            assert_eq!(span_hz(bad), None, "undefined codes must not be guessed");
        }
    }

    /// CENTER modes are the only ones whose edges follow from the dial. CURSOR and FIX are pinned
    /// to something CAT does not report, so they must yield no row rather than a wrong one.
    #[test]
    fn only_centered_modes_get_edges() {
        // W/F CENTER (NORMAL) = 4 — what the radio reported (`SS0640000;`).
        assert_eq!(
            sweep_edges(14_100_000.0, b'7', b'4'),
            Some((14_000_000.0, 14_200_000.0))
        );
        // 3DSS CENTER (0) centres on the dial too.
        assert_eq!(
            sweep_edges(14_100_000.0, b'7', b'0'),
            Some((14_000_000.0, 14_200_000.0))
        );
        for &cursor_or_fix in b"12679A" {
            assert_eq!(
                sweep_edges(14_100_000.0, b'7', cursor_or_fix),
                None,
                "mode {} is not centred on the dial",
                cursor_or_fix as char
            );
        }
        // An undefined span is refused even in a centred mode.
        assert_eq!(sweep_edges(14_100_000.0, b'Z', b'4'), None);
        // A span wider than the dial would put the row below 0 Hz.
        assert_eq!(sweep_edges(100_000.0, b'9', b'4'), None);
    }
}
