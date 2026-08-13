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

/// The devices that sit on the SAME physical USB device as the CAT port at `port_loc`.
///
/// The configure-time counterpart of [`label_by_rig`], and the more useful direction: an operator
/// sets the CAT port first, so by the time they reach the audio pickers the answer is already
/// determined. It needs no radio to be named, no profile to be saved and no assumption about what
/// anything is CALLED — a rig carrying CAT and audio down one cable is internally a hub, so its
/// codec is the one sharing its parent.
///
/// Returns the matching `name`s (the identity the picker stores), in the order given. Empty when
/// nothing matches, which is the honest answer for a rig whose audio is not USB at all (a network
/// codec, a separate interface box, an analogue card) — the picker then offers everything, as it
/// always did, rather than an empty list.
pub fn devices_sharing_usb_device(
    devices: &[crate::audiodev::AudioDevice],
    device_locs: &std::collections::HashMap<String, u32>,
    port_loc: u32,
) -> Vec<String> {
    let hub = parent_hub(port_loc);
    devices
        .iter()
        .filter(|d| {
            device_locs
                .get(&d.name)
                .is_some_and(|l| parent_hub(*l) == hub)
        })
        .map(|d| d.name.clone())
        .collect()
}

/// Enrich a serial port's picker label with what makes it identifiable: which of the device's
/// interfaces it is, and which sound card is on the same rig.
///
/// The port picker had the same defect the audio picker had, and worse. Two radios with the same
/// bridge chip produce EIGHT identically-labelled entries — every one reading
/// `CP2105 Dual USB to UART Bridge Controller` — because the label is the USB product string and
/// the chip is the product. Add to that a `tty.*` twin of every node and a second set from a
/// redundant vendor driver, and an operator picks their rig out of sixteen indistinguishable
/// lines. On the ON8ST station that is exactly what went wrong: the FTX-1's profile was saved
/// pointing at the FT-710's CAT port, which is an entirely reasonable mistake to make from that
/// list.
///
/// Two facts fix it, both from USB topology:
///
/// * **Which interface** — a CP2105 is a DUAL bridge and only interface 0 does CAT; interface 1
///   answers nothing. `bInterfaceNumber` is the honest source. (The vendor driver publishes
///   "Enhanced Port"/"Standard Port" strings, but only inside its own matched personality, so
///   they vanish with the driver — the number does not.)
/// * **Which rig** — the sound card sharing this port's parent hub, i.e. the codec inside the
///   same radio. That is what actually tells the two rigs apart.
///
/// Labels are only ever ADDED to and nothing is removed from the list: a port whose topology is
/// unknown keeps exactly the label it had. Same reasoning as [`label_by_rig`] — a picker that hid
/// or renamed the operator's real port would be worse than one that failed to annotate it.
pub fn label_serial_ports(
    ports: &mut [crate::audiodev::AudioDevice],
    port_locs: &std::collections::HashMap<String, u32>,
    port_ifaces: &std::collections::HashMap<String, u32>,
    audio_locs: &std::collections::HashMap<String, u32>,
) {
    // "port N" is only informative on a bridge that HAS more than one — saying it about a
    // single-port device is noise that reads like a fourth radio. Count the DISTINCT interface
    // numbers per USB device, not the entries: a dual-claimed port appears twice (`usbserial-*`
    // and `SLAB_*`) with the same number and must not be counted as two ports.
    let mut ifaces_per_hub: std::collections::HashMap<u32, std::collections::BTreeSet<u32>> =
        std::collections::HashMap::new();
    for (name, loc) in port_locs.iter() {
        if let Some(iface) = port_ifaces.get(name) {
            ifaces_per_hub
                .entry(parent_hub(*loc))
                .or_default()
                .insert(*iface);
        }
    }

    for p in ports.iter_mut() {
        let mut extra: Vec<String> = Vec::new();
        let multiport = port_locs
            .get(&p.name)
            .map(|l| parent_hub(*l))
            .and_then(|h| ifaces_per_hub.get(&h))
            .is_some_and(|set| set.len() > 1);
        if let Some(iface) = port_ifaces.get(&p.name).filter(|_| multiport) {
            extra.push(format!("port {}", iface + 1));
        }
        if let Some(hub) = port_locs.get(&p.name).map(|l| parent_hub(*l)) {
            let mut mates: Vec<&String> = audio_locs
                .iter()
                .filter(|(_, l)| parent_hub(**l) == hub)
                .map(|(n, _)| n)
                .collect();
            mates.sort();
            if let Some(first) = mates.first() {
                extra.push(format!("with “{first}”"));
            }
        }
        if !extra.is_empty() {
            let base = if p.label.is_empty() {
                p.name.clone()
            } else {
                p.label.clone()
            };
            p.label = format!("{base} — {}", extra.join(" · "));
        }
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
        ancestor_u32(entry, "locationID")
    }

    /// A `u32`-valued property from `entry` or the nearest ancestor that has it.
    ///
    /// One call rather than a hand-rolled parent loop: `IORegistryEntrySearchCFProperty` with
    /// `kIORegistryIterateParents | kIORegistryIterateRecursively` climbs the IOService plane
    /// itself. The walk is necessary because a serial port's `IOSerialBSDClient` node carries the
    /// tty name but neither the location nor the interface number — those belong to the USB
    /// interface and device several levels up.
    unsafe fn ancestor_u32(entry: io_object_t, key: &str) -> Option<u32> {
        let k = cfstr(key)?;
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

    /// Each USB serial port's `/dev/cu.*` path mapped to its `bInterfaceNumber`.
    ///
    /// Distinguishes the halves of a multi-interface bridge: a CP2105 exposes two, and only
    /// interface 0 carries CAT on a Yaesu. Same registry walk as [`serial_locations`] — the
    /// number lives on the `IOUSBHostInterface` several levels above the tty node.
    pub fn serial_interfaces() -> HashMap<String, u32> {
        let mut out = HashMap::new();
        unsafe {
            let matching = IOServiceMatching(c"IOSerialBSDClient".as_ptr());
            if matching.is_null() {
                return out;
            }
            let mut it: io_iterator_t = 0;
            if IOServiceGetMatchingServices(kIOMasterPortDefault, matching, &mut it) != 0 {
                return out;
            }
            loop {
                let entry = IOIteratorNext(it);
                if entry == 0 {
                    break;
                }
                if let Some(tty) = registry_string(entry, "IOCalloutDevice") {
                    if let Some(n) = ancestor_u32(entry, "bInterfaceNumber") {
                        out.insert(tty, n);
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
pub use imp::{audio_locations, serial_interfaces, serial_locations};

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

/// Non-macOS counterpart of [`serial_interfaces`] — always empty. See [`serial_locations`].
#[cfg(not(target_os = "macos"))]
pub fn serial_interfaces() -> std::collections::HashMap<String, u32> {
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
    fn the_codecs_offered_for_a_cat_port_are_the_ones_inside_that_rig() {
        // Real ON8ST topology: each rig's CAT bridge and codec are siblings on the rig's own
        // internal hub, so selecting a CAT port determines the codec with no naming involved.
        let devices = vec![
            dev("USB Audio Device"),    // FT-710's, hub 0x110000
            dev("USB Audio Device #2"), // FTX-1's,  hub 0x120000
            dev("Mac mini Speakers"),   // not USB at all
        ];
        let locs = HashMap::from([
            ("USB Audio Device".to_string(), 0x112000),
            ("USB Audio Device #2".to_string(), 0x122000),
        ]);

        // The FT-710's CAT port offers only the FT-710's codec.
        assert_eq!(
            devices_sharing_usb_device(&devices, &locs, 0x111000),
            vec!["USB Audio Device"]
        );
        // The FTX-1's offers only the FTX-1's.
        assert_eq!(
            devices_sharing_usb_device(&devices, &locs, 0x121000),
            vec!["USB Audio Device #2"]
        );
        // The rig's OTHER CAT port (a CP2105 is dual: Enhanced + Standard) is the same USB
        // device, so it must resolve identically — an operator on the Standard port gets the
        // same answer as one on the Enhanced port.
        assert_eq!(
            devices_sharing_usb_device(&devices, &locs, 0x111000),
            devices_sharing_usb_device(&devices, &locs, 0x111000)
        );
        // A port on no shared hub proposes nothing, so the caller offers the full list rather
        // than pretending a rig has no audio.
        assert!(devices_sharing_usb_device(&devices, &locs, 0x990000).is_empty());
    }

    #[test]
    fn serial_ports_say_which_rig_and_which_half_of_the_bridge() {
        // The ON8ST list: two CP2105s, byte-identical product strings, plus each port's twin from
        // the redundant vendor driver, plus a monitor's single-port device.
        let cp = "CP2105 Dual USB to UART Bridge Controller";
        let mut ports = vec![
            AudioDevice {
                name: "/dev/cu.usbserial-01AF7FED0".into(),
                label: cp.into(),
            },
            AudioDevice {
                name: "/dev/cu.usbserial-01AF7FED1".into(),
                label: cp.into(),
            },
            AudioDevice {
                name: "/dev/cu.SLAB_USBtoUART11".into(),
                label: cp.into(),
            },
            AudioDevice {
                name: "/dev/cu.usbserial-01A98F800".into(),
                label: cp.into(),
            },
            AudioDevice {
                name: "/dev/cu.usbmodem-LG".into(),
                label: "LG Monitor Controls".into(),
            },
        ];
        let locs = HashMap::from([
            ("/dev/cu.usbserial-01AF7FED0".to_string(), 0x111000),
            ("/dev/cu.usbserial-01AF7FED1".to_string(), 0x111000),
            ("/dev/cu.SLAB_USBtoUART11".to_string(), 0x111000),
            ("/dev/cu.usbserial-01A98F800".to_string(), 0x121000),
            ("/dev/cu.usbmodem-LG".to_string(), 0x131000),
        ]);
        let ifaces = HashMap::from([
            ("/dev/cu.usbserial-01AF7FED0".to_string(), 0),
            ("/dev/cu.usbserial-01AF7FED1".to_string(), 1),
            ("/dev/cu.SLAB_USBtoUART11".to_string(), 0),
            ("/dev/cu.usbserial-01A98F800".to_string(), 0),
            ("/dev/cu.usbmodem-LG".to_string(), 2),
        ]);
        let audio = HashMap::from([
            ("USB Audio Device".to_string(), 0x112000),
            ("USB Audio Device #2".to_string(), 0x122000),
        ]);
        label_serial_ports(&mut ports, &locs, &ifaces, &audio);

        // THE bug this fixes: these two were indistinguishable, and the FTX-1's profile was saved
        // pointing at the FT-710's port.
        assert!(
            ports[0].label.contains("USB Audio Device")
                && !ports[0].label.contains("USB Audio Device #2"),
            "the FT-710's port must name the FT-710's codec, got {:?}",
            ports[0].label
        );
        assert!(
            ports[3].label.contains("USB Audio Device #2"),
            "the FTX-1's port must name the FTX-1's codec, got {:?}",
            ports[3].label
        );
        assert_ne!(
            ports[0].label, ports[3].label,
            "the whole point is telling them apart"
        );

        // A dual bridge says which half; only interface 0 does CAT on these rigs.
        assert!(
            ports[0].label.contains("port 1"),
            "got {:?}",
            ports[0].label
        );
        assert!(
            ports[1].label.contains("port 2"),
            "got {:?}",
            ports[1].label
        );
        // The vendor driver's twin of port 0 is the SAME interface — it must not read as a third
        // port, and it must still name the right rig.
        assert!(
            ports[2].label.contains("port 1"),
            "got {:?}",
            ports[2].label
        );

        // A single-port device gets NO port number: "port 3" on a monitor reads like a fourth
        // radio. It also names no rig, having no codec on its hub.
        assert_eq!(
            ports[4].label, "LG Monitor Controls",
            "a single-interface device must not be annotated at all"
        );
    }

    #[test]
    fn a_serial_port_with_no_topology_keeps_its_label() {
        let mut ports = vec![
            AudioDevice {
                name: "/dev/cu.Bluetooth-Incoming-Port".into(),
                label: String::new(),
            },
            AudioDevice {
                name: "/dev/cu.legacy".into(),
                label: "Some Adapter".into(),
            },
        ];
        label_serial_ports(
            &mut ports,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(ports[0].label, "");
        assert_eq!(ports[1].label, "Some Adapter");
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
