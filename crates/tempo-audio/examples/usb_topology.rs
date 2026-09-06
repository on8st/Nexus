//! The USB topology the Settings pickers derive their advice from, printed raw.
//!
//! Diagnostic for a station where two rig codecs report the same name: prints every serial port's
//! USB port path, every codec's, and — for each configured radio — which sound card shares that
//! radio's parent hub. Run it when the rig form's "not inside the radio on …" warning fires and
//! you want to see the numbers it fired on, or when it stays silent and you expected it to speak.
//!
//!   cargo run -p tempo-audio --features device,serial --example usb_topology
//!
//! ⚠️ Nothing here RENAMES anything, because nothing in the app does: `usbtopo` hands the pickers
//! structured facts and they keep showing the strings they always showed. What this prints is
//! exactly those facts.
use tempo_audio::usbtopo::{audio_locations, parent_hub, serial_interfaces, serial_locations};

fn main() {
    let serial = serial_locations();
    let ifaces = serial_interfaces();

    println!("--- serial ports (USB) ---");
    let mut ports: Vec<_> = serial
        .iter()
        .filter(|(t, _)| t.contains("usbserial") || t.contains("SLAB") || t.contains("tty.usb"))
        .collect();
    ports.sort();
    for (tty, loc) in ports {
        let iface = ifaces
            .get(tty)
            .map(|i| i.to_string())
            .unwrap_or_else(|| "?".into());
        println!(
            "  {tty:<32} loc 0x{loc:06x}  hub 0x{:06x}  interface {iface}",
            parent_hub(*loc)
        );
    }

    for (dir, input) in [("input", true), ("output", false)] {
        println!("--- {dir} codecs ---");
        let locs = audio_locations(input);
        let mut cards: Vec<_> = locs.iter().collect();
        cards.sort();
        if cards.is_empty() {
            println!("  (none carries a USB port path — built-in, aggregate or virtual only)");
        }
        for (name, loc) in cards {
            println!(
                "  {name:<32} loc 0x{loc:06x}  hub 0x{:06x}",
                parent_hub(*loc)
            );
        }
    }

    // The radios as configured, paired with the hub their CAT port sits on — the same relation
    // `get_serial_ports_detailed` reports as `paired_audio`, and the one `checkRigForm` warns on.
    // Same config location the app uses: XDG_CONFIG_HOME, else ~/.config (macOS included —
    // Nexus follows the Linux convention here, see MACOS.md §4).
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .unwrap_or_default()
        .join("tempo/settings.json");
    let settings = tempo_app::settings::Settings::load(&config);
    println!("--- radios with a resolvable CAT port ---");
    let mut any = false;
    for r in &settings.radios {
        let Some(loc) = serial.get(r.serial_port.trim()) else {
            continue;
        };
        any = true;
        let hub = parent_hub(*loc);
        println!("  {:<32} hub 0x{hub:06x}", r.name);
        for (dir, input) in [("in ", true), ("out", false)] {
            let mut mates: Vec<String> = audio_locations(input)
                .into_iter()
                .filter(|(_, al)| parent_hub(*al) == hub)
                .map(|(n, _)| n)
                .collect();
            mates.sort();
            if mates.is_empty() {
                println!("      {dir}  (no codec shares this hub)");
            }
            for m in mates {
                println!("      {dir}  {m}");
            }
        }
    }
    if !any {
        println!("  (none — every profile's port is absent, or they are VOX-only)");
    }
}
