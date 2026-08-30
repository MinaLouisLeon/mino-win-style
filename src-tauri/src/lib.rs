pub mod commands;
pub mod dock;
pub mod packs;
pub mod shell_look;
pub mod state;
pub mod top_bar;

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
        // The overlay's processor and network figures are rates, so the sampler
        // has to remember the previous reading between calls. One per process.
        .manage(mino_shell::Sampler::new())
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
            dock::dock_set_reveal,
            dock::dock_set_placement,
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
            shell_look::shell_config,
            shell_look::shell_looks,
            shell_look::shell_set_look,
            shell_look::shell_set_options,
            shell_look::shell_telemetry,
            top_bar::top_bar_config,
            top_bar::top_bar_set_enabled,
            top_bar::top_bar_foreground,
            top_bar::top_bar_task_view,
            top_bar::top_bar_open_settings,
            top_bar::top_bar_quit,
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
            // One call for all three states: on screen, waiting at the bottom
            // edge for the pointer, or off.
            dock::apply_mode(&handle, &config);

            // Same story for the overlay, and for the same reason: it is built
            // here on the main thread whether or not a Look is being worn.
            let shell = shell_look::ShellConfig::load();
            if let Err(err) = shell_look::create(&handle) {
                dock::trace(&format!("overlay create() failed: {err}"));
            }
            if shell.active.is_some() {
                shell_look::apply_surfaces(&handle, &shell);
            }

            // And the bar. Same rule again — built here, shown only if asked
            // for — with the difference that showing it reserves a strip of the
            // desktop, which is why the exit handler below exists.
            let bar = top_bar::TopBarConfig::load();
            if let Err(err) = top_bar::create(&handle) {
                dock::trace(&format!("bar create() failed: {err}"));
            }
            if bar.enabled {
                if let Err(err) = top_bar::show(&handle) {
                    dock::trace(&format!("bar show() failed: {err}"));
                }
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to start the Mino Win Style window")
        .run(|_app, event| {
            // The strip the bar reserved has to be handed back, and this is the
            // last place it can be. An appbar left registered leaves a band of
            // dead screen that survives a reboot with nothing on it to say why
            // — the worst thing this program can do to a machine. The window's
            // own WM_NCDESTROY unregisters too; this catches the exits that do
            // not go through one.
            if matches!(event, tauri::RunEvent::Exit) {
                mino_shell::appbar::unregister_all();
            }
        });
}
