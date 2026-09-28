#![windows_subsystem = "windows"]
use sysinfo::{System, ProcessRefreshKind, UpdateKind};
use std::time::Duration;
use std::thread;
use serde::Serialize;
const API_URL: &str = "http://auth.comeback-links.ru";
const API_KEY: &str = "rust_app_api_key_secret_123";
const HEARTBEAT_INTERVAL_MS: u64 = 2000;

#[derive(Serialize, Debug, Clone, PartialEq)]
struct SessionEntry {
    login: String,
    role: String,
}

#[derive(Serialize)]
struct HeartbeatPayload {
    hostname: String,
    sessions: Vec<SessionEntry>,
}

fn get_hostname() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn normalize_field(value: &str) -> String {
    value
        .replace('\u{00A0}', " ")
        .trim()
        .to_string()
}

fn scan_sessions(system: &System) -> Vec<SessionEntry> {
    let mut sessions = Vec::new();
    for (_pid, process) in system.processes() {
        let name = process.name().to_lowercase();
        if !name.starts_with("elementclient") {
            continue;
        }
        let cmd = process.cmd();
        if cmd.is_empty() { continue; }
        let login = cmd.iter().find_map(|arg| arg.strip_prefix("user:").map(normalize_field));
        let role = cmd.iter().find_map(|arg| arg.strip_prefix("role:").map(normalize_field));
        if let (Some(l), Some(r)) = (login, role) {
            if !sessions.iter().any(|s: &SessionEntry| s.role.trim().eq_ignore_ascii_case(r.trim())) {
                sessions.push(SessionEntry { login: l, role: r });
            }
        }
    }
    sessions
}

fn main() {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build() {
            Ok(c) => c,
            Err(_) => return,
        };

    let mut system = System::new_all();
    let refresh_kind = ProcessRefreshKind::new().with_cmd(UpdateKind::Always);
    let hostname = get_hostname();

    loop {
        system.refresh_processes_specifics(refresh_kind);
        let final_sessions = scan_sessions(&system);

        let url = format!("{}/api/rust/heartbeat", API_URL);
        let payload = HeartbeatPayload {
            hostname: hostname.clone(),
            sessions: final_sessions,
        };

        let _ = client.post(&url)
            .header("x-api-key", API_KEY)
            .json(&payload)
            .timeout(Duration::from_secs(5))
            .send();

        thread::sleep(Duration::from_millis(HEARTBEAT_INTERVAL_MS));
    }
}
