use crate::constants::{
    DEN_SAVE, OLD_SAVE_TIME_MARK, SAVE_STEM, VALID_SOURCE_SAVE_FILE_EXTENSIONS, ELDENRING_ID,
};
use crate::{constants::SAVE_EXTENSION, steam_id};
use cli_select::Select;
use std::io::stdout;
use std::thread;
use std::env;
use std::fs;
use std::time::Duration;
use steam_shortcuts_util::parse_shortcuts;
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const STEAM_ID_IDENT: u64 = 0x0110_0001_0000_0000;

fn get_save_list(steam_id: u64) -> Option<Vec<PathBuf>> {
    let save_file_path: PathBuf;
    let running_under_linux = std::env::var("WINEPREFIX").is_ok()
                    || std::env::var("PROTON_NO_ESYNC").is_ok();

    if running_under_linux{
        println!("linux block");

        //build save_file_path
        let user = env::var("USER").unwrap();
        let home_dir = format!("/home/{}", user);
        let appdata = format!("{}/.local/share/Steam/steamapps/compatdata/{}/pfx/drive_c/users/steamuser/AppData/Roaming/", home_dir, ELDENRING_ID);
        let full_path = format!("{}{}{}", appdata,"EldenRing/",steam_id );

        save_file_path = PathBuf::from(full_path);

        //check if its valid
        if !save_file_path.exists(){
            println!("save_file_path path does not exist");
            thread::sleep(Duration::from_secs(10));
            std::process::exit(1);
        }
    } else {
        println!("windows block");
        //build save_file_path for windows
        let appdata = std::env::var("APPDATA").expect("APPDATA not found");
        save_file_path = Path::new(&appdata)
            .join("EldenRing")
            .join(steam_id.to_string());
    }


    tracing::info!("Save file path: {:?}", save_file_path);
    let mut save_files = Vec::new();
    let save_stem = OsStr::new(SAVE_STEM);
    for entry in save_file_path.read_dir().ok()? {
        let path = entry.unwrap().path();
        if path.is_dir() {
            continue;
        }
        if path.file_stem() == Some(save_stem) {
            save_files.push(path);
        }
    }
    Some(save_files)
}

fn get_den_save_location(steam_id: u64) -> String {
    let steam3_id = steam_id - STEAM_ID_IDENT;

    //build path for shortcuts.vdf
    let user = env::var("USER").unwrap();
    let home_dir = format!("/home/{}", user);
    let path = format!("{}/.local/share/Steam/userdata/{}/config/shortcuts.vdf", home_dir, steam3_id);

    //read and parse contents
    let contents = fs::read(&path).unwrap_or_else(|_| {
        tracing::error!("Failed to locate Steam shortcuts.vdf at {}", path);
        println!("Failed to locate Steam shortcuts.vdf at {}", path);
        thread::sleep(Duration::from_secs(10));
        std::process::exit(1);
    });
    let shortcuts = parse_shortcuts(contents.as_slice());

    //Find the app_id of DEN-Launcher.exe
    let den_steam_app_id: Option<u32> = if let Ok(shortcuts_vec) = &shortcuts {
        shortcuts_vec.iter()
            .find(|shortcut| shortcut.app_name == "DEN-Launcher.exe")
            .map(|shortcut| {
                println!("Found DEN-Launcher.exe with app_id: {}", shortcut.app_id);
                shortcut.app_id
           })
    } else {
        tracing::error!("Failed to parse shortcuts");
        None
    };

    //Now build the path for the den save file location
    let appdata = match den_steam_app_id {
        Some(app_id) => format!(
            "{}/.local/share/Steam/steamapps/compatdata/{}/pfx/drive_c/users/steamuser/AppData/Roaming/",
            home_dir, app_id
        ),
        None => {
            tracing::error!("Could not determine DEN-Launcher App ID");
            println!("Failed to find DEN-Launcher App ID");
            thread::sleep(Duration::from_secs(10));
            std::process::exit(1);
        }
    };

    let save_file_path = format!("{}{}{}", appdata,"EldenRing/",steam_id );
    save_file_path
}

fn get_save_list_linux_den(steam_id: u64) -> Option<Vec<PathBuf>> {
    let save_file_path = get_den_save_location(steam_id);

    //Return vector of all saves in den save file location
    tracing::info!("Save file path linux den: {:?}", save_file_path);
    let mut save_files = Vec::new();
    let save_stem = OsStr::new(SAVE_STEM);
    for entry in std::fs::read_dir(&save_file_path).ok()? {
        let path = entry.unwrap().path();
        if path.is_dir() {
            continue;
        }
        if path.file_stem() == Some(save_stem) {
            save_files.push(path);
        }
    }
    Some(save_files)
}

fn pick_base_save(saves: Vec<PathBuf>) -> Option<PathBuf> {
    tracing::warn!("No {} save file found", &*DEN_SAVE);
    println!("Select a save file to use as a base:");
    println!("Use the arrow keys to navigate and Enter to select.\n");

    let save_files_map = saves
        .iter()
        .map(|f| (f.file_name().unwrap().to_str().unwrap(), f))
        .collect::<HashMap<_, _>>();

    let mut keys: Vec<_> = save_files_map.keys().collect();
    // Insert create new option
    keys.push(&"Create new save");
    keys.reverse();

    let &selected_save = Select::new(&keys, stdout()).start();
    Some(save_files_map.get(selected_save)?.to_path_buf())
}

pub fn check_saves() {
    let steam_id = steam_id::get_steam_id();
    let saves: Vec<PathBuf> = get_save_list(steam_id)
        .map(|s| {
            s.into_iter()
                .inspect(|f| tracing::info!("Checking save file: {:?}", f))
                .filter(|f| {
                    f.metadata().unwrap().modified().unwrap() >= UNIX_EPOCH + OLD_SAVE_TIME_MARK
                })
                .filter(|f| {
                    let ext = f.extension().unwrap().to_str().unwrap();
                    VALID_SOURCE_SAVE_FILE_EXTENSIONS.contains(&ext) || ext == *SAVE_EXTENSION
                })
                .collect()
        })
        .unwrap_or_default();

    let running_under_linux = std::env::var("WINEPREFIX").is_ok()
                    || std::env::var("PROTON_NO_ESYNC").is_ok();

    if running_under_linux{
        let user = env::var("USER").unwrap();
        let home_dir = format!("/home/{}", user);
        let appdata = format!("{}/.local/share/Steam/steamapps/compatdata/{}/pfx/drive_c/users/steamuser/AppData/Roaming/", home_dir, ELDENRING_ID);
        let elden_ring_save_path = format!("{}{}{}", appdata,"EldenRing/",steam_id );
        let den_path = get_den_save_location(steam_id);
        // Create the den save directory if it doesn't exist
        std::fs::create_dir_all(&den_path)
            .expect("Failed to create den save directory");

        //first get save files in the linux den location
        let saves_linux: Vec<PathBuf> = get_save_list_linux_den(steam_id)
            .map(|s| {
                s.into_iter()
                    .inspect(|f| tracing::info!("Checking save file: {:?}", f))
                    .filter(|f| {
                        f.metadata().unwrap().modified().unwrap() >= UNIX_EPOCH + OLD_SAVE_TIME_MARK
                    })
                    .filter(|f| {
                        let ext = f.extension().unwrap().to_str().unwrap();
                        VALID_SOURCE_SAVE_FILE_EXTENSIONS.contains(&ext) || ext == *SAVE_EXTENSION
                    })
                    .collect()
            })
            .unwrap_or_default();

        //check for existing valid den save file and sync it if it exists. 
        for save in &saves_linux {
            let save_name = save.file_name().unwrap().to_str().unwrap();
            if save_name.eq(&format!("{}.{}", SAVE_STEM, &*SAVE_EXTENSION)) {
                tracing::info!("Found valid save file: {:?}", save);
                //Copy and overwrite the ER000.170 save file in the Elden Ring save folder aka sync
                let destination_file = format!("{}/ER0000.170den", elden_ring_save_path);
                match std::fs::copy(save, &destination_file) {
                    Ok(_) => {
                        tracing::info!("Successfully copied save file to: {}", destination_file);
                    },
                    Err(e) => {
                        tracing::error!("Failed to sync .170den save file with one in Elden Ring folder: {}", e);
                    }
                }       
                return;
            }
        }

        //No valid save in linux den found, so continue
        //check for save files in elden ring save folder

        //If none in Elden Ring folder 
        if saves.is_empty() {
            tracing::warn!(
                "No existing save files found in Elden Ring save folder, game will create and use {}",
                &*DEN_SAVE
            );
            return;
        }

        //Since there are some, check for a .170den file
        //if one is found, copy it to den save location
        for save in &saves {
            let save_name = save.file_name().unwrap().to_str().unwrap();
            if save_name.eq(&format!("{}.{}", SAVE_STEM, &*SAVE_EXTENSION)) {
                tracing::info!("Found valid save file in Elden Ring save location: {:?}", save);
                let destination_file = format!("{}/ER0000.170den", den_path);
                match std::fs::copy(save, &destination_file) {
                    Ok(_) => {
                        tracing::info!("Successfully copied save file to: {}", destination_file);
                    },
                    Err(e) => {
                        tracing::error!("Failed to copy .170den save file from Elden Ring save folder to DEN save folder: {}", e);
                    }
                }
                return;
            }
        }
        
        //since no 170den file either elden ring or den folder, let user pick a base save
        let save = pick_base_save(saves);
        if let Some(s) = save {
            tracing::debug!("Selected save: {:?}", s);


            let destination = PathBuf::from(&den_path)
                .join(SAVE_STEM)
                .with_extension(&*SAVE_EXTENSION);
            std::fs::copy(&s, &destination)
                .expect("Failed to copy save file");
        }



        
    } else {
        //windows block
        if saves.is_empty() {
            tracing::warn!(
                "No existing save files found, game will create and use {}",
                &*DEN_SAVE
            );
            return;
        }



        // iterate over save files exit function if valid save file is found
        for save in &saves {
            let save_name = save.file_name().unwrap().to_str().unwrap();
            if save_name.eq(&format!("{}.{}", SAVE_STEM, &*SAVE_EXTENSION)) {
                tracing::info!("Found valid save file: {:?}", save);
                return;
            }
        }

        let save = pick_base_save(saves);
        if let Some(s) = save {
            tracing::debug!("Selected save: {:?}", s);
            std::fs::copy(
                &s,
                s.parent()
                    .unwrap()
                    .join(SAVE_STEM)
                    .with_extension(&*SAVE_EXTENSION),
            )
            .expect("Failed to copy save file");
        }
    }
}
    
