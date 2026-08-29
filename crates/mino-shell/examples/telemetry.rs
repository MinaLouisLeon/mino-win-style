//! Prints what the JARVIS HUD would show, without drawing anything.
//!
//! Two samples a second apart, because the processor and network figures are
//! rates: the first read has nothing to compare against and reports zero.
//!
//! `cargo run -p mino-shell --example telemetry`

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only.");
}

#[cfg(windows)]
fn main() {
    let sampler = mino_shell::Sampler::new();
    sampler.read(); // primes the counters

    for _ in 0..5 {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let t = sampler.read();
        println!(
            "cpu {:5.1}%   mem {:5.1}% ({:.1}/{:.1} GB)   disk {:5.1}% ({:.0}/{:.0} GB)   \
             net down {:8.1} KB/s up {:8.1} KB/s   up {}h{:02}m   battery {}",
            t.cpu_percent,
            percent(t.memory_used_bytes, t.memory_total_bytes),
            gb(t.memory_used_bytes),
            gb(t.memory_total_bytes),
            percent(t.disk_used_bytes, t.disk_total_bytes),
            gb(t.disk_used_bytes),
            gb(t.disk_total_bytes),
            t.net_down_bps / 1024.0,
            t.net_up_bps / 1024.0,
            t.uptime_seconds / 3600,
            (t.uptime_seconds % 3600) / 60,
            match t.battery {
                Some(b) => format!("{}%{}", b.percent, if b.charging { " charging" } else { "" }),
                None => "none".into(),
            }
        );
    }
}

#[cfg(windows)]
fn gb(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0 / 1024.0
}

#[cfg(windows)]
fn percent(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64 * 100.0
    }
}
