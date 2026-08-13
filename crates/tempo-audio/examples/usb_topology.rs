//! What the Settings audio picker will show, and the USB topology it derives that from.
//!
//! Diagnostic for a station where two rig codecs report the same name: prints every codec's USB
//! port path, every serial port's, and the label each device ends up with once matched to a radio
//! profile. Run it when a picker entry names the wrong rig, or names none.
//!
//!   cargo run -p tempo-audio --features device,serial --example usb_topology
use tempo_audio::usbtopo::{audio_locations, label_by_rig, parent_hub, serial_locations};

fn main() {
    let serial = serial_locations();

    println!("--- serial ports (USB) ---");
    let mut ports: Vec<_> = serial
        .iter()
        .filter(|(t, _)| t.contains("usbserial") || t.contains("SLAB") || t.contains("tty.usb"))
        .collect();
    ports.sort();
    for (tty, loc) in ports {
        println!(
            "  {tty:<32} loc 0x{loc:06x}  hub 0x{:06x}",
            parent_hub(*loc)
        );
    }

    // The radios as configured, paired with the hub their CAT port sits on — the same list the
    // `get_audio_devices` command builds.
    // Same config location the app uses: XDG_CONFIG_HOME, else ~/.config (macOS included —
    // Nexus follows the Linux convention here, see MACOS.md §4).
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .unwrap_or_default()
        .join("tempo/settings.json");
    let settings = tempo_app::settings::Settings::load(&config);
    let rigs: Vec<(String, u32)> = settings
        .radios
        .iter()
        .filter_map(|r| {
            serial
                .get(r.serial_port.trim())
                .map(|loc| (r.name.clone(), *loc))
        })
        .collect();
    println!("--- radios with a resolvable CAT port ---");
    if rigs.is_empty() {
        println!("  (none — every profile's port is absent, or they are VOX-only)");
    }
    for (name, loc) in &rigs {
        println!("  {name:<32} hub 0x{:06x}", parent_hub(*loc));
    }

    for (dir, input) in [("input", true), ("output", false)] {
        println!("--- what the {dir} picker shows ---");
        let (mut ins, mut outs) = tempo_audio::device::available_devices();
        let devices = if input { &mut ins } else { &mut outs };
        label_by_rig(devices, &audio_locations(input), &rigs);
        for d in devices.iter() {
            let note = if d.label == d.name {
                ""
            } else {
                "   <= named by topology"
            };
            println!("  {}{note}", d.label);
        }
    }
}
