// Release builds have no console window; debug builds keep one so panics and
// tracing are visible while developing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    mino_win_style_lib::run()
}
