use dotenv_codegen::dotenv;
use std::collections::HashSet;
use std::env;
use std::sync::LazyLock;
use std::time::Duration;
use windows::Win32::System::Threading::{
    PROCESS_ACCESS_RIGHTS, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_TERMINATE,
    PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};

pub static REPO_PRIVATE_KEY: LazyLock<String> = LazyLock::new(|| {
    env::var("REPO_PRIVATE_KEY").unwrap_or_else(|_| dotenv!("REPO_PRIVATE_KEY").to_string())
});

pub static REPO_OWNER: LazyLock<String> =
    LazyLock::new(|| env::var("REPO_OWNER").unwrap_or_else(|_| dotenv!("REPO_OWNER").to_string()));

pub static REPO_NAME: LazyLock<String> =
    LazyLock::new(|| env::var("REPO_NAME").unwrap_or_else(|_| dotenv!("REPO_NAME").to_string()));

pub static DLL_NAME: LazyLock<String> =
    LazyLock::new(|| env::var("DLL_NAME").unwrap_or_else(|_| dotenv!("DLL_NAME").to_string()));

pub static CONTENT_DIR: LazyLock<String> = LazyLock::new(|| {
    env::var("CONTENT_DIR").unwrap_or_else(|_| dotenv!("CONTENT_DIR").to_string())
});

pub static SAVE_EXTENSION: LazyLock<String> = LazyLock::new(|| {
    env::var("SAVE_EXTENSION").unwrap_or_else(|_| dotenv!("SAVE_EXTENSION").to_string())
});
pub const ELDENRING_ID: u32 = 1245620;
pub const ELDENRING_EXE: &str = "eldenring.exe";

pub const SAVE_STEM: &str = "ER0000";

pub static DEN_SAVE: LazyLock<String> =
    LazyLock::new(|| SAVE_STEM.to_string() + "." + SAVE_EXTENSION.as_str());

// constant 10.10.2024
pub const OLD_SAVE_TIME_MARK: Duration = Duration::from_secs(1728507600u64);

// pub const VALID_SOURCE_SAVE_FILE_EXTENSIONS: [&str; 4] = ["sl2", "co2", "160den", "170den"];
pub const VALID_SOURCE_SAVE_FILE_EXTENSIONS: LazyLock<HashSet<&str>> =
    LazyLock::new(|| HashSet::from(["sl2", "co2", "160den", "170den"]));

pub static PROCESS_INJECTION_ACCESS: LazyLock<PROCESS_ACCESS_RIGHTS> = LazyLock::new(|| {
    PROCESS_CREATE_THREAD
        | PROCESS_QUERY_INFORMATION
        | PROCESS_VM_OPERATION
        | PROCESS_VM_READ
        | PROCESS_VM_WRITE
        | PROCESS_TERMINATE
});
