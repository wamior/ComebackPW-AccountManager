#![windows_subsystem = "windows"]

use eframe::egui;
use comeback_multitool::app;
use comeback_multitool::updater;
use sysinfo::System;

fn enforce_single_instance() {
    let mut system = System::new_all();
    system.refresh_processes();
    let current_pid = std::process::id();
    for (pid, process) in system.processes() {
        if pid.as_u32() == current_pid { continue; }
        let name = process.name().to_lowercase();
        if name == "login.exe" {
             let _ = process.kill();
        }
    }
}

fn cleanup_old_files() {
    if let Ok(exe) = std::env::current_exe() {
        let old_exe = exe.with_extension("old");
        let temp_exe = exe.with_extension("tmp");
        std::thread::spawn(move || {
            for _ in 0..10 {
                let _ = std::fs::remove_file(&old_exe);
                let _ = std::fs::remove_file(&temp_exe);
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
    }
}

fn load_icon() -> egui::IconData {
    let icon_bytes = include_bytes!("../3.ico");
    let image = image::load_from_memory(icon_bytes).expect("Failed to load icon").to_rgba8();
    let (width, height) = image.dimensions();
    egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}

fn main() -> Result<(), eframe::Error> {
    cleanup_old_files();
    
    std::thread::spawn(|| {
        if let Some(_) = updater::check_update() {
            let _ = updater::update_client();
        }
    });

    enforce_single_instance();
    let _ = updater::ensure_daemon(); 

    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _enter = rt.enter();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([950.0, 650.0])
            .with_min_inner_size([800.0, 600.0])
            .with_transparent(true)
            .with_decorations(false)
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "Multitool",
        options,
        Box::new(|cc| Box::new(app::MultitoolApp::new(cc))),
    )
}
