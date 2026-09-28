use crate::app::SharedState;
use crate::process::{ProcessInfo, ProcessMonitor};
use crate::theme::Theme;
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub struct OptimizationTab {
    monitor: Arc<Mutex<ProcessMonitor>>,
    shared_state: Arc<Mutex<SharedState>>,
    selected_pids: HashSet<u32>,
    priority_by_pid: HashMap<u32, u32>,
}

impl OptimizationTab {
    pub fn new(monitor: Arc<Mutex<ProcessMonitor>>, shared_state: Arc<Mutex<SharedState>>) -> Self {
        Self {
            monitor,
            shared_state,
            selected_pids: HashSet::new(),
            priority_by_pid: HashMap::new(),
        }
    }

    fn target_pids(&self, current_pid: u32) -> Vec<u32> {
        if self.selected_pids.is_empty() {
            vec![current_pid]
        } else {
            self.selected_pids.iter().copied().collect()
        }
    }

    fn all_mask() -> u64 {
        let count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(64);
        if count >= 64 {
            u64::MAX
        } else {
            (1u64 << count) - 1
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
        ui.heading(egui::RichText::new("Оптимизация процессов").size(22.0).color(egui::Color32::from_gray(200)));
        ui.label(
            egui::RichText::new("Управление ядрами CPU и приоритетом для запущенных клиентов.")
                .color(Theme::TEXT_SECONDARY)
                .size(14.0),
        );
        ui.add_space(15.0);

        let processes = { self.monitor.lock().unwrap().refresh() };
        let mut sorted_procs: Vec<ProcessInfo> = processes.values().cloned().collect();
        sorted_procs.sort_by_key(|p| p.pid);

        let all_selected = !sorted_procs.is_empty() && self.selected_pids.len() == sorted_procs.len();

        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::new(egui::RichText::new("Выбрать все").size(14.0)).fill(egui::Color32::from_rgb(60, 60, 60)))
                .clicked()
            {
                if all_selected {
                    self.selected_pids.clear();
                } else {
                    self.selected_pids.clear();
                    for proc in &sorted_procs {
                        self.selected_pids.insert(proc.pid);
                    }
                }
            }

            ui.add_space(10.0);

            if ui.button("Только 0").clicked() {
                let targets: Vec<u32> = if all_selected {
                    self.selected_pids.iter().copied().collect()
                } else {
                    sorted_procs.iter().map(|p| p.pid).collect()
                };
                for pid in targets {
                    self.monitor.lock().unwrap().set_cpu_affinity(pid, 1);
                }
            }

            if ui.button("Все ядра").clicked() {
                let mask = Self::all_mask();
                let targets: Vec<u32> = if all_selected {
                    self.selected_pids.iter().copied().collect()
                } else {
                    sorted_procs.iter().map(|p| p.pid).collect()
                };
                for pid in targets {
                    self.monitor.lock().unwrap().set_cpu_affinity(pid, mask);
                }
            }

            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(format!("Выбрано окон: {}", self.selected_pids.len()))
                    .color(Theme::TEXT_SECONDARY)
                    .size(14.0),
            );
        });
        ui.add_space(10.0);

        let cpu_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(64);

        egui::ScrollArea::vertical().show(ui, |ui| {
            for proc in sorted_procs {
                let display_name = proc.role.as_deref().unwrap_or(&proc.name).to_string();
                let selected = self.selected_pids.contains(&proc.pid);
                let card_bg = if selected {
                    egui::Color32::from_rgb(94, 82, 28)
                } else {
                    egui::Color32::from_rgb(28, 28, 28)
                };

                egui::Frame::none()
                    .fill(card_bg)
                    .rounding(4.0)
                    .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());

                        ui.horizontal(|ui| {
                            let (select_rect, select_resp) = ui.allocate_exact_size(egui::vec2(14.0, 56.0), egui::Sense::click());
                            ui.painter().rect_filled(
                                select_rect.shrink(2.0),
                                3.0,
                                if selected {
                                    egui::Color32::from_rgb(180, 150, 40)
                                } else {
                                    egui::Color32::from_rgb(48, 48, 48)
                                },
                            );
                            if select_resp.clicked() {
                                if selected {
                                    self.selected_pids.remove(&proc.pid);
                                } else {
                                    self.selected_pids.insert(proc.pid);
                                }
                            }

                            ui.add_space(8.0);

                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("[{}] {}", proc.pid, display_name))
                                            .color(egui::Color32::WHITE)
                                            .size(15.0),
                                    );
                                    ui.add_space(8.0);
                                    ui.label(egui::RichText::new("Ядра:").color(Theme::TEXT_SECONDARY).size(14.0));
                                });
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("PID: {} | {}", proc.pid, display_name))
                                            .color(Theme::TEXT_SECONDARY)
                                            .size(12.0),
                                    );
                                    ui.add_space(10.0);

                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                        for i in 0..cpu_count {
                                            let mask = 1u64 << i;
                                            let active = proc.affinity_mask & mask != 0;
                                            let bg = if active {
                                                Theme::STATUS_ONLINE
                                            } else {
                                                egui::Color32::from_rgb(60, 60, 60)
                                            };

                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new(i.to_string())
                                                            .color(egui::Color32::BLACK)
                                                            .size(12.0),
                                                    )
                                                    .fill(bg)
                                                    .rounding(2.0),
                                                )
                                                .clicked()
                                            {
                                                let targets = self.target_pids(proc.pid);
                                                for pid in targets {
                                                    let mask_to_set = if self.selected_pids.is_empty() {
                                                        let mut new_mask = proc.affinity_mask;
                                                        if active {
                                                            new_mask &= !mask;
                                                        } else {
                                                            new_mask |= mask;
                                                        }
                                                        if new_mask == 0 {
                                                            new_mask = mask;
                                                        }
                                                        new_mask
                                                    } else {
                                                        let current_mask = processes
                                                            .get(&pid)
                                                            .map(|p| p.affinity_mask)
                                                            .unwrap_or(proc.affinity_mask);
                                                        let is_active = current_mask & mask != 0;
                                                        let mut new_mask = current_mask;
                                                        if is_active {
                                                            new_mask &= !mask;
                                                        } else {
                                                            new_mask |= mask;
                                                        }
                                                        if new_mask == 0 {
                                                            new_mask = mask;
                                                        }
                                                        new_mask
                                                    };

                                                    self.monitor.lock().unwrap().set_cpu_affinity(pid, mask_to_set);
                                                }
                                            }
                                        }
                                    });
                                });
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("Приоритет").color(Theme::TEXT_SECONDARY).size(12.0));
                                    let current_priority = self.priority_by_pid.get(&proc.pid).copied().unwrap_or(2);
                                    let priority_text = match current_priority {
                                        0 => "Низкий",
                                        3 => "Высокий",
                                        _ => "Средний",
                                    };

                                    egui::ComboBox::from_id_source(("priority", proc.pid))
                                        .selected_text(priority_text)
                                        .width(120.0)
                                        .show_ui(ui, |ui| {
                                            if ui.selectable_label(current_priority == 0, "Низкий").clicked() {
                                                let targets = self.target_pids(proc.pid);
                                                for pid in targets {
                                                    self.priority_by_pid.insert(pid, 0);
                                                    self.monitor.lock().unwrap().set_process_priority(pid, 0);
                                                }
                                            }
                                            if ui.selectable_label(current_priority == 2, "Средний").clicked() {
                                                let targets = self.target_pids(proc.pid);
                                                for pid in targets {
                                                    self.priority_by_pid.insert(pid, 2);
                                                    self.monitor.lock().unwrap().set_process_priority(pid, 2);
                                                }
                                            }
                                            if ui.selectable_label(current_priority == 3, "Высокий").clicked() {
                                                let targets = self.target_pids(proc.pid);
                                                for pid in targets {
                                                    self.priority_by_pid.insert(pid, 3);
                                                    self.monitor.lock().unwrap().set_process_priority(pid, 3);
                                                }
                                            }
                                        });
                                });
                            });
                        });
                    });

                ui.add_space(6.0);
            }
        });
    }
}
