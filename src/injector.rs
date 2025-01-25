use windows::Win32::Foundation::HANDLE;

use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use steamlocate::SteamDir;
use windows::core::{PCSTR, PSTR};
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Memory::{VirtualAllocEx, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE};
use windows::Win32::System::Threading::{
    CreateProcessA, CreateRemoteThread, OpenProcess, TerminateProcess, CREATE_NEW_PROCESS_GROUP,
    CREATE_SUSPENDED, PROCESS_INFORMATION, STARTUPINFOA,
};

use crate::constants::{
    CONTENT_DIR, DLL_NAME, ELDENRING_EXE, ELDENRING_ID, PROCESS_INJECTION_ACCESS,
};

fn locate_executable() -> PathBuf {
    let steam_dir = SteamDir::locate().expect("Failed to locate Steam directory");
    let (app, lib) = steam_dir
        .find_app(ELDENRING_ID)
        .ok()
        .flatten()
        .expect("Failed to locate Elden Ring");
    lib.resolve_app_dir(&app).join("Game").join(ELDENRING_EXE)
}

fn open_process_by_pid(pid: u32) -> Option<HANDLE> {
    unsafe {
        OpenProcess(
            // access required for performing dll injection
            *PROCESS_INJECTION_ACCESS,
            false,
            pid,
        )
    }
    .ok()
}

fn kill_process(pid: u32) {
    open_process_by_pid(pid).and_then(|handle| {
        unsafe { TerminateProcess(handle, 1) }
            .map_err(|err| tracing::error!("Failed to terminate process: {:?}", err))
            .ok()
    });
}

fn get_pids_by_name(name: &str) -> Vec<u32> {
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    system
        .processes()
        .values()
        .filter(move |process| process.name().to_str().is_some_and(|n| n.contains(name)))
        .map(|process| process.pid().as_u32())
        .collect()
}

pub fn start_game() {
    get_pids_by_name(ELDENRING_EXE)
        .iter()
        .for_each(|&pid| kill_process(pid));

    let executable_path = locate_executable();
    let dll_path = std::env::current_exe()
        .unwrap()
        .parent()
        .expect("Failed to get current executable dir path")
        .join(&*CONTENT_DIR)
        .join(&*DLL_NAME);

    tracing::info!("Injecting DLL: {:?}", dll_path);

    if !dll_path.exists() {
        panic!("DLL not found: {:?}", dll_path);
    }

    std::env::set_var("SteamAppId", ELDENRING_ID.to_string());

    let exe_path_cstr =
        std::ffi::CString::new(executable_path.to_str().unwrap()).expect("CString::new failed");

    let mut startup_info = STARTUPINFOA::default();
    let mut process_info = PROCESS_INFORMATION::default();
    // change cwd to the game directory
    let cwd = executable_path.parent().unwrap();
    std::env::set_current_dir(cwd).expect("Failed to change cwd");
    unsafe {
        CreateProcessA(
            PCSTR(exe_path_cstr.as_ptr() as *const u8),
            PSTR::null(),
            None,
            None,
            false,
            CREATE_SUSPENDED | CREATE_NEW_PROCESS_GROUP,
            None,
            None,
            &mut startup_info,
            &mut process_info,
        )
    }
    .unwrap();

    let dll_path_w = dll_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<u16>>();
    let buffer_size = dll_path_w.len() * std::mem::size_of::<u16>();

    let buffer = unsafe {
        VirtualAllocEx(
            process_info.hProcess,
            None,
            buffer_size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };

    unsafe {
        WriteProcessMemory(
            process_info.hProcess,
            buffer,
            dll_path_w.as_ptr() as *const _,
            buffer_size,
            None,
        )
        .expect("Failed to write process memory");
    };

    unsafe {
        GetModuleHandleA(PCSTR("kernel32.dll\0".as_ptr()))
            .map(|kernel32| {
                GetProcAddress(kernel32, PCSTR("LoadLibraryW\0".as_ptr())).map(|load_library| {
                    CreateRemoteThread(
                        process_info.hProcess,
                        None,
                        0,
                        Some(std::mem::transmute(load_library)),
                        Some(buffer),
                        0,
                        None,
                    )
                    .map(|handle| {
                        tracing::info!("DLL injected, waiting for dllmain");
                        windows::Win32::System::Threading::WaitForSingleObject(handle, 0xFFFFFFFF)
                    })
                    .expect("Failed to create remote thread")
                })
            })
            .expect("Failed to get module handle");
    };

    unsafe { windows::Win32::System::Threading::ResumeThread(process_info.hThread) };
}
