mod constants;
mod injector;
mod logging;
mod save_file_step;
mod steam_id;
mod updater;

use injector::start_game;
use logging::{den_panic_hook, enable_ansi_support, setup_logging};
use save_file_step::check_saves;
use updater::start_updater;

fn main() {
    dotenv::dotenv().ok();

    enable_ansi_support().unwrap();

    setup_logging();

    std::panic::set_hook(Box::new(den_panic_hook));

    tracing::info!("Starting DenLauncher v{}", env!("CARGO_PKG_VERSION"));

    if std::env::args()
        .into_iter()
        .any(|arg| arg == "--skip-update")
    {
        tracing::info!("Skipping update check...");
    } else {
        tracing::info!("Checking for updates...");
        start_updater();
    }

    tracing::info!("Checking for valid save file...");
    check_saves();

    tracing::info!("Starting Elden Ring...");

    if let Err(err) = start_game() {
        tracing::error!("Failed to start Elden Ring: {:?}", err);
        std::thread::sleep(std::time::Duration::from_secs(5));
        std::process::exit(1);
    } else {
        tracing::info!("Elden Ring started successfully!");
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}
