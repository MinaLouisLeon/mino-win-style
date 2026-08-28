use mino_core::{Engine, Journal};

pub struct AppState {
    pub engine: Engine,
}

impl AppState {
    /// Builds the engine from the real machine. Any failure here means we could
    /// not even read the registry, which is not something to paper over — the
    /// caller shows it and stops.
    #[cfg(windows)]
    pub fn boot() -> mino_core::Result<Self> {
        let (registry, shell, os) = mino_win::boot()?;
        let engine = Engine::new(registry, shell, os, Journal::new(Journal::default_dir()));
        Ok(AppState { engine })
    }
}
