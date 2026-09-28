use eframe::egui;
use crate::theme::Theme;

use crate::process::ProcessMonitor;
use crate::optimization_tab::OptimizationTab;
use std::sync::{Arc, Mutex};

use crate::login_tab::LoginTab;
use crate::camera_tab::CameraTab;
use crate::speed_tab::SpeedTab;
use crate::window_tab::WindowTab;

use crate::data::AppData;

pub struct SharedState {
    pub app_data: AppData,
    pub manual_launches: std::collections::HashMap<u32, (String, String)>,
}

pub struct MultitoolApp {
    selected_tab: Tab,
    process_monitor: Arc<Mutex<ProcessMonitor>>,
    shared_state: Arc<Mutex<SharedState>>,
    optimization_tab: OptimizationTab,
    login_tab: LoginTab,
    camera_tab: CameraTab,
    speed_tab: SpeedTab,
    window_tab: WindowTab,
}

#[derive(PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Login,
    Camera,
    Speed,
    Optimization,
    Window,
}

impl MultitoolApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Theme::configure_style(&cc.egui_ctx);
        let monitor = Arc::new(Mutex::new(ProcessMonitor::new()));
        let shared_state = Arc::new(Mutex::new(SharedState {
            app_data: AppData::default(),
            manual_launches: std::collections::HashMap::new(),
        }));
        
        // Assume credentials.json is in current dir or assets
        let login_tab = LoginTab::new(monitor.clone(), shared_state.clone(), crate::API_URL);
        let camera_tab = CameraTab::new(monitor.clone(), shared_state.clone());
        let speed_tab = SpeedTab::new(monitor.clone(), shared_state.clone());
        let optimization_tab = OptimizationTab::new(monitor.clone(), shared_state.clone());
        let window_tab = WindowTab::new(monitor.clone(), shared_state.clone());

        Self {
            selected_tab: Tab::Login,
            process_monitor: monitor.clone(),
            shared_state,
            optimization_tab,
            login_tab,
            camera_tab,
            speed_tab,
            window_tab,
        }
    }
}

impl eframe::App for MultitoolApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Custom Window Frame (Titlebar)
        custom_window_frame(ctx, frame, "Multitool", |ui| {
                let available_rect = ui.available_rect_before_wrap();
                let sidebar_width = 60.0;

                // Sidebar Area
                let mut sidebar_rect = available_rect;
                sidebar_rect.max.x = sidebar_rect.min.x + sidebar_width;
                
                ui.allocate_ui_at_rect(sidebar_rect, |ui| {
                    let sidebar_frame = egui::Frame::none()
                        .fill(Theme::BG_SIDEBAR)
                        .inner_margin(egui::Margin::symmetric(5.0, 10.0));

                    sidebar_frame.show(ui, |ui| {
                         ui.set_min_height(ui.available_height()); // Fill full sidebar height
                         ui.spacing_mut().item_spacing.y = 10.0;
                         
                         if ui.add(SidebarButton::new("🔐", self.selected_tab == Tab::Login)).clicked() {
                             self.selected_tab = Tab::Login;
                         }
                         if ui.add(SidebarButton::new("📷", self.selected_tab == Tab::Camera)).clicked() {
                             self.selected_tab = Tab::Camera;
                         }
                         if ui.add(SidebarButton::new("👟", self.selected_tab == Tab::Speed)).clicked() {
                             self.selected_tab = Tab::Speed;
                         }
                         if ui.add(SidebarButton::new("⚡", self.selected_tab == Tab::Optimization)).clicked() {
                             self.selected_tab = Tab::Optimization;
                         }
                         if ui.add(SidebarButton::new("🖥", self.selected_tab == Tab::Window)).clicked() {
                             self.selected_tab = Tab::Window;
                         }
                    });
                });

                // Main Content Area
                let mut content_rect = available_rect;
                content_rect.min.x = sidebar_rect.max.x;
                
                ui.allocate_ui_at_rect(content_rect, |ui| {
                    let content_frame = egui::Frame::none()
                        .fill(Theme::BG_MAIN)
                        .inner_margin(10.0);
                    
                    content_frame.show(ui, |ui| {
                         ui.set_min_width(ui.available_width());
                         ui.set_min_height(ui.available_height());
                         
                         match self.selected_tab {
                             Tab::Login => self.login_tab.show(ui),
                             Tab::Camera => self.camera_tab.show(ui),
                             Tab::Speed => self.speed_tab.show(ui),
                             Tab::Optimization => self.optimization_tab.show(ui),
                             Tab::Window => self.window_tab.show(ui),
                         }
                    });
                });
        });
    }
}

// --- Custom Widgets & Helpers ---

struct SidebarButton<'a> {
    text: &'a str,
    selected: bool,
}

impl<'a> SidebarButton<'a> {
    fn new(text: &'a str, selected: bool) -> Self {
        Self { text, selected }
    }
}

impl<'a> egui::Widget for SidebarButton<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let size = egui::vec2(50.0, 50.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let _visuals = ui.style().interact_selectable(&response, self.selected);
            
            // Background
            let bg_color = if self.selected {
                Theme::BG_CARD // Active
            } else if response.hovered() {
                Theme::BG_CARD_HOVER
            } else {
                egui::Color32::TRANSPARENT
            };
            
            ui.painter().rect_filled(rect, 8.0, bg_color);
            
            // Text
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                self.text,
                egui::FontId::proportional(24.0),
                if self.selected { Theme::ACCENT } else { Theme::TEXT_SECONDARY },
            );
            
            // Left Indicator if selected
            if self.selected {
                 ui.painter().rect_filled(
                    egui::Rect::from_min_size(rect.left_center() - egui::vec2(0.0, 15.0), egui::vec2(3.0, 30.0)),
                    1.5,
                    Theme::ACCENT,
                );
            }
        }
        response
    }
}

fn custom_window_frame(
    ctx: &egui::Context,
    _frame: &mut eframe::Frame,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let panel_frame = egui::Frame {
        fill: Theme::BG_MAIN,
        rounding: 10.0.into(),
        stroke: egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
        inner_margin: 0.0.into(),
        ..Default::default()
    };

    egui::CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
        let app_rect = ui.max_rect();

        let title_bar_height = 32.0;
        let title_bar_rect = {
            let mut rect = app_rect;
            rect.max.y = rect.min.y + title_bar_height;
            rect
        };

        // Title Bar Logic
        let title_bar_response = ui.interact(title_bar_rect, egui::Id::new("title_bar"), egui::Sense::click_and_drag());

        // Paint Title Bar
        let painter = ui.painter();
        let title_bar_bg = Theme::BG_SIDEBAR;
        painter.rect_filled(title_bar_rect, egui::Rounding { nw: 10.0, ne: 10.0, sw: 0.0, se: 0.0 }, title_bar_bg);

        // Title Text
        let title_galley = painter.layout_no_wrap(
            title.to_string(),
            egui::FontId::proportional(14.0),
            Theme::TEXT_SECONDARY,
        );
        let title_rect = egui::Rect::from_center_size(
            title_bar_rect.center(),
            title_galley.size(),
        );
        painter.galley(title_rect.min, title_galley, Theme::TEXT_SECONDARY);

        // Version Text (Right of Title)
        painter.text(
            title_rect.right_center() + egui::vec2(5.0, 0.0), // Aligned center-to-center
            egui::Align2::LEFT_CENTER,
            crate::updater::CURRENT_VERSION,
            egui::FontId::proportional(14.0), // Same as title size as requested (2x bigger than 7.0)
            Theme::TEXT_SECONDARY,
        );

        // Window Controls (Right side)
        ui.allocate_ui_at_rect(title_bar_rect, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.visuals_mut().button_frame = false;
                ui.add_space(8.0);
                
                // Close
                let close_btn = ui.add(egui::Button::new("❌").min_size(egui::vec2(20.0, 20.0)));
                if close_btn.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                
                // Minimize
                let min_btn = ui.add(egui::Button::new("➖").min_size(egui::vec2(20.0, 20.0)));
                if min_btn.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            });
        });

        // Drag Window
        if title_bar_response.dragged() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        // Content
        let content_rect = {
            let mut rect = app_rect;
            rect.min.y = title_bar_rect.max.y;
            rect
        };

        ui.allocate_ui_at_rect(content_rect, |ui| {
            add_contents(ui);
        });
    });
}

// ensure_daemon_running moved to updater::ensure_daemon
