// Prevents an extra console window on Windows in release; harmless on macOS.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Agents start Redrule with --mcp to read notes; that never opens the app.
    if minutes_lib::agents::started_for_mcp() {
        return minutes_lib::agents::serve_mcp();
    }
    minutes_lib::run()
}
