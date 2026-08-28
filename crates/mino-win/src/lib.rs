//! Everything that knows it is running on Windows.
//!
//! `os` is portable (it only reads through a provider) so the test suite can
//! exercise build detection anywhere. The rest is `cfg(windows)`, which keeps
//! `cargo test -p mino-core` honest on any machine.

pub mod os;

#[cfg(windows)]
mod reg;
#[cfg(windows)]
mod shell;

#[cfg(windows)]
pub use reg::WindowsRegistry;
#[cfg(windows)]
pub use shell::WindowsShell;

#[cfg(windows)]
mod boot {
    use std::sync::Arc;

    use mino_core::error::Result;
    use mino_core::os::OsBuild;
    use mino_core::provider::{RegistryProvider, ShellRefresher};

    use crate::{WindowsRegistry, WindowsShell};

    /// The real providers plus the build we are running on. Every entry point —
    /// the app, the CLI — starts here so they cannot drift apart.
    pub fn boot() -> Result<(Arc<dyn RegistryProvider>, Arc<dyn ShellRefresher>, OsBuild)> {
        let registry = Arc::new(WindowsRegistry::new());
        let os = crate::os::detect(registry.as_ref())?;
        let shell = Arc::new(WindowsShell::new());
        Ok((registry, shell, os))
    }
}

#[cfg(windows)]
pub use boot::boot;
