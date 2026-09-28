use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Memory::{
    VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_PRIVATE,
    PAGE_READWRITE, PAGE_EXECUTE_READWRITE,
};
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_VM_READ, PROCESS_VM_WRITE, PROCESS_VM_OPERATION, PROCESS_QUERY_INFORMATION};
// use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, SendMessageW};
// use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT};
use std::ffi::c_void;
use anyhow::Result;

// --- CAMERA SETTINGS ---
// Ты можешь менять эти значения здесь. 
// CAMERA_BASE_OFFSET - это базовый адрес (статичный указатель)
// CAMERA_OFFSETS - это цепочка оффсетов к зуму камеры
// pub const CAMERA_BASE_OFFSET: usize = 0x00C4B114;
//pub const CAMERA_OFFSETS: &[usize] = &[0x14, 0x4, 0x28, 0xB14];
pub const CAMERA_BASE_OFFSET: usize = 0x00c00d60;
pub const CAMERA_OFFSETS: &[usize] = &[0x144, 0x1b8, 0x1dc, 0x4, 0x248, 0xb14];

// --- Memory Manager ---

pub struct MemoryManager {
    handle: HANDLE,
}

impl MemoryManager {
    pub fn attach(pid: u32) -> Result<Self> {
        unsafe {
            let handle = OpenProcess(
                PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION | PROCESS_QUERY_INFORMATION,
                false,
                pid
            )?;
            Ok(Self { handle })
        }
    }

    pub fn get_base_address(&self) -> Option<usize> {
        use windows::Win32::System::ProcessStatus::EnumProcessModules;
        let mut modules = [windows::Win32::Foundation::HMODULE::default(); 1];
        let mut cb_needed = 0;
        unsafe {
            if EnumProcessModules(self.handle, modules.as_mut_ptr(), std::mem::size_of_val(&modules) as u32, &mut cb_needed).is_ok() {
                if cb_needed > 0 {
                    return Some(modules[0].0 as usize);
                }
            }
        }
        Some(0x00400000)
    }

    pub fn read_ptr32(&self, address: usize) -> Option<usize> {
        let mut buffer = [0u8; 4];
        let mut bytes_read = 0;
        unsafe {
            if ReadProcessMemory(
                self.handle,
                address as *const c_void,
                buffer.as_mut_ptr() as *mut c_void,
                4,
                Some(&mut bytes_read)
            ).is_ok() && bytes_read == 4 {
                Some(u32::from_le_bytes(buffer) as usize)
            } else {
                None
            }
        }
    }

    pub fn read_float(&self, address: usize) -> Option<f32> {
        let mut buffer = [0u8; 4];
        let mut bytes_read = 0;
        unsafe {
            if ReadProcessMemory(
                self.handle,
                address as *const c_void,
                buffer.as_mut_ptr() as *mut c_void,
                4,
                Some(&mut bytes_read)
            ).is_ok() && bytes_read == 4 {
                Some(f32::from_le_bytes(buffer))
            } else {
                None
            }
        }
    }

    pub fn read_memory(&self, address: usize, size: usize) -> Option<Vec<u8>> {
        let mut buffer = vec![0u8; size];
        let mut bytes_read = 0;
        unsafe {
             if ReadProcessMemory(
                self.handle,
                address as *const c_void,
                buffer.as_mut_ptr() as *mut c_void,
                size,
                Some(&mut bytes_read)
            ).is_ok() { // ReadProcessMemory might read partial, but we check size? Actually ReadProcessMemory returns BOOL but wrapped as Result
                if bytes_read == size {
                     Some(buffer)
                } else {
                     Some(buffer[..bytes_read].to_vec()) // Return what we got
                }
            } else {
                None
            }
        }
    }

    pub fn write_float(&self, address: usize, value: f32) -> bool {
        let buffer = value.to_le_bytes();
        let mut bytes_written = 0;
        unsafe {
            WriteProcessMemory(
                self.handle,
                address as *const c_void,
                buffer.as_ptr() as *const c_void,
                4,
                Some(&mut bytes_written)
            ).is_ok() && bytes_written == 4
        }
    }

    pub fn scan_for_value(&self, value: f32) -> Vec<usize> {
        let mut results = Vec::new();
        let target_bytes = value.to_le_bytes();
        let mut address = 0x10000000;
        let max_address = 0x7FFFFFFF;

        unsafe {
            let mut mbi: MEMORY_BASIC_INFORMATION = std::mem::zeroed();
            
            while address < max_address {
                if VirtualQueryEx(self.handle, Some(address as *const c_void), &mut mbi, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) == 0 {
                    address += 0x1000;
                    continue;
                }

                if mbi.State == MEM_COMMIT && mbi.Type == MEM_PRIVATE &&
                   (mbi.Protect & (PAGE_READWRITE | PAGE_EXECUTE_READWRITE) != Default::default()) {
                    
                    let mut region_size = mbi.RegionSize;
                     if region_size > 50 * 1024 * 1024 { region_size = 50 * 1024 * 1024; }

                     if let Some(buffer) = self.read_memory(mbi.BaseAddress as usize, region_size) {
                         // Naive search
                         for i in (0..buffer.len()).step_by(4) {
                             if i + 4 <= buffer.len() && buffer[i..i+4] == target_bytes {
                                 results.push(mbi.BaseAddress as usize + i);
                             }
                         }
                     }
                }
                address = (mbi.BaseAddress as usize + mbi.RegionSize) as usize;
            }
        }
        results
    }
}

impl Drop for MemoryManager {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

// --- Camera Scanner & Sync ---

#[derive(Clone, Debug)]
pub struct CameraAddresses {
    pub x1: usize,
    pub x2: usize,
    pub y1: usize,
    pub y2: usize,
    pub scroll1: usize,
    pub scroll2: usize,
}

pub struct CameraScanner;

impl CameraScanner {
    pub fn scan(pid: u32, _hwnd: usize, status_cb: impl Fn(&str)) -> Option<CameraAddresses> {
        let mem = MemoryManager::attach(pid).ok()?;
        
        let base_module = mem.get_base_address().unwrap_or(0x400000);
        status_cb(&format!("База: 0x{:X}", base_module));

        // Dereference levels one by one for diagnostics
        let mut current_addr = match mem.read_ptr32(base_module + CAMERA_BASE_OFFSET) {
            Some(0) => { status_cb("Err: Base ptr is 0"); return None; }
            Some(addr) => {
                status_cb(&format!("Lvl 0 (Base + Offset): 0x{:X}", addr));
                addr
            },
            None => { status_cb("Err: Base read failed"); return None; }
        };

        for (i, &offset) in CAMERA_OFFSETS.iter().enumerate() {
            let is_last = i == CAMERA_OFFSETS.len() - 1;
            let target = current_addr + offset;
            
            if is_last {
                if let Some(val) = mem.read_float(target) {
                    if val > 0.0 && val < 5000.0 {
                        status_cb("Указатель найден!");
                        return Some(CameraAddresses {
                            scroll2: target,
                            scroll1: target - 4,
                            x2: target - 8,
                            x1: target - 12,
                            y2: target - 16,
                            y1: target - 20,
                        });
                    } else {
                        status_cb(&format!("Err: Val={:.1} (Gbg)", val));
                        return None;
                    }
                }
            } else {
                // Intermediate dereference
                match mem.read_ptr32(target) {
                    Some(0) => { status_cb(&format!("Err: Lvl {} is 0", i + 1)); return None; }
                    Some(next_addr) => {
                        status_cb(&format!("Lvl {} OK: 0x{:X}", i + 1, next_addr));
                        current_addr = next_addr;
                    }
                    None => { status_cb(&format!("Err: Lvl {} read fail", i + 1)); return None; }
                }
            }
        }

        status_cb("Ошибка: указатель не найден.");
        None
    }
}
