use eframe::egui;
use crate::process::ProcessMonitor;
use crate::theme::Theme;
use crate::app::SharedState;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;

pub struct SpeedTab {
    process_monitor: Arc<Mutex<ProcessMonitor>>,
    shared_state: Arc<Mutex<SharedState>>,
    enabled_pids: Arc<Mutex<HashSet<u32>>>,
    sync_started: bool,
}

impl SpeedTab {
    pub fn new(process_monitor: Arc<Mutex<ProcessMonitor>>, shared_state: Arc<Mutex<SharedState>>) -> Self {
        let enabled_pids = Arc::new(Mutex::new(HashSet::new()));
        Self {
            process_monitor,
            shared_state,
            enabled_pids,
            sync_started: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        if !self.sync_started {
            self.sync_started = true;
            crate::speed::run_speed_sync(self.enabled_pids.clone());
        }

        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));

        ui.heading(egui::RichText::new("Ускорение").size(22.0).color(egui::Color32::from_gray(200)));
        ui.label(egui::RichText::new("Список активных окон для изменения скорости:").color(Theme::TEXT_SECONDARY).size(14.0));
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

        let enabled = self.enabled_pids.lock().unwrap().clone();

        for proc in sorted_procs {
            let is_enabled = enabled.contains(&proc.pid);
            let display_name = proc.role.as_deref().unwrap_or(&proc.name).to_string();

            let fill_color = if is_enabled {
                egui::Color32::from_rgb(33, 33, 33) // slightly lit? The screenshot doesn't show an enabled state clearly, but let's use dark card for off
            } else {
                egui::Color32::from_rgb(33, 33, 33)
            };

            egui::Frame::none()
                .fill(fill_color)
                .rounding(4.0)
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.set_max_width(200.0);
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(2.0, 16.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 1.0, egui::Color32::from_gray(120)); // simple grey indicator
                        ui.add_space(4.0);
                        
                        let text_color = if is_enabled { egui::Color32::WHITE } else { egui::Color32::from_gray(180) };
                        if ui.add(egui::Button::new(egui::RichText::new(&display_name).color(text_color).size(14.0)).frame(false)).clicked() {
                            let mut eps = self.enabled_pids.lock().unwrap();
                            if is_enabled {
                                eps.remove(&proc.pid);
                            } else {
                                eps.insert(proc.pid);
                            }
                        }
                    });
                });
            ui.add_space(6.0);
        }
    }
}
