#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    let default_filter = if cfg!(debug_assertions) {
        "info"
    } else {
        "warn"
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_filter))
        .init();

    // Async logger will be initialized lazily when first needed (after Tauri runtime starts)
    log::info!("Starting application");
    app_lib::run();
}
