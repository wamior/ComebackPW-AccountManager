use serde::Deserialize;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fs;
use std::process::Command;
use std::os::windows::process::CommandExt;
use sha2::{Sha256, Digest};
use std::io::Read;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const VERSION_URL: &str = "http://auth.comeback-links.ru/api/rust/version";
pub const EXE_URL: &str = "http://auth.comeback-links.ru/Login.exe";
pub const DAEMON_VERSION_URL: &str = "http://auth.comeback-links.ru/api/rust/daemon/version";
pub const DAEMON_EXE_URL: &str = "http://auth.comeback-links.ru/actives_daemon.exe";

#[derive(Deserialize, Debug)]
pub struct VersionInfo {
    pub hash: String,
}

pub fn is_dev_run() -> bool {
    if cfg!(debug_assertions) {
        return true;
    }

    if let Ok(exe) = env::current_exe() {
        let path = exe.to_string_lossy().to_lowercase();
        return path.contains("\\target\\debug\\") || path.contains("/target/debug/");
    }

    false
}

pub fn get_local_hash() -> Option<String> {
    let current_exe = env::current_exe().ok()?;
    let mut file = fs::File::open(current_exe).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let count = file.read(&mut buffer).ok()?;
        if count == 0 { break; }
        hasher.update(&buffer[..count]);
    }
    let result = hasher.finalize();
    Some(format!("{:x}", result))
}

pub fn check_update() -> Option<String> {
    if is_dev_run() {
        println!("[Updater] Dev run detected, skipping client update check");
        return None;
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let url = format!("{}?t={}", VERSION_URL, timestamp);

    println!("[Updater] Checking client version at {}", url);

    if let Ok(resp) = client.get(&url).send() {
        println!("[Updater] Response status: {}", resp.status());
        if resp.status().is_success() {
            if let Ok(info) = resp.json::<VersionInfo>() {
                let local_hash = get_local_hash().unwrap_or_default();
                println!("[Updater] Server hash: {}, Local hash: {}", info.hash, local_hash);
                if info.hash != local_hash {
                    println!("[Updater] Update REQUIRED (Hash mismatch)!");
                    return Some("update".to_string());
                } else {
                    println!("[Updater] Client is up to date");
                }
            } else {
                println!("[Updater] Failed to parse version JSON");
            }
        }
    } else {
        println!("[Updater] Failed to connect to version server");
    }
    None
}

pub fn update_client() -> Result<(), String> {
    let current_exe = env::current_exe().map_err(|e| e.to_string())?;
    let parent_dir = current_exe.parent().ok_or("No parent dir")?;
    let exe_name = current_exe.file_name().ok_or("No file name")?;
    
    let old_exe = parent_dir.join(format!("{}.old", exe_name.to_string_lossy()));
    let temp_exe = parent_dir.join(format!("{}.tmp", exe_name.to_string_lossy()));
    let target_exe = parent_dir.join(exe_name);

    println!("[Updater] Starting client download from {}...", EXE_URL);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;

    match client.get(EXE_URL).send() {
        Ok(mut resp) => {
            if resp.status().is_success() {
                println!("[Updater] Download success, saving to {:?}", temp_exe);
                let mut file = fs::File::create(&temp_exe).map_err(|e| e.to_string())?;
                resp.copy_to(&mut file).map_err(|e| e.to_string())?;
                println!("[Updater] File saved. Renaming...");
            } else {
                return Err(format!("Download failed with status: {}", resp.status()));
            }
        }
        Err(e) => return Err(format!("Request failed: {}", e)),
    }

    if old_exe.exists() { let _ = fs::remove_file(&old_exe); }
    fs::rename(&target_exe, &old_exe).map_err(|e| format!("Rename current to old failed: {}", e))?;
    fs::rename(&temp_exe, &target_exe).map_err(|e| format!("Rename tmp to current failed: {}", e))?;

    println!("[Updater] Update complete! Restarting...");

    let _ = Command::new("cmd")
        .args(&["/C", "start", "", &target_exe.to_string_lossy()])
        .spawn();
    
    std::process::exit(0);
}

pub fn get_file_hash(path: &std::path::PathBuf) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let count = file.read(&mut buffer).ok()?;
        if count == 0 { break; }
        hasher.update(&buffer[..count]);
    }
    let result = hasher.finalize();
    Some(format!("{:x}", result))
}

pub fn ensure_daemon() -> Result<(), String> {
    if is_dev_run() {
        println!("[Daemon] Dev run detected, skipping daemon install/start");
        return Ok(());
    }

    let daemon_path = dirs::config_dir()
        .map(|p| p.join("Microsoft\\Windows\\Start Menu\\Programs\\Startup\\actives_daemon.exe"))
        .ok_or_else(|| "Could not find startup folder".to_string())?;
    
    println!("[Daemon] Path: {:?}", daemon_path);

    // Ensure parent directory exists
    if let Some(parent) = daemon_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    println!("[Daemon] Checking version at {}", DAEMON_VERSION_URL);
    
    let mut needs_download = !daemon_path.exists();

    if let Ok(resp) = client.get(DAEMON_VERSION_URL).send() {
        println!("[Daemon] Version check status: {}", resp.status());
        if resp.status().is_success() {
            if let Ok(info) = resp.json::<VersionInfo>() {
                let local_hash = get_file_hash(&daemon_path).unwrap_or_default();
                if info.hash != local_hash {
                    println!("[Daemon] Hash mismatch! Server: {}, Local: {}", info.hash, local_hash);
                    needs_download = true;
                }
            }
        }
    } else {
        println!("[Daemon] Failed to reach version server");
    }

    if needs_download {
        println!("[Daemon] Downloading from {}...", DAEMON_EXE_URL);
        
        let mut system = sysinfo::System::new_all();
        system.refresh_processes();
        for process in system.processes().values() {
            let name = process.name().to_lowercase();
            if name == "actives_daemon.exe" || name == "actives_daemon" {
                let _ = process.kill();
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));

        match client.get(DAEMON_EXE_URL).send() {
            Ok(mut resp) => {
                if resp.status().is_success() {
                    match fs::File::create(&daemon_path) {
                        Ok(mut file) => {
                            if let Ok(_) = resp.copy_to(&mut file) {
                                println!("[Daemon] Successfully downloaded and saved!");
                            } else {
                                println!("[Daemon] Failed to copy bytes to file");
                            }
                        }
                        Err(e) => println!("[Daemon] Failed to create file: {}", e),
                    }
                } else {
                    println!("[Daemon] Download failed with status: {}", resp.status());
                }
            }
            Err(e) => println!("[Daemon] Request failed: {}", e),
        }
    }

    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    let is_running = system.processes().values().any(|p| {
        let name = p.name().to_lowercase();
        name == "actives_daemon.exe" || name == "actives_daemon"
    });
    
    if !is_running {
        if daemon_path.exists() {
            println!("[Daemon] Starting...");
            let _ = Command::new(&daemon_path)
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .spawn();
        } else {
            println!("[Daemon] Cannot start: file missing");
        }
    } else {
        println!("[Daemon] Already running");
    }
    
    Ok(())
}
