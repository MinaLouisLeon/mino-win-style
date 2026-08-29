pub mod commands;
pub mod dock;
pub mod packs;
pub mod state;

use state::AppState;

pub fn run() {
    let app_state = match AppState::boot() {
        Ok(state) => state,
        Err(err) => {
            // Nothing to show a message in yet, so say it where it will be seen
            // and stop. Starting with a half-built engine would be worse.
            eprintln!("Mino Win Style could not start: {err}");
            std::process::exit(1);
        }
    };

    let os = app_state.engine.os().clone();
    if !os.is_supported() {
        eprintln!("Mino Win Style needs Windows 11 (build 22000 or newer). Found: {os}");
        std::process::exit(1);
    }

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::os_info,
            commands::list_tweaks,
            commands::plan_changes,
            commands::apply_changes,
            commands::history,
            commands::revert_entry,
            commands::revert_all,
            commands::restart_explorer,
            commands::journal_dir,
            commands::list_packs,
            commands::plan_pack,
            commands::apply_pack,
            dock::dock_config,
            dock::dock_set_enabled,
            dock::dock_layout,
            dock::dock_items,
            dock::dock_icon,
            dock::dock_activate,
            dock::dock_launch,
            dock::dock_place,
            dock::dock_trace,
            dock::dock_minimize,
            dock::dock_toggle_maximize,
            dock::dock_close,
            dock::dock_pin,
            dock::dock_unpin,
        ])
        .setup(|app| {
            // The window is built here, on the main thread, whether or not the
            // dock is switched on — see dock::create for why it cannot be built
            // later. Toggling then only shows and hides it.
            let handle = app.handle().clone();
            let config = dock::DockConfig::load();
            dock::trace(&format!("setup: dock enabled = {}", config.enabled));
            if let Err(err) = dock::create(&handle) {
                dock::trace(&format!("create() failed: {err}"));
            }
            if config.enabled {
                if let Err(err) = dock::show(&handle) {
                    dock::trace(&format!("show() failed: {err}"));
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start the Mino Win Style window");
}
