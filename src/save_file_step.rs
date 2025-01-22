use crate::constants::{
    DEN_SAVE, OLD_SAVE_TIME_MARK, SAVE_STEM, VALID_SOURCE_SAVE_FILE_EXTENSIONS,
};
use crate::{constants::SAVE_EXTENSION, steam_id};
use cli_select::Select;
use std::io::stdout;
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

fn get_save_list() -> Option<Vec<PathBuf>> {
    let steam_id = steam_id::get_steam_id();
    let appdata = std::env::var("APPDATA").expect("APPDATA not found");
    let save_file_path = Path::new(&appdata)
        .join("EldenRing")
        .join(steam_id.to_string());

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
                .join(&*SAVE_STEM)
                .with_extension(&*SAVE_EXTENSION),
        )
        .expect("Failed to copy save file");
    }
}
