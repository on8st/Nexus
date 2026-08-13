//! Which rig does this sound card belong to? Answered from USB topology, on macOS.
//!
//! # The problem
//!
//! Two rig interfaces can report the byte-identical CoreAudio device name. An ON8ST station
//! (2026-08-13) runs an FT-710 and an FTX-1 whose codecs are both C-Media, both called
//! `USB Audio Device`, differing only in channel count. [`crate::audiodev::disambiguate_names`]
//! keeps them selectable by appending `" #2"`, but that ordinal is positional and says nothing
//! about WHICH radio it is — the operator picked between two identical strings by trial and
//! error, and a wrong guess sends TX audio to the other rig.
//!
//! Nothing in the devices themselves helps: a C-Media codec carries no serial number, and its
//! name is fixed in the chip. The only fact that distinguishes them is WHERE THEY ARE PLUGGED IN.
//!
//! # The mechanism
//!
//! macOS gives every USB device a `locationID`: a hex port path whose leading byte is the
//! controller and whose following nibbles are the port used at each hub tier, zero-padded right.
//! A rig that presents CAT and audio over one cable is internally a hub, so its interfaces are
//! SIBLINGS — their paths differ only in the final nibble:
//!
//! ```text
//!   FT-710 CAT   (CP2105)  0x111000  ┐ parent 0x110000
//!   FT-710 audio (C-Media) 0x112000  ┘
//!   FTX-1  CAT   (CP2105)  0x121000  ┐ parent 0x120000
//!   FTX-1  audio (C-Media) 0x122000  ┘
//! ```
//!
//! So: take the codec's location, take the location of the CAT port each radio profile is
//! configured on, reduce both to their parent hub ([`parent_hub`]), and a match names the radio.
//! Both numbers are obtainable:
//!
//! * **Audio** — free, no IOKit. CoreAudio's `kAudioDevicePropertyDeviceUID` for a USB device
//!   embeds it: `AppleUSBAudioEngine:C-Media Electronics Inc.:USB Audio Device:112000:2,1`.
//! * **Serial** — one IOKit call. `IORegistryEntrySearchCFProperty` with
//!   `kIORegistryIterateParents` walks up from the `IOSerialBSDClient` node to the USB device
//!   that owns it, because the tty lives in the IOService plane, not the IOUSB one.
//!
//! # What this is not
//!
//! The nibble-per-tier encoding is a long-standing Apple convention, not a documented contract,
//! so this is a well-founded heuristic rather than a guarantee — every function here fails soft,
//! returning `None`/empty so the picker simply falls back to today's labels. And a `locationID`
//! describes a PHYSICAL PORT: replug a rig into a different socket and it changes. That is the
//! right behaviour (the label follows the wiring) but it means the mapping must be recomputed at
//! every enumeration and MUST NEVER be persisted — settings continue to store the device name.

/// The parent hub of a USB port path: this path with its last non-zero nibble cleared.
///
/// Sibling interfaces of one composite device (a rig's CAT bridge and its codec) differ only in
/// that nibble, so equal parents means "same physical device". Returns `loc` unchanged when there
/// is no non-zero nibble — a root-port device has no hub to share, and comparing it to itself is
/// the correct degenerate answer.
pub fn parent_hub(loc: u32) -> u32 {
    for shift in (0..32).step_by(4) {
        if (loc >> shift) & 0xf != 0 {
            return loc & !(0xf << shift);
        }
    }
    loc
}

/// Pull the `locationID` out of a CoreAudio device UID, if it carries one.
///
/// USB audio UIDs look like
/// `AppleUSBAudioEngine:C-Media Electronics Inc.:USB Audio Device:112000:2,1` — the fourth
/// colon-separated field is the location in hex. Built-in and virtual devices (`BuiltInSpeaker`,
/// aggregate devices, Teams/OBS virtual cards) have no such field and yield `None`, which is
/// exactly right: they belong to no rig.
///
/// The manufacturer field is vendor text and may itself contain colons, so this reads the
/// location as "the last field that parses as hex and is followed by the channel-layout field"
/// rather than by a fixed index: split on ':', then take the LAST-BUT-ONE component.
pub fn location_from_audio_uid(uid: &str) -> Option<u32> {
    let parts: Vec<&str> = uid.split(':').collect();
    // `<...>:<location>:<channel layout>` — need at least a name, a location and a layout.
    if parts.len() < 3 {
        return None;
    }
    let candidate = parts[parts.len() - 2];
    // A bare decimal like "2" would parse as hex too, so require the shape of a port path:
    // non-empty, all hex digits, and more than one digit (real paths are 0x11000-scale).
    if candidate.len() < 2 || !candidate.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(candidate, 16).ok()
}

/// Name the radio that owns each audio device, by matching USB parent hubs.
///
/// `devices` are the picker's entries (their `name` is the disambiguated identity stored in
/// settings); `device_locs` maps that same name to the device's USB location; `rigs` pairs each
/// radio profile's name with the location of the CAT port it is configured on.
///
/// Pure — no IOKit, no CoreAudio — so the matching itself is unit-testable on every platform and
/// the FFI above only has to be right about two numbers.
///
/// Labels are only ever ADDED to. A device whose location is unknown, or whose hub matches no
/// configured radio, keeps the label it already had: an unplugged rig, a codec on a plain USB
/// port, and a Linux/Windows build all degrade to today's behaviour rather than to a wrong name.
/// A hub matching MORE than one radio is left alone too — that means the topology cannot
/// distinguish them, and silence beats a coin-flip when the cost is TX into the wrong radio.
pub fn label_by_rig(
    devices: &mut [crate::audiodev::AudioDevice],
    device_locs: &std::collections::HashMap<String, u32>,
    rigs: &[(String, u32)],
) {
    for d in devices.iter_mut() {
        let Some(hub) = device_locs.get(&d.name).map(|l| parent_hub(*l)) else {
            continue;
        };
        let mut owners = rigs.iter().filter(|(_, loc)| parent_hub(*loc) == hub);
        let Some((rig, _)) = owners.next() else {
            continue;
        };
        if owners.next().is_some() {
            continue; // ambiguous — two radios on one hub cannot be told apart this way
        }
        d.label = format!("{} — {rig}", d.label);
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use core_foundation_sys::base::{kCFAllocatorDefault, CFRelease, CFTypeRef};
    use core_foundation_sys::number::{kCFNumberSInt32Type, CFNumberGetValue, CFNumberRef};
    use core_foundation_sys::string::{
        kCFStringEncodingUTF8, CFStringCreateWithCString, CFStringGetCString, CFStringRef,
    };
    use coreaudio_sys::*;
    use io_kit_sys::keys::kIOServicePlane;
    use io_kit_sys::types::{io_iterator_t, io_object_t};
    use io_kit_sys::*;
    use std::collections::HashMap;
    use std::ffi::{CStr, CString};
    use std::ptr;

    /// A `CFStringRef` for `s`. Caller releases. `None` only if `s` contains a NUL, which no
    /// caller here can produce (all keys are literals).
    unsafe fn cfstr(s: &str) -> Option<CFStringRef> {
        let c = CString::new(s).ok()?;
        let r = CFStringCreateWithCString(kCFAllocatorDefault, c.as_ptr(), kCFStringEncodingUTF8);
        if r.is_null() {
            None
        } else {
            Some(r)
        }
    }

    /// Read a `CFStringRef` out into an owned `String`. 512 bytes is generous for the two keys
    /// this module reads (a device UID and a `/dev` path); a longer value is skipped rather than
    /// truncated, because a half-parsed UID would yield a wrong location, not a missing one.
    unsafe fn cfstring_to_string(v: CFStringRef) -> Option<String> {
        let mut buf = [0i8; 512];
        if CFStringGetCString(
            v,
            buf.as_mut_ptr(),
            buf.len() as isize,
            kCFStringEncodingUTF8,
        ) == 0
        {
            return None;
        }
        Some(CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    }

    /// `IORegistryEntryCreateCFProperty` for a string-valued key on one registry entry.
    unsafe fn registry_string(entry: io_object_t, key: &str) -> Option<String> {
        let k = cfstr(key)?;
        let v = IORegistryEntryCreateCFProperty(entry, k, kCFAllocatorDefault, 0);
        CFRelease(k as CFTypeRef);
        if v.is_null() {
            return None;
        }
        let out = cfstring_to_string(v as CFStringRef);
        CFRelease(v);
        out
    }

    /// The `locationID` of the nearest USB ancestor of `entry`.
    ///
    /// One call rather than a hand-rolled parent loop: `IORegistryEntrySearchCFProperty` with
    /// `kIORegistryIterateParents | kIORegistryIterateRecursively` climbs the IOService plane
    /// itself. The walk is necessary because a serial port's `IOSerialBSDClient` node carries the
    /// tty name but no location — the location belongs to the USB device several levels up.
    unsafe fn ancestor_location(entry: io_object_t) -> Option<u32> {
        let k = cfstr("locationID")?;
        let v = IORegistryEntrySearchCFProperty(
            entry,
            kIOServicePlane as *const std::os::raw::c_char,
            k,
            kCFAllocatorDefault,
            kIORegistryIterateRecursively | kIORegistryIterateParents,
        );
        CFRelease(k as CFTypeRef);
        if v.is_null() {
            return None;
        }
        let mut out: i32 = 0;
        let ok = CFNumberGetValue(
            v as CFNumberRef,
            kCFNumberSInt32Type,
            &mut out as *mut _ as *mut _,
        );
        CFRelease(v);
        if ok {
            Some(out as u32)
        } else {
            None
        }
    }

    /// Every USB serial port's `/dev/cu.*` callout path mapped to its USB location.
    ///
    /// Both driver nodes of a dual-claimed port appear (a CP2105 with the Silicon Labs extension
    /// installed alongside Apple's own driver yields `usbserial-*` AND `SLAB_USBtoUART*`); they
    /// report the SAME location, since they are two names for one physical interface, so either
    /// spelling in a radio profile resolves identically.
    pub fn serial_locations() -> HashMap<String, u32> {
        let mut out = HashMap::new();
        unsafe {
            let matching = IOServiceMatching(c"IOSerialBSDClient".as_ptr());
            if matching.is_null() {
                return out;
            }
            let mut it: io_iterator_t = 0;
            // Consumes `matching` whether it succeeds or fails — no leak on the error path.
            if IOServiceGetMatchingServices(kIOMasterPortDefault, matching, &mut it) != 0 {
                return out;
            }
            loop {
                let entry = IOIteratorNext(it);
                if entry == 0 {
                    break;
                }
                if let Some(tty) = registry_string(entry, "IOCalloutDevice") {
                    if let Some(loc) = ancestor_location(entry) {
                        out.insert(tty, loc);
                    }
                }
                IOObjectRelease(entry);
            }
            IOObjectRelease(it);
        }
        out
    }

    unsafe fn global_addr(selector: u32) -> AudioObjectPropertyAddress {
        AudioObjectPropertyAddress {
            mSelector: selector,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        }
    }

    unsafe fn device_string(dev: AudioObjectID, selector: u32) -> Option<String> {
        let addr = global_addr(selector);
        let mut s: CFStringRef = ptr::null();
        let mut size = std::mem::size_of::<CFStringRef>() as u32;
        if AudioObjectGetPropertyData(
            dev,
            &addr,
            0,
            ptr::null(),
            &mut size,
            &mut s as *mut _ as *mut _,
        ) != 0
            || s.is_null()
        {
            return None;
        }
        let out = cfstring_to_string(s);
        CFRelease(s as CFTypeRef);
        out
    }

    /// Channel count in one scope — the direction filter. A device with zero input channels is
    /// not in cpal's `input_devices()` list, so it must not consume an ordinal here either.
    unsafe fn channels(dev: AudioObjectID, scope: u32) -> u32 {
        let addr = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyStreamConfiguration,
            mScope: scope,
            mElement: kAudioObjectPropertyElementMaster,
        };
        let mut size = 0u32;
        if AudioObjectGetPropertyDataSize(dev, &addr, 0, ptr::null(), &mut size) != 0 || size == 0 {
            return 0;
        }
        let mut buf = vec![0u8; size as usize];
        if AudioObjectGetPropertyData(
            dev,
            &addr,
            0,
            ptr::null(),
            &mut size,
            buf.as_mut_ptr() as *mut _,
        ) != 0
        {
            return 0;
        }
        let list = &*(buf.as_ptr() as *const AudioBufferList);
        std::slice::from_raw_parts(list.mBuffers.as_ptr(), list.mNumberBuffers as usize)
            .iter()
            .map(|b| b.mNumberChannels)
            .sum()
    }

    /// Device locations keyed by the SAME disambiguated name the picker stores.
    ///
    /// Alignment with the picker is by name-plus-ordinal, not by raw index: this walks
    /// CoreAudio's `kAudioHardwarePropertyDevices` in order, filters to the requested direction,
    /// and applies [`crate::audiodev::disambiguate_names`] — the identical transformation
    /// `available_devices` applies to cpal's list. It lines up because cpal's macOS enumerator
    /// reads that same property in that same order, so both sides see one ordering; and because
    /// the key is the full disambiguated string, a mismatch degrades to a missing entry (no
    /// label) rather than to a wrong rig name.
    pub fn audio_locations(input: bool) -> HashMap<String, u32> {
        let mut names = Vec::new();
        let mut locs = Vec::new();
        unsafe {
            let addr = global_addr(kAudioHardwarePropertyDevices);
            let mut size = 0u32;
            if AudioObjectGetPropertyDataSize(
                kAudioObjectSystemObject,
                &addr,
                0,
                ptr::null(),
                &mut size,
            ) != 0
            {
                return HashMap::new();
            }
            let count = size as usize / std::mem::size_of::<AudioObjectID>();
            let mut ids = vec![0 as AudioObjectID; count];
            if AudioObjectGetPropertyData(
                kAudioObjectSystemObject,
                &addr,
                0,
                ptr::null(),
                &mut size,
                ids.as_mut_ptr() as *mut _,
            ) != 0
            {
                return HashMap::new();
            }
            let scope = if input {
                kAudioObjectPropertyScopeInput
            } else {
                kAudioObjectPropertyScopeOutput
            };
            for id in ids {
                if channels(id, scope) == 0 {
                    continue;
                }
                let Some(name) = device_string(id, kAudioObjectPropertyName) else {
                    continue;
                };
                let loc = device_string(id, kAudioDevicePropertyDeviceUID)
                    .as_deref()
                    .and_then(location_from_audio_uid);
                names.push(name);
                locs.push(loc);
            }
        }
        crate::audiodev::disambiguate_names(names)
            .into_iter()
            .zip(locs)
            .filter_map(|(n, l)| l.map(|l| (n, l)))
            .collect()
    }
}

#[cfg(target_os = "macos")]
pub use imp::{audio_locations, serial_locations};

/// Non-macOS: no USB topology source, so nothing is known and every label is left alone.
///
/// Windows could do this through SetupAPI device instance paths and Linux through sysfs, but
/// neither is written; the picker there keeps the `" #2"` ordinal it has always had.
#[cfg(not(target_os = "macos"))]
pub fn serial_locations() -> std::collections::HashMap<String, u32> {
    std::collections::HashMap::new()
}

/// Non-macOS counterpart of [`audio_locations`] — always empty. See [`serial_locations`].
#[cfg(not(target_os = "macos"))]
pub fn audio_locations(_input: bool) -> std::collections::HashMap<String, u32> {
    std::collections::HashMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audiodev::AudioDevice;
    use std::collections::HashMap;

    #[test]
    fn parent_hub_clears_only_the_last_tier() {
        // The real ON8ST paths: each rig's CAT bridge and codec are siblings.
        assert_eq!(parent_hub(0x111000), 0x110000, "FT-710 CAT");
        assert_eq!(parent_hub(0x112000), 0x110000, "FT-710 audio");
        assert_eq!(parent_hub(0x121000), 0x120000, "FTX-1 CAT");
        assert_eq!(parent_hub(0x122000), 0x120000, "FTX-1 audio");
        // The two rigs must NOT collapse together — that would label both codecs for one radio.
        assert_ne!(parent_hub(0x112000), parent_hub(0x122000));
        // Deeper nesting still only loses its last tier.
        assert_eq!(parent_hub(0x14321000), 0x14320000);
        // A root-port device has no hub; comparing it to itself is the right degenerate answer.
        assert_eq!(parent_hub(0), 0);
    }

    #[test]
    fn location_is_read_out_of_a_real_coreaudio_uid() {
        assert_eq!(
            location_from_audio_uid(
                "AppleUSBAudioEngine:C-Media Electronics Inc.:USB Audio Device:112000:2,1"
            ),
            Some(0x112000)
        );
        assert_eq!(
            location_from_audio_uid(
                "AppleUSBAudioEngine:C-Media Electronics Inc.:USB Audio Device:122000:2,1"
            ),
            Some(0x122000)
        );
        // Built-in, aggregate and virtual devices carry no port path and belong to no rig.
        assert_eq!(location_from_audio_uid("BuiltInSpeakerDevice"), None);
        assert_eq!(location_from_audio_uid("AppleAggregateDevice:0"), None);
        // A single digit is a channel-layout field, not a port path — never accept it as one.
        assert_eq!(location_from_audio_uid("Some:Device:2"), None);
    }

    fn dev(name: &str) -> AudioDevice {
        AudioDevice {
            name: name.to_string(),
            label: name.to_string(),
        }
    }

    #[test]
    fn identical_codecs_are_labelled_with_the_rig_they_are_plugged_into() {
        // The case this module exists for: two C-Media dongles, byte-identical names, told apart
        // only by which rig's hub they hang off.
        let mut devices = vec![dev("USB Audio Device"), dev("USB Audio Device #2")];
        let locs = HashMap::from([
            ("USB Audio Device".to_string(), 0x112000),
            ("USB Audio Device #2".to_string(), 0x122000),
        ]);
        let rigs = vec![
            ("FT710".to_string(), 0x111000),
            ("FTX-1".to_string(), 0x121000),
        ];
        label_by_rig(&mut devices, &locs, &rigs);
        assert_eq!(devices[0].label, "USB Audio Device — FT710");
        assert_eq!(devices[1].label, "USB Audio Device #2 — FTX-1");
        // The stored identity is untouched — settings keep resolving exactly as before.
        assert_eq!(devices[0].name, "USB Audio Device");
        assert_eq!(devices[1].name, "USB Audio Device #2");
    }

    #[test]
    fn anything_unproven_keeps_the_label_it_had() {
        let mut devices = vec![
            dev("USB Audio Device"), // location known, but on no configured rig's hub
            dev("Built-in Output"),  // no location at all
            dev("Shared Codec"),     // hub carries TWO radios — cannot be told apart
        ];
        let locs = HashMap::from([
            ("USB Audio Device".to_string(), 0x992000),
            ("Shared Codec".to_string(), 0x332000),
        ]);
        let rigs = vec![
            ("FT710".to_string(), 0x111000),
            ("A".to_string(), 0x331000),
            ("B".to_string(), 0x333000),
        ];
        label_by_rig(&mut devices, &locs, &rigs);
        assert_eq!(
            devices[0].label, "USB Audio Device",
            "a codec on no configured rig's hub must not be named"
        );
        assert_eq!(
            devices[1].label, "Built-in Output",
            "a device with no USB path belongs to no rig"
        );
        assert_eq!(
            devices[2].label, "Shared Codec",
            "two radios on one hub is ambiguous — silence beats guessing when the cost of being \
             wrong is TX audio into the other radio"
        );
    }
}
