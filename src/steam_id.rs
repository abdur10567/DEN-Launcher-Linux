use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

const STEAM_ID_IDENT: u64 = 0x0110_0001_0000_0000;

pub fn get_steam_id() -> u64 {
    let hklm = RegKey::predef(HKEY_CURRENT_USER);
    let subkey = hklm
        .open_subkey("Software\\Valve\\Steam\\ActiveProcess")
        .unwrap();

    match subkey.get_value::<u32, _>("ActiveUser") {
        Err(_) | Ok(0) => {
            tracing::error!("Failed to get Steam ID, is Steam running?");
            std::thread::sleep(std::time::Duration::from_secs(10));
            std::process::exit(1);
        }
        Ok(steam_id) => steam_id as u64 + STEAM_ID_IDENT,
    }
}
