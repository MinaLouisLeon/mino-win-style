//! What the machine is doing, in numbers a HUD can show.
//!
//! The JARVIS overlay wants live readouts — processor load, memory, disk,
//! network throughput, uptime, battery. Every one of those is a documented
//! kernel call, so there is no new dependency here: `sysinfo` would have pulled
//! in a second copy of `windows-sys` to tell us what six `kernel32` functions
//! already say.
//!
//! Two of the readings are rates rather than levels — processor load and
//! network throughput are both "how much has happened since last time" — so
//! this module keeps the previous sample. `Sampler` owns that state; a single
//! call with nothing to compare against reports zero and primes itself, which
//! is why the HUD's first tick shows a flat line for a moment.

use std::sync::Mutex;

use windows::Win32::Foundation::FILETIME;
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::{
    GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows::Win32::System::Threading::GetSystemTimes;

use crate::{Battery, Telemetry};

/// Loopback interfaces move gigabytes that never touch a wire; counting them
/// would make the network readout meaningless on a machine running a local
/// server. `IF_TYPE_SOFTWARE_LOOPBACK` from `ipifcons.h`.
const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
/// `IfOperStatusUp` — an interface that is actually carrying traffic. Windows
/// reports every adapter it has ever seen, including the disconnected ones.
const IF_OPER_STATUS_UP: i32 = 1;

/// A pair of counters and the moment they were read.
#[derive(Debug, Clone, Copy)]
struct Sample {
    /// 100ns ticks the processor spent idle, across all cores.
    idle: u64,
    /// 100ns ticks the processor spent doing anything at all, idle included.
    total: u64,
    bytes_in: u64,
    bytes_out: u64,
    /// Milliseconds since boot, which is the one clock here that cannot go
    /// backwards when the user changes the time zone.
    at_ms: u64,
}

/// Holds the previous sample so rates can be worked out. One of these lives for
/// the life of the process; the HUD polls it on a timer.
pub struct Sampler {
    previous: Mutex<Option<Sample>>,
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Sampler {
    pub fn new() -> Self {
        Sampler {
            previous: Mutex::new(None),
        }
    }

    /// Reads everything once and returns it.
    ///
    /// A failure in any one reading is not a failure of the whole: a machine
    /// with no battery, or a network table that will not enumerate, still has a
    /// processor and memory worth showing. Each field falls back to zero or
    /// `None` on its own.
    pub fn read(&self) -> Telemetry {
        let (idle, total) = cpu_times();
        let (bytes_in, bytes_out) = network_octets();
        let now = Sample {
            idle,
            total,
            bytes_in,
            bytes_out,
            at_ms: unsafe { GetTickCount64() },
        };

        let (cpu_percent, net_down_bps, net_up_bps) = {
            let mut previous = self.previous.lock().unwrap_or_else(|e| e.into_inner());
            let rates = previous.map_or((0.0, 0.0, 0.0), |before| rates_between(before, now));
            *previous = Some(now);
            rates
        };

        let memory = memory();
        let disk = disk();

        Telemetry {
            cpu_percent,
            memory_used_bytes: memory.0,
            memory_total_bytes: memory.1,
            disk_used_bytes: disk.0,
            disk_total_bytes: disk.1,
            net_down_bps,
            net_up_bps,
            uptime_seconds: now.at_ms / 1000,
            battery: battery(),
        }
    }
}

/// Works out the three rates between two samples.
///
/// Counters can wrap or reset — a network adapter that is disabled and
/// re-enabled starts again from zero — so a negative delta is reported as zero
/// rather than as an enormous spike.
fn rates_between(before: Sample, now: Sample) -> (f32, f64, f64) {
    let elapsed_ms = now.at_ms.saturating_sub(before.at_ms);
    if elapsed_ms == 0 {
        return (0.0, 0.0, 0.0);
    }
    let seconds = elapsed_ms as f64 / 1000.0;

    let total = now.total.saturating_sub(before.total);
    let idle = now.idle.saturating_sub(before.idle);
    let cpu = if total == 0 {
        0.0
    } else {
        // Busy time over total time. Windows counts idle inside kernel time, so
        // the two subtract cleanly without double counting.
        ((1.0 - idle as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32
    };

    let down = now.bytes_in.saturating_sub(before.bytes_in) as f64 / seconds;
    let up = now.bytes_out.saturating_sub(before.bytes_out) as f64 / seconds;

    (cpu, down, up)
}

/// `(idle ticks, total ticks)` across every processor, in 100ns units.
fn cpu_times() -> (u64, u64) {
    unsafe {
        let mut idle = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_err() {
            return (0, 0);
        }
        // Kernel time already includes idle time, so kernel + user is the whole
        // of it and needs no third term.
        (ticks(idle), ticks(kernel) + ticks(user))
    }
}

/// A `FILETIME` is a 64-bit count delivered in two halves.
fn ticks(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

/// Total bytes in and out across every connected, non-loopback interface.
fn network_octets() -> (u64, u64) {
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table).is_err() || table.is_null() {
            return (0, 0);
        }

        let mut down = 0u64;
        let mut up = 0u64;

        // `Table` is declared as a one-element array and is in fact `NumEntries`
        // long — the usual Win32 trailing-array shape. The pointer walk is why
        // this function is unsafe, and why the length comes from the header
        // rather than from anywhere else.
        let count = (*table).NumEntries as usize;
        let rows = (*table).Table.as_ptr();
        for index in 0..count {
            let row = &*rows.add(index);
            if row.Type == IF_TYPE_SOFTWARE_LOOPBACK || row.OperStatus.0 != IF_OPER_STATUS_UP {
                continue;
            }
            down = down.saturating_add(row.InOctets);
            up = up.saturating_add(row.OutOctets);
        }

        FreeMibTable(table as *const core::ffi::c_void);
        (down, up)
    }
}

/// `(used, total)` physical memory in bytes.
fn memory() -> (u64, u64) {
    unsafe {
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        if GlobalMemoryStatusEx(&mut status).is_err() {
            return (0, 0);
        }
        (
            status.ullTotalPhys.saturating_sub(status.ullAvailPhys),
            status.ullTotalPhys,
        )
    }
}

/// `(used, total)` bytes on the volume Windows itself is installed on.
///
/// One volume, not all of them: the HUD has room for one bar, and the system
/// drive filling up is the one that stops the machine working.
fn disk() -> (u64, u64) {
    let root = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let mut path: Vec<u16> = format!("{root}\\").encode_utf16().collect();
    path.push(0);

    unsafe {
        let mut free = 0u64;
        let mut total = 0u64;
        if GetDiskFreeSpaceExW(
            windows::core::PCWSTR(path.as_ptr()),
            None,
            Some(&mut total),
            Some(&mut free),
        )
        .is_err()
        {
            return (0, 0);
        }
        (total.saturating_sub(free), total)
    }
}

/// The battery, on a machine that has one. A desktop reports `None`, and the
/// HUD simply leaves the row out rather than showing a meaningless 100%.
fn battery() -> Option<Battery> {
    unsafe {
        let mut status = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut status).is_err() {
            return None;
        }
        // 255 means "unknown", which is what a desktop with no battery says.
        if status.BatteryLifePercent > 100 {
            return None;
        }
        // BatteryFlag bit 7 is "no system battery". Belt and braces: some
        // virtual machines report a percentage anyway.
        if status.BatteryFlag & 128 != 0 {
            return None;
        }
        Some(Battery {
            percent: status.BatteryLifePercent,
            charging: status.ACLineStatus == 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filetime_is_read_as_one_64_bit_number() {
        let time = FILETIME {
            dwLowDateTime: 0x0000_0002,
            dwHighDateTime: 0x0000_0001,
        };
        assert_eq!(ticks(time), 0x0000_0001_0000_0002);
    }

    #[test]
    fn rates_need_two_samples_that_are_apart_in_time() {
        let sample = Sample {
            idle: 10,
            total: 100,
            bytes_in: 0,
            bytes_out: 0,
            at_ms: 1_000,
        };
        assert_eq!(rates_between(sample, sample), (0.0, 0.0, 0.0));
    }

    #[test]
    fn idle_time_becomes_the_load_that_is_left_over() {
        let before = Sample {
            idle: 0,
            total: 0,
            bytes_in: 0,
            bytes_out: 0,
            at_ms: 0,
        };
        let after = Sample {
            idle: 250,
            total: 1_000,
            bytes_in: 0,
            bytes_out: 0,
            at_ms: 1_000,
        };
        // A quarter of the ticks were idle, so three quarters were work.
        assert_eq!(rates_between(before, after).0, 75.0);
    }

    #[test]
    fn counters_that_go_backwards_report_nothing_rather_than_a_spike() {
        let before = Sample {
            idle: 0,
            total: 0,
            bytes_in: 5_000,
            bytes_out: 5_000,
            at_ms: 0,
        };
        // An adapter that was disabled and re-enabled starts again at zero.
        let after = Sample {
            idle: 0,
            total: 0,
            bytes_in: 0,
            bytes_out: 0,
            at_ms: 1_000,
        };
        let (_, down, up) = rates_between(before, after);
        assert_eq!((down, up), (0.0, 0.0));
    }

    #[test]
    fn bytes_are_divided_by_the_seconds_they_took() {
        let before = Sample {
            idle: 0,
            total: 0,
            bytes_in: 0,
            bytes_out: 0,
            at_ms: 0,
        };
        let after = Sample {
            idle: 0,
            total: 0,
            bytes_in: 2_048,
            bytes_out: 1_024,
            at_ms: 2_000,
        };
        let (_, down, up) = rates_between(before, after);
        assert_eq!((down, up), (1_024.0, 512.0));
    }
}
