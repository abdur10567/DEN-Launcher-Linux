use crate::constants::{
    DEN_SAVE, OLD_SAVE_TIME_MARK, SAVE_STEM, VALID_SOURCE_SAVE_FILE_EXTENSIONS,
};
use crate::{constants::SAVE_EXTENSION, steam_id};
use cli_select::Select;
use std::io::stdout;
use std::thread;
use std::env;
use std::time::Duration;
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

fn get_save_list() -> Option<Vec<PathBuf>> {
    let steam_id = steam_id::get_steam_id();
    let save_file_path: PathBuf;

    let running_under_linux = std::env::var("WINEPREFIX").is_ok()
                    || std::env::var("PROTON_NO_ESYNC").is_ok();

    if running_under_linux{
        println!("linux block");

        //build save_file_path
        let user = env::var("USER").unwrap();
        let home_dir = format!("/home/{}", user);
        let appdata = format!("{}{}", home_dir, "/.local/share/Steam/steamapps/compatdata/1245620/pfx/drive_c/users/steamuser/AppData/Roaming/");
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
    let saves: Vec<PathBuf> = get_save_list()
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

    
    //this should be run only under windows
    //for linux, if there is a save file in the den saves location, ignore this.
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
