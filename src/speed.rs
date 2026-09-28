use crate::camera::MemoryManager;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use std::thread;
use std::time::Duration;

pub const SPEED_BASE_OFFSET: usize = 0x00c4d10c;
pub const SPEED_OFFSETS: &[usize] = &[0x14, 0x4, 0x28, 0x3f0, 0x0, 0x544];
pub const SPEED_INCREMENT: f32 = 2.5111;

pub struct SpeedManager;

impl SpeedManager {
    pub fn get_speed_address(mem: &MemoryManager) -> Option<usize> {
        let base_module = mem.get_base_address().unwrap_or(0x400000);
        let mut current_addr = mem.read_ptr32(base_module + SPEED_BASE_OFFSET)?;
        
        for (i, &offset) in SPEED_OFFSETS.iter().enumerate() {
            let target = current_addr + offset;
            if i == SPEED_OFFSETS.len() - 1 {
                return Some(target);
            } else {
                current_addr = mem.read_ptr32(target)?;
                if current_addr == 0 { return None; }
            }
        }
        None
    }
}

pub fn run_speed_sync(enabled_pids: Arc<Mutex<HashSet<u32>>>) {
    thread::spawn(move || {
        let mut last_base_speeds: std::collections::HashMap<u32, f32> = std::collections::HashMap::new();
        
        loop {
            thread::sleep(Duration::from_millis(100));
            
            let pids: Vec<u32> = {
                let enabled = enabled_pids.lock().unwrap();
                enabled.iter().cloned().collect()
            };

            for pid in pids {
                if let Ok(mem) = MemoryManager::attach(pid) {
                    if let Some(addr) = SpeedManager::get_speed_address(&mem) {
                        if let Some(current_speed) = mem.read_float(addr) {
                            let target_bonus = SPEED_INCREMENT;
                            
                            // "Умная" логика: 
                            // Мы хотим, чтобы итоговая скорость была speed_base + 2.5111
                            // Но игра часто сама меняет скорость (бафы, дебафы, петы).
                            
                            let last_base = last_base_speeds.get(&pid).cloned().unwrap_or(0.0);
                            
                            // Если текущая скорость НЕ равна (last_base + bonus), 
                            // значит либо мы только начали, либо игра изменила базовую скорость.
                            if (current_speed - (last_base + target_bonus)).abs() > 0.001 {
                                // Если текущая скорость НЕ является нашим результатом, 
                                // значит это новая базовая скорость.
                                let new_base = current_speed;
                                let new_target = new_base + target_bonus;
                                
                                if mem.write_float(addr, new_target) {
                                    last_base_speeds.insert(pid, new_base);
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}
