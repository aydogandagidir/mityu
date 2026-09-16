#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    // NO logger is installed here.
    //
    // There used to be an `env_logger` writing to stdout, and on Windows that
    // meant the shipped app logged NOWHERE: the release binary is built with
    // `windows_subsystem = "windows"` (above), so it has no console attached
    // and every line went to a handle nothing reads. A user whose recording
    // would not start had no file to send and no way to say why — which is
    // exactly how that bug reached a release undiagnosed.
    //
    // Logging is now installed by `tauri-plugin-log` inside `app_lib::run()`,
    // which writes to the OS log directory as well as stdout. It has to be the
    // ONLY logger in the process: `log` accepts exactly one, and a second
    // installer (`env_logger::init`) panics or fails the plugin's setup, which
    // would abort startup.
    app_lib::run();
}
