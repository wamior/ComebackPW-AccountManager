use crate::app::SharedState;
use crate::process::ProcessMonitor;
use crate::theme::Theme;
use eframe::egui;
use std::sync::{Arc, Mutex};

pub struct WindowTab {
    process_monitor: Arc<Mutex<ProcessMonitor>>,
    shared_state: Arc<Mutex<SharedState>>,
}

impl WindowTab {
    pub fn new(process_monitor: Arc<Mutex<ProcessMonitor>>, shared_state: Arc<Mutex<SharedState>>) -> Self {
        Self {
            process_monitor,
            shared_state,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(2));

        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("Исправить расположение окна").size(22.0).color(egui::Color32::from_gray(200)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Исправить все").clicked() {
                    let pids: Vec<u32> = {
                        let mut monitor = self.process_monitor.lock().unwrap();
                        monitor.refresh().keys().copied().collect()
                    };

                    for pid in pids {
                        let monitor = self.process_monitor.lock().unwrap();
                        let _ = monitor.fix_window_position(pid);
                    }
                }
            });
        });
        ui.add_space(15.0);

        let processes = {
            let mut monitor = self.process_monitor.lock().unwrap();
            monitor.refresh()
        };

        if processes.is_empty() {
            return;
        }

        let mut sorted_procs: Vec<_> = processes.values().cloned().collect();
        sorted_procs.sort_by_key(|p| p.pid);

        for proc in sorted_procs {
            let display_name = proc.role.clone().unwrap_or_else(|| proc.name.clone());
            let user_str = proc.user.clone().unwrap_or_else(|| "".to_string());

            egui::Frame::none()
                .fill(egui::Color32::from_rgb(33, 33, 33))
                .rounding(4.0)
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&display_name).color(egui::Color32::WHITE).size(16.0));
                            ui.label(egui::RichText::new(format!("User: {}", user_str)).color(Theme::TEXT_SECONDARY).size(13.0));
                            ui.label(egui::RichText::new(format!("Role: {}", display_name)).color(Theme::TEXT_SECONDARY).size(13.0));
                            ui.label(egui::RichText::new(format!("PID: {}", proc.pid)).color(Theme::TEXT_SECONDARY).size(13.0));
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("OK").color(Theme::STATUS_ONLINE).size(16.0));
                        });
                    });
                });
            ui.add_space(8.0);
        }
    }
}
