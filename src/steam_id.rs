use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use std::env;
use std::fs;
use std::thread;
use std::time::Duration;
use keyvalues_parser::{Vdf, Value};


//For windows only
const STEAM_ID_IDENT: u64 = 0x0110_0001_0000_0000;

pub fn get_steam_id() -> u64 {
    let running_under_linux = std::env::var("WINEPREFIX").is_ok() 
                        || std::env::var("PROTON_NO_ESYNC").is_ok();
    if running_under_linux {
        let user = env::var("USER").unwrap_or_else(|_| {
            eprintln!("USER environment variable not set");
            thread::sleep(Duration::from_secs(10));
            std::process::exit(1);
        });
        let home_dir = format!("/home/{}", user);
        let path = format!("{}/.steam/steam/config/loginusers.vdf", home_dir);

        let contents = fs::read_to_string(&path).unwrap_or_else(|_| {
            tracing::error!("Failed to locate Steam loginusers.vdf at {}", path);
            thread::sleep(Duration::from_secs(10));
            std::process::exit(1);
        });


        let vdf = Vdf::parse(&contents).unwrap_or_else(|_| {
            tracing::error!("Failed to parse Steam loginusers.vdf");
            thread::sleep(Duration::from_secs(10));
            std::process::exit(1);
        });

        let users_obj = vdf.value.unwrap_obj();

        let most_recent_steam_id: Option<u64> = users_obj.iter().find_map(|(steam_id, user_value)| {
            if let Value::Obj(ref obj_map) = user_value[0] {
                if let Some(Value::Str(most_recent)) = obj_map.get("MostRecent").and_then(|v| v.get(0)) {
                    if most_recent == "1" {
                        // Try to parse the Steam ID into u64
                        return steam_id.parse::<u64>().ok();
                    }
                }
            }
            None
        });

         // Return the u64 or exit if none found
        most_recent_steam_id.unwrap_or_else(|| {
            tracing::error!("No recent Steam user found");
            thread::sleep(Duration::from_secs(10));
            std::process::exit(1);
        })

    } else {
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

}

