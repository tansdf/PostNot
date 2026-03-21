#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = postnot_lib::run() {
        postnot_lib::report_startup_failure(&error);
    }
}
