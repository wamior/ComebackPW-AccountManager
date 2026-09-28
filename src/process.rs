use sysinfo::{System, ProcessRefreshKind, UpdateKind};
use std::collections::HashMap;

use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, GetWindowTextW, SetWindowPos,
    HWND_TOP, SWP_NOACTIVATE, SWP_NOZORDER,
    EnumWindows, GetWindowThreadProcessId,
};
use windows::Win32::Foundation::{HWND, LPARAM, BOOL};
use windows::Win32::System::Threading::{
    OpenProcess, SetPriorityClass,
    PROCESS_QUERY_INFORMATION, PROCESS_SET_INFORMATION,
};
#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub hwnd: usize,
    pub window_title: String,
    pub user: Option<String>,
    pub role: Option<String>,
    pub priority_class: u32,
    pub affinity_mask: u64,
}

pub struct ProcessMonitor {
    system: System,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        Self {
            system: System::new_all(),
        }
    }

    pub fn refresh(&mut self) -> HashMap<u32, ProcessInfo> {
        let kind = ProcessRefreshKind::new().with_cmd(UpdateKind::Always);
        self.system.refresh_processes_specifics(kind);

        let mut result = HashMap::new();

        for (pid, process) in self.system.processes() {
            let name = process.name().to_lowercase();
            if !name.starts_with("elementclient") {
                continue;
            }

            let pid_u32 = pid.as_u32();
            let cmd = process.cmd();
            if cmd.is_empty() { continue; }

            let user = cmd.iter().find_map(|arg| arg.strip_prefix("user:").map(|v| v.to_string()));
            let role = cmd.iter().find_map(|arg| arg.strip_prefix("role:").map(|v| v.to_string()));
            if role.is_none() {
                continue;
            }


            let (hwnd, window_title) = find_window_for_pid(pid_u32);
            let (priority_class, affinity_mask) = get_process_info(pid_u32);

            result.insert(pid_u32, ProcessInfo {
                pid: pid_u32,
                name: process.name().to_string(),
                hwnd,
                window_title,
                user,
                role,
                priority_class,
                affinity_mask,
            });
        }

        result
    }

    pub fn set_process_priority(&self, pid: u32, level: u32) {
        let priority_class = match level {
            0 => 0x00000040u32,
            1 => 0x00004000u32,
            2 => 0x00000020u32,
            3 => 0x00008000u32,
            4 => 0x00000080u32,
            _ => 0x00000020u32,
        };

        unsafe {
            if let Ok(handle) = OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_SET_INFORMATION,
                false,
                pid,
            ) {
                let _ = SetPriorityClass(
                    handle,
                    windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(priority_class),
                );
                let _ = windows::Win32::Foundation::CloseHandle(handle);
            }
        }
    }

    pub fn set_cpu_affinity(&self, pid: u32, mask: u64) {
        use windows::Win32::System::Threading::SetProcessAffinityMask;
        unsafe {
            if let Ok(handle) = OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_SET_INFORMATION,
                false,
                pid,
            ) {
                let _ = SetProcessAffinityMask(handle, mask as usize);
                let _ = windows::Win32::Foundation::CloseHandle(handle);
            }
        }
    }

    pub fn check_window_position(&self, pid: u32) -> bool {
        let (hwnd, _) = find_window_for_pid(pid);
        if hwnd == 0 {
            return true;
        }
        unsafe {
            let mut rect = windows::Win32::Foundation::RECT::default();
            if GetWindowRect(HWND(hwnd as isize), &mut rect).is_ok() {
                return rect.left >= 0 && rect.top >= 0;
            }
        }
        true
    }

    pub fn fix_window_position(&self, pid: u32) -> Result<(), String> {
        let (hwnd, _) = find_window_for_pid(pid);
        if hwnd == 0 {
            return Err("Window not found".to_string());
        }
        unsafe {
            let _ = SetWindowPos(
                HWND(hwnd as isize),
                HWND_TOP,
                0,
                0,
                1024,
                768,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
        Ok(())
    }

    pub fn kill_process(&self, pid: u32) -> Result<(), String> {
        unsafe {
            if let Ok(handle) = OpenProcess(
                windows::Win32::System::Threading::PROCESS_TERMINATE,
                false,
                pid,
            ) {
                let _ = windows::Win32::System::Threading::TerminateProcess(handle, 1);
                let _ = windows::Win32::Foundation::CloseHandle(handle);
                return Ok(());
            }
        }
        Err("Failed to kill process".to_string())
    }

    pub fn switch_to_window(&self, pid: u32) {
        let (hwnd, _) = find_window_for_pid(pid);
        if hwnd != 0 {
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::ShowWindow(HWND(hwnd as isize), windows::Win32::UI::WindowsAndMessaging::SW_SHOW);
                windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(HWND(hwnd as isize));
            }
        }
    }
}

fn find_window_for_pid(target_pid: u32) -> (usize, String) {
    struct EnumData {
        target_pid: u32,
        hwnd: usize,
        title: String,
    }

    unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let data = &mut *(lparam.0 as *mut EnumData);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        if pid == data.target_pid {
            let mut buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, &mut buf);
            if len > 0 {
                let title = String::from_utf16_lossy(&buf[..len as usize]);
                if !title.is_empty() {
                    data.hwnd = hwnd.0 as usize;
                    data.title = title;
                    return BOOL(0);
                }
            }
        }
        BOOL(1)
    }

    let mut data = EnumData {
        target_pid,
        hwnd: 0,
        title: String::new(),
    };

    unsafe {
        let _ = EnumWindows(Some(enum_callback), LPARAM(&mut data as *mut _ as isize));
    }

    (data.hwnd, data.title)
}

fn get_process_info(pid: u32) -> (u32, u64) {
    use windows::Win32::System::Threading::{GetProcessAffinityMask, GetPriorityClass};

    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid) {
            let priority = GetPriorityClass(handle);
            let mut proc_mask: usize = 0;
            let mut sys_mask: usize = 0;
            let _ = GetProcessAffinityMask(handle, &mut proc_mask, &mut sys_mask);
            let _ = windows::Win32::Foundation::CloseHandle(handle);
            return (priority, proc_mask as u64);
        }
    }
    (0x00000020, u64::MAX)
}
