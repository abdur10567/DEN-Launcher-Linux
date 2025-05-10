use std::io::{Seek, Write};

use crate::{
    constants::{CONTENT_DIR, DLL_NAME, ELDENRING_EXE, REPO_NAME, REPO_OWNER, REPO_PRIVATE_KEY},
    injector::{get_pids_by_name, kill_process},
};

use const_format::formatcp;
use semver::Version;
use serde::Deserialize;
use walkdir::WalkDir;
use zip::ZipArchive;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize, Clone)]
struct ReleaseAsset {
    pub url: String,
    pub name: String,
}

#[derive(Deserialize, Clone)]
struct Release {
    pub tag_name: String,
    pub assets: Vec<ReleaseAsset>,
}

pub fn bump_is_greater(current: &str, other: &str) -> Option<bool> {
    Some(Version::parse(other).ok()? > Version::parse(current).ok()?)
}

fn get_update() -> Option<Release> {
    let mut request = ureq::get(&format!(
        "https://api.github.com/repos/{}/{}/releases",
        *REPO_OWNER, *REPO_NAME
    ))
    .set("User-Agent", formatcp!("denlauncher/{}", VERSION))
    .query("per_page", "20");

    if !REPO_PRIVATE_KEY.is_empty() {
        request = request.set("Authorization", &format!("token {}", *REPO_PRIVATE_KEY));
    }

    let response = request
        .call()
        .map_err(|e| {
            tracing::error!("Failed to fetch releases: {}", e);
            e
        })
        .ok()?
        .into_json::<Vec<Release>>()
        .map_err(|e| {
            tracing::error!("Failed to parse JSON: {}", e);
            e
        })
        .ok()?;

    response
        .into_iter()
        .find(|r| bump_is_greater(VERSION, r.tag_name.trim_start_matches("v")).unwrap_or(false))
}

// #[cfg(not(debug_assertions))]
fn verify_signature(
    archive: &mut std::fs::File,
    context: &[u8],
    keys: &[[u8; zipsign_api::PUBLIC_KEY_LENGTH]],
) -> Result<(), zipsign_api::ZipsignError> {
    if keys.is_empty() {
        return Ok(());
    }

    tracing::info!("Verifying signature of update archive");

    let keys = keys.iter().copied().map(Ok);
    let keys = zipsign_api::verify::collect_keys(keys).map_err(zipsign_api::ZipsignError::from)?;

    zipsign_api::verify::verify_zip(archive, &keys, Some(context))
        .map_err(zipsign_api::ZipsignError::from)?;
    Ok(())
}

fn update_from_asset(asset: &ReleaseAsset) {
    for pid in get_pids_by_name(ELDENRING_EXE) {
        kill_process(pid);
    }

    let (exe_dir, content_dir, dll_path) = get_paths();
    let (tmp_archive, tmp_dir) = download_asset(asset);
    remove_old_content(content_dir, dll_path);
    extract_archive(&tmp_archive, &tmp_dir);
    perform_binary_replacement(&tmp_dir, exe_dir);
}

fn get_paths() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let current_exe = std::env::current_exe().unwrap();
    let exe_dir = current_exe.parent().unwrap().to_path_buf();
    let content_dir = exe_dir.join(&*CONTENT_DIR);
    let dll_path = content_dir.join(&*DLL_NAME);
    (exe_dir, content_dir, dll_path)
}

fn download_asset(asset: &ReleaseAsset) -> (std::fs::File, tempfile::TempDir) {
    let tmp_archive_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let mut tmp_archive_file = tempfile::tempfile().expect("Failed to create temp file");

    tracing::info!("Downloading archive: {}", asset.url);

    let mut request = ureq::get(&asset.url)
        .set(
            "User-Agent",
            formatcp!("denlauncher/{}", env!("CARGO_PKG_VERSION")),
        )
        .set("Accept", "application/octet-stream");
    if !REPO_PRIVATE_KEY.is_empty() {
        request = request.set("Authorization", &format!("token {}", *REPO_PRIVATE_KEY));
    }
    let mut buf = Vec::new();
    let response = request.call().expect("Failed to download archive");

    if response.status() != 200 {
        panic!("Failed to download archive: HTTP {}", response.status());
    }

    response
        .into_reader()
        .read_to_end(&mut buf)
        .expect("Failed to read archive content");

    tmp_archive_file
        .write_all(&buf)
        .expect("Failed to write to temp file");

    // Rewind the file cursor to the beginning before verifying the signature
    tmp_archive_file
        .seek(std::io::SeekFrom::Start(0))
        .expect("Failed to seek to start of temp file");

    tracing::info!("Downloaded archive: {:?}", tmp_archive_file);

    verify_signature(
        &mut tmp_archive_file,
        asset.name.as_bytes(),
        &[*include_bytes!("../release_public_key.bin")],
    )
    .expect("Failed to verify update archive signature");

    (tmp_archive_file, tmp_archive_dir)
}

fn extract_archive(tmp_archive: &std::fs::File, temp_dir: &tempfile::TempDir) {
    tracing::debug!("Extracting archive to: {:?}", tmp_archive);

    ZipArchive::new(tmp_archive)
        .expect("Failed to open archive")
        .extract(temp_dir)
        .expect("Failed to extract archive");
}

fn remove_old_content(content_dir: std::path::PathBuf, dll_path: std::path::PathBuf) {
    tracing::info!("Removing old content");
    std::fs::remove_file(&dll_path)
        .map_err(|e| tracing::warn!("Failed to remove old DLL at {:?}: {}", &dll_path, e))
        .ok();
    // remove content dir if it's empty
    std::fs::read_dir(&content_dir)
        .map(|dir| dir.count())
        .map(|count| {
            if count == 0 {
                std::fs::remove_dir(&content_dir)
                    .map_err(|e| {
                        tracing::warn!(
                            "Failed to remove old content dir at {:?}: {}",
                            &content_dir,
                            e
                        )
                    })
                    .ok();
            }
        })
        .ok();
}

fn perform_binary_replacement(tmp_dir: &tempfile::TempDir, exe_dir: std::path::PathBuf) {
    let mut new_exe_path: Option<std::path::PathBuf> = None;

    for entry in WalkDir::new(tmp_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        let relative_path = path.strip_prefix(tmp_dir).expect("Failed to strip prefix");
        let target_path = exe_dir.join(relative_path);

        if entry.file_type().is_file() {
            if let Some(path) = handle_file_entry(entry, &target_path) {
                new_exe_path = Some(path);
            }
        } else if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target_path).expect("Failed to create directory");
        }
    }

    if let Some(new_exe_path) = new_exe_path {
        tracing::info!("Replacing binary with new version");
        self_replace::self_replace(new_exe_path).expect("Failed to replace binary");
    } else {
        self_replace::self_delete().expect("Failed to delete updater");
    }
}

fn handle_file_entry(
    entry: walkdir::DirEntry,
    target_path: &std::path::PathBuf,
) -> Option<std::path::PathBuf> {
    // returns path if it's the current exe, otherwise copies the file
    if entry.file_name()
        == std::env::current_exe()
            .expect("Failed to get current exe name")
            .file_name()
            .unwrap()
    {
        Some(entry.path().to_path_buf())
    } else {
        std::fs::copy(entry.path(), target_path).expect("Failed to copy file");
        None
    }
}

pub fn start_updater() {
    if let Some(release) = get_update() {
        tracing::info!(
            "Found new release: {}",
            release.tag_name.trim_start_matches("v")
        );

        if let Some(asset) = release
            .assets
            .iter()
            .find(|asset| asset.name.ends_with(".zip"))
        {
            update_from_asset(asset)
        }
        tracing::info!("Update complete, please restart the launcher");
        std::thread::sleep(std::time::Duration::from_secs(5));
        std::process::exit(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bump_is_greater() {
        assert_eq!(bump_is_greater("1.0.0", "1.0.1"), Some(true));
        assert_eq!(bump_is_greater("1.0.1", "1.0.0"), Some(false));
        assert_eq!(bump_is_greater("1.0.0", "1.0.0"), Some(false));
        assert_eq!(bump_is_greater("1.0.0", "invalid"), None);
        assert_eq!(bump_is_greater("invalid", "1.0.0"), None);
        assert_eq!(
            bump_is_greater("2.0.0-beta.10", "2.0.0-beta.9"),
            Some(false)
        );
        assert_eq!(bump_is_greater("2.0.0-beta9", "2.0.0-beta.10"), Some(false));
        assert_eq!(
            bump_is_greater("2.0.0-rc.1", "2.0.0-rc.1+patch.1"),
            Some(true)
        );
    }
}
