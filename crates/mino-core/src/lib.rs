//! The engine behind mino-win-style.
//!
//! Two rules hold this crate together:
//!
//! 1. **Nothing here knows it is on Windows.** Every side effect goes through
//!    [`provider::RegistryProvider`] or [`provider::ShellRefresher`], which
//!    `mino-win` implements for real and [`provider::MemoryRegistry`] fakes for
//!    tests. The whole planner runs on any machine, in milliseconds.
//! 2. **A tweak describes; the engine acts.** [`tweak::Tweak::plan`] is pure.
//!    [`engine::Engine::apply`] is the only path to a write, which is why
//!    journalling and rollback exist once rather than once per setting.

pub mod engine;
pub mod error;
pub mod journal;
pub mod os;
pub mod profile;
pub mod provider;
pub mod time;
pub mod tweak;
pub mod tweaks;
pub mod value;

pub use engine::{ApplyReport, Engine, Plan, PlanItem, Skipped};
pub use error::{Error, Result};
pub use journal::{Journal, JournalEntry, Status};
pub use os::{OsBuild, Support, MIN_SUPPORTED_BUILD};
pub use profile::{PackManifest, Settings};
pub use provider::{
    Hive, MemoryRegistry, NoopRefresher, RegLoc, RegValue, RegistryProvider, ShellRefresher,
};
pub use tweak::{Category, Change, Privilege, Refresh, Tier, Tweak, TweakInfo, TweakState};
pub use tweaks::TweakRegistry;
pub use value::{Color, Value, ValueKind};
