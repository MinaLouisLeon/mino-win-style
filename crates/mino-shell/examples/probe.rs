//! Prints what the dock would show, without drawing anything.
//!
//! `cargo run -p mino-shell --example probe`

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only.");
}

#[cfg(windows)]
fn main() {
    let area = mino_shell::work_area();
    println!(
        "work area: {}x{} at ({}, {})\n",
        area.width, area.height, area.x, area.y
    );

    let pinned = vec![r"C:\Windows\explorer.exe".to_string()];
    let items = mino_shell::dock_items(&pinned);

    println!("NAME                       PINNED      ICON  WINDOWS");
    for item in &items {
        let icon = mino_shell::icon_rgba(&item.exe, 64)
            .map(|i| format!("{}x{}", i.width, i.height))
            .unwrap_or_else(|| "-".into());
        println!(
            "{:<26} {:>6} {:>9}  {}",
            item.name,
            if item.pinned { "yes" } else { "" },
            icon,
            item.windows.len()
        );
        for window in &item.windows {
            let title: String = window.title.chars().take(58).collect();
            println!(
                "    {}{}",
                title,
                if window.minimized {
                    "  [minimised]"
                } else {
                    ""
                }
            );
        }
    }
    println!("\n{} entries", items.len());
}
