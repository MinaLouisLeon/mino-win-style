//! The test that decides whether a tweak may ship.
//!
//! Apply everything, revert everything, and require that the registry is
//! byte-identical to where it started — including values that were absent
//! before and must be absent again.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use mino_core::provider::{Hive, RegLoc};
use mino_core::{
    Color, Engine, Journal, MemoryRegistry, NoopRefresher, OsBuild, RegValue, RegistryProvider,
    Value,
};

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("mino-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn hkcu(path: &str, name: &str) -> RegLoc {
    RegLoc {
        hive: Hive::CurrentUser,
        path: path.into(),
        name: name.into(),
    }
}

/// A machine that has been used: some values set, some never touched.
fn lived_in_registry() -> Arc<MemoryRegistry> {
    let reg = Arc::new(MemoryRegistry::new());
    let advanced = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
    let personalize = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

    reg.seed(&hkcu(personalize, "AppsUseLightTheme"), RegValue::Dword(1));
    reg.seed(
        &hkcu(personalize, "SystemUsesLightTheme"),
        RegValue::Dword(1),
    );
    reg.seed(&hkcu(advanced, "TaskbarAl"), RegValue::Dword(1));
    reg.seed(&hkcu(advanced, "HideFileExt"), RegValue::Dword(1));
    // `Hidden`, `TaskbarDa` and the whole accent ramp are deliberately absent.
    reg
}

fn engine_for(reg: Arc<MemoryRegistry>, journal_dir: PathBuf) -> Engine {
    Engine::new(
        reg as Arc<dyn RegistryProvider>,
        Arc::new(NoopRefresher::new()),
        OsBuild::fake(26200),
        Journal::new(journal_dir),
    )
}

fn wanted() -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("appearance.dark_mode".to_string(), Value::Bool(true)),
        (
            "appearance.accent_color".to_string(),
            Value::Color(Color::new(0x0F, 0x62, 0xC0)),
        ),
        (
            "taskbar.alignment".to_string(),
            Value::Choice("left".into()),
        ),
        ("taskbar.widgets".to_string(), Value::Bool(false)),
        (
            "explorer.show_file_extensions".to_string(),
            Value::Bool(true),
        ),
        (
            "explorer.classic_context_menu".to_string(),
            Value::Bool(true),
        ),
    ])
}

#[test]
fn planning_changes_nothing() {
    let reg = lived_in_registry();
    let before = reg.snapshot();
    let dir = temp_dir("plan");
    let engine = engine_for(Arc::clone(&reg), dir.clone());

    let plan = engine.plan("test", &wanted()).unwrap();
    assert!(!plan.is_empty(), "expected work to do");
    assert_eq!(reg.snapshot(), before, "plan() must not write");
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "plan() must not journal"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn apply_then_revert_all_restores_the_machine() {
    let reg = lived_in_registry();
    let before = reg.snapshot();
    let dir = temp_dir("revert");
    let engine = engine_for(Arc::clone(&reg), dir.clone());

    let plan = engine.plan("Test pack", &wanted()).unwrap();
    let report = engine.apply(&plan).unwrap();
    assert!(!report.entry.changes.is_empty());

    // The classic context menu needs Explorer restarted, and we must have asked
    // rather than done it ourselves.
    assert!(report.shell_restart_pending);

    let after = reg.snapshot();
    assert_ne!(after, before, "nothing actually changed");
    assert_eq!(
        engine.read("appearance.dark_mode").unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        engine.read("taskbar.alignment").unwrap(),
        Value::Choice("left".into())
    );

    engine.revert_all().unwrap();

    assert_eq!(
        reg.snapshot(),
        before,
        "revert left the registry in a different state than it found it"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn re_applying_the_same_settings_is_a_no_op() {
    let reg = lived_in_registry();
    let dir = temp_dir("noop");
    let engine = engine_for(Arc::clone(&reg), dir.clone());

    engine
        .apply(&engine.plan("first", &wanted()).unwrap())
        .unwrap();
    let second = engine.plan("second", &wanted()).unwrap();

    assert!(
        second.is_empty(),
        "expected no work the second time, got {:?}",
        second.items.iter().map(|i| &i.tweak).collect::<Vec<_>>()
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn unsupported_and_unknown_settings_are_reported_not_applied() {
    let reg = lived_in_registry();
    let dir = temp_dir("skip");
    // Windows 11 21H2: too old for Start_Layout, which needs 22H2.
    let engine = Engine::new(
        Arc::clone(&reg) as Arc<dyn RegistryProvider>,
        Arc::new(NoopRefresher::new()),
        OsBuild::fake(22000),
        Journal::new(dir.clone()),
    );

    let wanted = BTreeMap::from([
        (
            "start.layout".to_string(),
            Value::Choice("more_pins".into()),
        ),
        ("taskbar.does_not_exist".to_string(), Value::Bool(true)),
    ]);

    let plan = engine.plan("mixed", &wanted).unwrap();
    assert!(plan.is_empty());
    assert_eq!(plan.skipped.len(), 2);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn journal_and_reg_backup_land_on_disk() {
    let reg = lived_in_registry();
    let dir = temp_dir("journal");
    let engine = engine_for(Arc::clone(&reg), dir.clone());

    let report = engine
        .apply(&engine.plan("Dark mode", &wanted()).unwrap())
        .unwrap();

    let json = dir.join(format!("{}.json", report.entry.id));
    let reg_file = dir.join(format!("{}.reg", report.entry.id));
    assert!(json.exists(), "journal entry missing");
    assert!(reg_file.exists(), ".reg backup missing");

    let backup = std::fs::read_to_string(&reg_file).unwrap();
    assert!(backup.starts_with("Windows Registry Editor Version 5.00"));

    let history = engine.history().unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].label, "Dark mode");

    let _ = std::fs::remove_dir_all(dir);
}
