//! `mino` — the recovery path.
//!
//! This binary exists mainly so that undoing a change never depends on the app
//! that made it. If the window will not open, `mino safe-restore` still works
//! from a command prompt.

#[cfg(not(windows))]
fn main() {
    eprintln!("mino-win-style runs on Windows 11 only.");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    if let Err(err) = windows_main::run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod windows_main {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    use clap::{Parser, Subcommand};

    use mino_core::error::{Error, Result};
    use mino_core::{Color, Engine, Journal, PackManifest, Plan, Value};

    #[derive(Parser)]
    #[command(
        name = "mino",
        about = "Change how Windows 11 looks — and put it back.",
        version
    )]
    struct Cli {
        /// Work out what would change, then stop.
        #[arg(long, global = true)]
        dry_run: bool,

        /// Use a different journal directory (mainly for testing).
        #[arg(long, global = true)]
        journal: Option<PathBuf>,

        #[command(subcommand)]
        command: Command,
    }

    #[derive(Subcommand)]
    enum Command {
        /// Which Windows this is, and whether it is supported.
        Os,
        /// Every setting, with its value on this machine.
        List {
            /// appearance | taskbar | start | explorer
            #[arg(long)]
            category: Option<String>,
        },
        /// Read one setting.
        Get { id: String },
        /// Write one setting, e.g. `mino set appearance.dark_mode true`.
        Set { id: String, value: String },
        /// Apply a pack manifest.
        Apply { manifest: PathBuf },
        /// What this app has done to the machine, newest first.
        History,
        /// Undo one entry from the history, or `latest`.
        Revert { entry: String },
        /// Undo everything this app has ever applied.
        RevertAll,
        /// Undo the most recent change. The command to reach for when something
        /// looks wrong and the app itself will not start.
        SafeRestore,
    }

    pub fn run() -> Result<()> {
        let cli = Cli::parse();
        let (registry, shell, os) = mino_win::boot()?;

        let journal = Journal::new(cli.journal.clone().unwrap_or_else(Journal::default_dir));
        let engine = Engine::new(Arc::clone(&registry), shell, os.clone(), journal);

        if !os.is_supported() && !matches!(cli.command, Command::Os) {
            return Err(Error::Unsupported {
                tweak: "this app".into(),
                reason: format!("{os} is older than Windows 11 (build 22000)"),
            });
        }

        match cli.command {
            Command::Os => {
                println!("{os}");
                println!(
                    "supported: {}",
                    if os.is_supported() { "yes" } else { "no" }
                );
            }

            Command::List { category } => {
                for state in engine.states() {
                    if let Some(wanted) = &category {
                        if state.info.category.id() != wanted {
                            continue;
                        }
                    }
                    let value = state
                        .value
                        .as_ref()
                        .map(|v| v.describe())
                        .or_else(|| state.error.clone())
                        .unwrap_or_else(|| "unavailable".into());
                    let tier = format!("{:?}", state.info.tier);
                    println!("{:<42} {:<10} {}", state.info.id, tier, value);
                    if let Some(note) = state.info.support.note() {
                        println!("{:<42} {:<10} {}", "", "", note.en);
                    }
                }
            }

            Command::Get { id } => {
                println!("{}", engine.read(&id)?.describe());
            }

            Command::Set { id, value } => {
                let wanted = BTreeMap::from([(id.clone(), parse_value(&value))]);
                let plan = engine.plan(format!("Set {id}"), &wanted)?;
                finish(&engine, plan, cli.dry_run)?;
            }

            Command::Apply { manifest } => {
                let pack = PackManifest::read(&manifest)?;
                if !pack.requires.allows(&os) {
                    return Err(Error::Unsupported {
                        tweak: pack.id.clone(),
                        reason: format!("this pack does not cover build {}", os.build),
                    });
                }
                let plan = engine.plan(
                    format!("Applied pack: {}", pack.display_name("en")),
                    &pack.settings,
                )?;
                finish(&engine, plan, cli.dry_run)?;
            }

            Command::History => {
                let entries = engine.history()?;
                if entries.is_empty() {
                    println!("Nothing has been changed by this app yet.");
                }
                for entry in entries {
                    println!(
                        "{}  {:<11}  {:>3} change(s)  {}",
                        // "2026-08-28T14:31:07.482Z" -> "2026-08-28 14:31:07"
                        entry
                            .when
                            .get(..19)
                            .unwrap_or(&entry.when)
                            .replace('T', " "),
                        format!("{:?}", entry.status),
                        entry.changes.len(),
                        entry.label
                    );
                    println!("            id: {}", entry.id);
                }
            }

            Command::Revert { entry } => {
                let id = resolve_entry(&engine, &entry)?;
                let report = engine.revert(&id)?;
                println!("Reverted {} change(s).", report.entry.changes.len());
                if report.shell_restart_pending {
                    println!("Some of it needs Explorer restarted: mino restart-shell is not");
                    println!("automatic — sign out, or restart Explorer from Task Manager.");
                }
            }

            Command::RevertAll => {
                let reports = engine.revert_all()?;
                let changes: usize = reports.iter().map(|r| r.entry.changes.len()).sum();
                println!(
                    "Reverted {} entr{} ({changes} change(s)).",
                    reports.len(),
                    if reports.len() == 1 { "y" } else { "ies" }
                );
            }

            Command::SafeRestore => {
                let id = resolve_entry(&engine, "latest")?;
                let report = engine.revert(&id)?;
                println!("Restored the state from before the last change.");
                println!("{} change(s) undone.", report.entry.changes.len());
            }
        }

        Ok(())
    }

    fn resolve_entry(engine: &Engine, entry: &str) -> Result<String> {
        if entry != "latest" {
            return Ok(entry.to_string());
        }
        engine
            .journal()
            .latest_revertible()?
            .map(|e| e.id)
            .ok_or_else(|| Error::Journal("there is nothing to revert".into()))
    }

    fn finish(engine: &Engine, plan: Plan, dry_run: bool) -> Result<()> {
        for skipped in &plan.skipped {
            println!("skipped {}: {}", skipped.tweak, skipped.reason);
        }

        if plan.is_empty() {
            println!("Already in that state — nothing to do.");
            return Ok(());
        }

        println!("{} change(s):", plan.change_count());
        for item in &plan.items {
            println!(
                "  {} : {} -> {}",
                item.tweak,
                item.from.describe(),
                item.to.describe()
            );
            for change in &item.changes {
                println!("      {}", change.describe());
            }
        }

        if dry_run {
            println!("\nDry run — nothing was written.");
            return Ok(());
        }

        let report = engine.apply(&plan)?;
        println!("\nApplied. Journal entry: {}", report.entry.id);
        println!("Undo with: mino revert {}", report.entry.id);
        if report.shell_restart_pending {
            println!("\nOne or more changes only show up once Explorer restarts.");
        }
        if report.sign_out_pending {
            println!("One or more changes need you to sign out and back in.");
        }
        Ok(())
    }

    /// `true`/`false`, `#RRGGBB`, or a choice id. The tweak validates the rest.
    fn parse_value(text: &str) -> Value {
        match text.trim().to_ascii_lowercase().as_str() {
            "true" | "on" | "yes" | "1" => return Value::Bool(true),
            "false" | "off" | "no" | "0" => return Value::Bool(false),
            _ => {}
        }
        if text.trim_start().starts_with('#') {
            if let Ok(color) = Color::parse(text) {
                return Value::Color(color);
            }
        }
        Value::Choice(text.trim().to_string())
    }
}
