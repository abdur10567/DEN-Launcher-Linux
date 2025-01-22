mod constants;
mod injector;
mod save_file_step;
mod steam_id;
mod updater;

use injector::start_game;
use save_file_step::check_saves;
use tracing_panic::panic_hook;
use tracing_subscriber::{self, layer::SubscriberExt, util::SubscriberInitExt, Layer};
use updater::start_updater;

pub fn den_panic_hook(panic_info: &std::panic::PanicHookInfo) {
    let message;
    let title = "Den-Launcher Error";
    let reason = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
        *s
    } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
        s.as_str()
    } else {
        "Unknown"
    };
    if let Some(location) = panic_info.location() {
        message = format!(
            "A panic occurred at {}:{}\nReason: {}",
            location.file(),
            location.line(),
            reason
        );
    } else {
        message = format!("A panic occurred\nReason: {}", reason);
    }

    let mut message_utf16: Vec<u16> = message.encode_utf16().collect();
    message_utf16.push(0);
    let mut title_utf16: Vec<u16> = title.encode_utf16().collect();
    title_utf16.push(0);

    unsafe {
        windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
            None,
            windows::core::PCWSTR(message_utf16.as_ptr()),
            windows::core::PCWSTR(title_utf16.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
        );
    }
    panic_hook(panic_info);
    std::thread::sleep(std::time::Duration::from_secs(10));
}

fn setup_logging() {
    let stdout_log = tracing_subscriber::fmt::layer().pretty();
    let filter = tracing_subscriber::filter::EnvFilter::from_default_env().add_directive(
        if cfg!(debug_assertions) {
            tracing_subscriber::filter::LevelFilter::DEBUG.into()
        } else {
            tracing_subscriber::filter::LevelFilter::INFO.into()
        },
    );
    let registry = tracing_subscriber::registry().with(stdout_log.with_filter(filter));
    if std::env::var("DEN_DEBUG").is_ok() || cfg!(debug_assertions) {
        let appender = tracing_appender::rolling::never("./", "denlauncher.log");
        let file_log = tracing_subscriber::fmt::layer()
            .with_writer(appender)
            .with_ansi(false);
        registry.with(file_log).init();
    } else {
        registry.init();
    }
}

fn main() {
    dotenv::dotenv().ok();
    let _ = unsafe { windows::Win32::System::Console::AllocConsole() };
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

    start_game();

    tracing::info!("Done!");
    std::thread::sleep(std::time::Duration::from_secs(3));
}
