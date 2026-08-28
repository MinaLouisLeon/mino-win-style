//! The bridge. Every command is thin on purpose: validation, planning and
//! journalling all live in `mino-core`, so the UI cannot reach past them.
//!
//! Note what is *not* here — there is no `write_registry_value` command. The
//! front end can only name settings the engine already implements.

use std::collections::BTreeMap;

use mino_core::{ApplyReport, JournalEntry, OsBuild, Plan, TweakState, Value};
use tauri::State;

use crate::state::AppState;

/// Commands return `Result<T, String>`: the UI shows the message, so it has to
/// be a sentence a person can act on rather than a debug dump.
type Answer<T> = Result<T, String>;

fn fail(err: impl std::fmt::Display) -> String {
    err.to_string()
}

#[tauri::command]
pub fn os_info(state: State<'_, AppState>) -> OsBuild {
    state.engine.os().clone()
}

#[tauri::command]
pub fn list_tweaks(state: State<'_, AppState>) -> Vec<TweakState> {
    state.engine.states()
}

/// Called on every change in the UI. Pure — safe to run on each keystroke.
#[tauri::command]
pub fn plan_changes(
    state: State<'_, AppState>,
    label: String,
    settings: BTreeMap<String, Value>,
) -> Answer<Plan> {
    state.engine.plan(label, &settings).map_err(fail)
}

#[tauri::command]
pub fn apply_changes(
    state: State<'_, AppState>,
    label: String,
    settings: BTreeMap<String, Value>,
) -> Answer<ApplyReport> {
    let plan = state.engine.plan(label, &settings).map_err(fail)?;
    state.engine.apply(&plan).map_err(fail)
}

#[tauri::command]
pub fn history(state: State<'_, AppState>) -> Answer<Vec<JournalEntry>> {
    state.engine.history().map_err(fail)
}

#[tauri::command]
pub fn revert_entry(state: State<'_, AppState>, id: String) -> Answer<ApplyReport> {
    state.engine.revert(&id).map_err(fail)
}

#[tauri::command]
pub fn revert_all(state: State<'_, AppState>) -> Answer<Vec<ApplyReport>> {
    state.engine.revert_all().map_err(fail)
}

/// Only ever called after the user has agreed in the confirmation dialog.
#[tauri::command]
pub fn restart_explorer(state: State<'_, AppState>) -> Answer<()> {
    state.engine.restart_explorer().map_err(fail)
}

#[tauri::command]
pub fn journal_dir(state: State<'_, AppState>) -> String {
    state.engine.journal().dir().display().to_string()
}
