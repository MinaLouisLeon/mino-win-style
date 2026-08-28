//! Which Windows this is.
//!
//! Read from the registry rather than from `GetVersionEx`, which lies to
//! processes without the right manifest and would quietly hand us build 19041
//! on a 25H2 machine — precisely the number the compatibility matrix gates on.

use mino_core::error::Result;
use mino_core::os::OsBuild;
use mino_core::provider::{Hive, RegLoc, RegValue, RegistryProvider};

const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

fn loc(name: &str) -> RegLoc {
    RegLoc {
        hive: Hive::LocalMachine,
        path: CURRENT_VERSION.to_string(),
        name: name.to_string(),
    }
}

pub fn detect(reg: &dyn RegistryProvider) -> Result<OsBuild> {
    let build = match reg.read(&loc("CurrentBuildNumber"))? {
        Some(RegValue::Sz(text)) => text.trim().parse::<u32>().unwrap_or(0),
        Some(RegValue::Dword(d)) => d,
        _ => 0,
    };

    let ubr = reg
        .read(&loc("UBR"))?
        .and_then(|v| v.as_dword())
        .unwrap_or(0);

    let display_version = match reg.read(&loc("DisplayVersion"))? {
        Some(RegValue::Sz(text)) => text,
        _ => String::new(),
    };

    let product_name = match reg.read(&loc("ProductName"))? {
        Some(RegValue::Sz(text)) => {
            // The value still says "Windows 10" on Windows 11 — Microsoft never
            // updated it, and plenty of tools report the wrong OS because of it.
            if build >= mino_core::os::WIN11_21H2 {
                text.replace("Windows 10", "Windows 11")
            } else {
                text
            }
        }
        _ => "Windows".to_string(),
    };

    Ok(OsBuild {
        build,
        ubr,
        display_version,
        product_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mino_core::provider::MemoryRegistry;

    #[test]
    fn reads_the_build_as_text_and_fixes_the_product_name() {
        let reg = MemoryRegistry::new();
        reg.seed(&loc("CurrentBuildNumber"), RegValue::Sz("26200".into()));
        reg.seed(&loc("UBR"), RegValue::Dword(8106));
        reg.seed(&loc("DisplayVersion"), RegValue::Sz("25H2".into()));
        reg.seed(
            &loc("ProductName"),
            RegValue::Sz("Windows 10 Pro".into()),
        );

        let os = detect(&reg).unwrap();
        assert_eq!(os.build, 26200);
        assert_eq!(os.ubr, 8106);
        assert_eq!(os.display_version, "25H2");
        assert_eq!(os.product_name, "Windows 11 Pro");
        assert!(os.is_supported());
    }

    #[test]
    fn a_blank_machine_is_not_supported() {
        let reg = MemoryRegistry::new();
        assert!(!detect(&reg).unwrap().is_supported());
    }
}
