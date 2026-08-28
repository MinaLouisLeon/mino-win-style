use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::journal::{Journal, JournalEntry, Status};
use crate::os::OsBuild;
use crate::provider::{RegistryProvider, ShellRefresher};
use crate::tweak::{Change, Privilege, Refresh, Tier, Tweak, TweakInfo, TweakState};
use crate::tweaks::TweakRegistry;
use crate::value::Value;

#[derive(Debug, Clone, Serialize)]
pub struct PlanItem {
    pub tweak: String,
    pub from: Value,
    pub to: Value,
    pub tier: Tier,
    pub privilege: Privilege,
    pub refresh: Refresh,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skipped {
    pub tweak: String,
    pub reason: String,
}

/// What would happen, worked out without touching anything. The UI shows this
/// verbatim before the user commits.
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub label: String,
    pub items: Vec<PlanItem>,
    pub skipped: Vec<Skipped>,
    pub needs_elevation: bool,
    pub needs_shell_restart: bool,
    pub needs_sign_out: bool,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn change_count(&self) -> usize {
        self.items.iter().map(|i| i.changes.len()).sum()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplyReport {
    pub entry: JournalEntry,
    /// True when the user still has to agree to restart Explorer for the change
    /// to show up. We never restart it behind their back.
    pub shell_restart_pending: bool,
    pub sign_out_pending: bool,
}

pub struct Engine {
    reg: Arc<dyn RegistryProvider>,
    shell: Arc<dyn ShellRefresher>,
    os: OsBuild,
    tweaks: TweakRegistry,
    journal: Journal,
}

impl Engine {
    pub fn new(
        reg: Arc<dyn RegistryProvider>,
        shell: Arc<dyn ShellRefresher>,
        os: OsBuild,
        journal: Journal,
    ) -> Self {
        Engine {
            reg,
            shell,
            os,
            tweaks: TweakRegistry::builtin(),
            journal,
        }
    }

    pub fn os(&self) -> &OsBuild {
        &self.os
    }

    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    pub fn infos(&self) -> Vec<TweakInfo> {
        self.tweaks
            .iter()
            .map(|t| TweakInfo {
                id: t.id(),
                category: t.category(),
                tier: t.tier(),
                kind: t.value_kind(),
                privilege: t.privilege(),
                refresh: t.refresh(),
                support: t.support(&self.os),
            })
            .collect()
    }

    /// Every tweak with its value on this machine. A read failure becomes an
    /// `error` on that one row instead of failing the whole call — one broken
    /// setting must not blank the window.
    pub fn states(&self) -> Vec<TweakState> {
        self.tweaks
            .iter()
            .map(|t| {
                let info = TweakInfo {
                    id: t.id(),
                    category: t.category(),
                    tier: t.tier(),
                    kind: t.value_kind(),
                    privilege: t.privilege(),
                    refresh: t.refresh(),
                    support: t.support(&self.os),
                };
                if !info.support.is_usable() {
                    return TweakState {
                        info,
                        value: None,
                        error: None,
                    };
                }
                match t.read(self.reg.as_ref()) {
                    Ok(value) => TweakState {
                        info,
                        value: Some(value),
                        error: None,
                    },
                    Err(err) => TweakState {
                        info,
                        value: None,
                        error: Some(err.to_string()),
                    },
                }
            })
            .collect()
    }

    pub fn read(&self, id: &str) -> Result<Value> {
        self.tweak(id)?.read(self.reg.as_ref())
    }

    fn tweak(&self, id: &str) -> Result<&dyn Tweak> {
        self.tweaks
            .get(id)
            .ok_or_else(|| Error::UnknownTweak(id.to_string()))
    }

    /// Works out the difference between what is wanted and what is there.
    /// Nothing is written, and nothing is journalled — this is safe to call on
    /// every keystroke.
    pub fn plan(&self, label: impl Into<String>, desired: &BTreeMap<String, Value>) -> Result<Plan> {
        let mut items = Vec::new();
        let mut skipped = Vec::new();

        for (id, want) in desired {
            let tweak = match self.tweaks.get(id) {
                Some(t) => t,
                None => {
                    skipped.push(Skipped {
                        tweak: id.clone(),
                        reason: format!("This build of the app does not know the setting `{id}`."),
                    });
                    continue;
                }
            };

            let support = tweak.support(&self.os);
            if !support.is_usable() {
                skipped.push(Skipped {
                    tweak: id.clone(),
                    reason: support.note().unwrap_or("Not supported here.").to_string(),
                });
                continue;
            }

            let from = match tweak.read(self.reg.as_ref()) {
                Ok(v) => v,
                Err(err) => {
                    skipped.push(Skipped {
                        tweak: id.clone(),
                        reason: format!("Could not read the current value: {err}"),
                    });
                    continue;
                }
            };

            let set = tweak.plan(self.reg.as_ref(), want)?;
            if set.is_empty() {
                continue; // already in the wanted state
            }

            items.push(PlanItem {
                tweak: id.clone(),
                from,
                to: want.clone(),
                tier: tweak.tier(),
                privilege: tweak.privilege(),
                refresh: set.refresh,
                changes: set.changes,
            });
        }

        let needs_elevation = items.iter().any(|i| i.privilege == Privilege::Elevated);
        let needs_shell_restart = items.iter().any(|i| i.refresh == Refresh::RestartShell);
        let needs_sign_out = items.iter().any(|i| i.refresh == Refresh::SignOut);

        Ok(Plan {
            label: label.into(),
            items,
            skipped,
            needs_elevation,
            needs_shell_restart,
            needs_sign_out,
        })
    }

    /// Applies a plan as one batch: journal first, then write, then refresh.
    /// If any write fails, everything already written in this batch is undone
    /// before the error is returned.
    pub fn apply(&self, plan: &Plan) -> Result<ApplyReport> {
        if plan.needs_elevation {
            // M3 delivers the broker. Until then we refuse rather than half-apply.
            let names: Vec<&str> = plan
                .items
                .iter()
                .filter(|i| i.privilege == Privilege::Elevated)
                .map(|i| i.tweak.as_str())
                .collect();
            return Err(Error::NeedsElevation(names.join(", ")));
        }

        let changes: Vec<Change> = plan
            .items
            .iter()
            .flat_map(|i| i.changes.iter().cloned())
            .collect();
        let tweaks: Vec<String> = plan.items.iter().map(|i| i.tweak.clone()).collect();

        let mut entry = JournalEntry::new(plan.label.clone(), self.os.build, tweaks, changes);
        if entry.changes.is_empty() {
            entry.status = Status::Applied;
            return Ok(ApplyReport {
                entry,
                shell_restart_pending: false,
                sign_out_pending: false,
            });
        }

        // Written while status is Pending, together with the .reg backup, so a
        // crash between here and the last write still leaves a full undo record.
        self.journal.write(&entry)?;

        let mut done: Vec<Change> = Vec::new();
        for change in &entry.changes {
            match execute(self.reg.as_ref(), change) {
                Ok(()) => done.push(change.clone()),
                Err(err) => {
                    for applied in done.iter().rev() {
                        // Best effort: if undo itself fails there is nothing
                        // further we can do from here, and the .reg backup on
                        // disk is the remaining path back.
                        let _ = execute(self.reg.as_ref(), &applied.inverted());
                    }
                    entry.status = Status::RolledBack;
                    self.journal.write(&entry)?;
                    return Err(err);
                }
            }
        }

        entry.status = Status::Applied;
        self.journal.write(&entry)?;

        self.run_refreshes(plan.items.iter().map(|i| i.refresh))?;

        Ok(ApplyReport {
            entry,
            shell_restart_pending: plan.needs_shell_restart,
            sign_out_pending: plan.needs_sign_out,
        })
    }

    /// Puts one journal entry back. Reverting is applying the inverse batch, so
    /// it goes through the same journalling path and is itself revertible.
    pub fn revert(&self, entry_id: &str) -> Result<ApplyReport> {
        let mut original = self.journal.load(entry_id)?;
        if original.status == Status::Reverted {
            return Err(Error::Journal(format!(
                "entry {entry_id} has already been reverted"
            )));
        }

        let undo = original.undo_changes();
        let mut entry = JournalEntry::new(
            format!("Reverted: {}", original.label),
            self.os.build,
            original.tweaks.clone(),
            undo,
        );
        self.journal.write(&entry)?;

        let mut done: Vec<Change> = Vec::new();
        for change in &entry.changes {
            match execute(self.reg.as_ref(), change) {
                Ok(()) => done.push(change.clone()),
                Err(err) => {
                    for applied in done.iter().rev() {
                        let _ = execute(self.reg.as_ref(), &applied.inverted());
                    }
                    entry.status = Status::RolledBack;
                    self.journal.write(&entry)?;
                    return Err(err);
                }
            }
        }

        entry.status = Status::Applied;
        self.journal.write(&entry)?;

        original.status = Status::Reverted;
        self.journal.write(&original)?;

        // We do not know which tweaks these keys belonged to any more, so refresh
        // broadly but still stop short of restarting Explorer on our own.
        self.shell.broadcast_setting_change("ImmersiveColorSet")?;
        self.shell.notify_assoc_changed()?;

        Ok(ApplyReport {
            entry,
            shell_restart_pending: true,
            sign_out_pending: false,
        })
    }

    /// Undoes every applied entry, newest first.
    pub fn revert_all(&self) -> Result<Vec<ApplyReport>> {
        let mut reports = Vec::new();
        for entry in self.journal.list()? {
            if entry.status == Status::Applied && !entry.changes.is_empty() {
                reports.push(self.revert(&entry.id)?);
            }
        }
        Ok(reports)
    }

    pub fn history(&self) -> Result<Vec<JournalEntry>> {
        self.journal.list()
    }

    /// Only ever called after the user agrees.
    pub fn restart_explorer(&self) -> Result<()> {
        self.shell.restart_explorer()
    }

    fn run_refreshes(&self, refreshes: impl Iterator<Item = Refresh>) -> Result<()> {
        let mut areas: BTreeSet<&'static str> = BTreeSet::new();
        let mut assoc = false;
        let mut cursors = false;

        for refresh in refreshes {
            match refresh {
                Refresh::Broadcast(area) => {
                    areas.insert(area);
                }
                Refresh::AssocChanged => assoc = true,
                Refresh::Cursors => cursors = true,
                // Handled by the caller: both need the user to agree first.
                Refresh::RestartShell | Refresh::SignOut | Refresh::None => {}
            }
        }

        for area in areas {
            self.shell.broadcast_setting_change(area)?;
        }
        if assoc {
            self.shell.notify_assoc_changed()?;
        }
        if cursors {
            self.shell.refresh_cursors()?;
        }
        Ok(())
    }
}

/// The one function in the codebase that changes the machine.
fn execute(reg: &dyn RegistryProvider, change: &Change) -> Result<()> {
    match change {
        Change::Value { loc, to, .. } => match to {
            Some(value) => reg.write(loc, value),
            None => reg.delete_value(loc),
        },
        Change::Key {
            hive,
            path,
            to_present,
            ..
        } => {
            if *to_present {
                reg.create_key(*hive, path)
            } else {
                reg.delete_key(*hive, path)
            }
        }
    }
}
