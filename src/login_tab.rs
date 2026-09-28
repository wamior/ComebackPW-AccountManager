use eframe::egui;
use crate::process::ProcessMonitor;
use crate::data::{DataSource, CharacterData};
use crate::launcher::Launcher;
use crate::app::SharedState;
use std::sync::{Arc, Mutex};
use std::collections::{HashSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};

const VOCATIONS: &[&str] = &[
    "Воин", "Маг", "Оборотень", "Друид", "Лучник", "Жрец",
    "Убийца", "Шаман", "Страж", "Мистик", "Призрак", "Жнец"
];

const VOCATION_FILE_MAP: &[(&str, &str)] = &[
    ("воин", "вар"),
    ("маг", "маг"),
    ("оборотень", "танк"),
    ("танк", "танк"),
    ("друид", "дру"),
    ("лучник", "лук"),
    ("лук", "лук"),
    ("жрец", "прист"),
    ("прист", "прист"),
    ("убийца", "син"),
    ("син", "син"),
    ("шаман", "шам"),
    ("страж", "сик"),
    ("сик", "сик"),
    ("мистик", "мист"),
];

#[derive(Clone, Debug, serde::Deserialize)]
pub struct ActiveSession {
    pub hostname: Option<String>,
    pub character_name: Option<String>,
    pub server_name: Option<String>,
    pub pid: Option<u32>, // Added to handle local kills
}

#[derive(Debug, serde::Deserialize)]
struct WsMessage {
    #[serde(rename = "type")]
    msg_type: String,
    data: Option<serde_json::Value>,
}

pub struct LoginTab {
    process_monitor: Arc<Mutex<ProcessMonitor>>,
    shared_state: Arc<Mutex<SharedState>>,
    data_source: DataSource,

    // Environmental
    local_hostname: String,

    // State
    server_order: Arc<Mutex<Vec<String>>>,
    error_message: Arc<Mutex<Option<String>>>,
    loading: Arc<AtomicBool>,

    // UI state
    selected_server: Option<String>,
    selected_category: String,
    search_query: String,

    // Edit / Add state
    edit_state: Option<EditState>,
    show_add_char_modal: bool,
    new_char_form: CharFormData,

    // Category management
    adding_category_inline: bool,
    new_category_name: String,
    rename_cat_modal: Option<String>,
    rename_cat_input: String,

    // Add to category popup
    show_add_to_cat_popup: bool,

    // Confirmation state
    confirm_delete_char: Option<(String, String, bool)>, 
    confirm_delete_cat: Option<(String, String)>,

    // Active
    active_sessions: Arc<Mutex<Vec<ActiveSession>>>,
    local_sessions: Arc<Mutex<Vec<ActiveSession>>>,
    listener_started: Arc<AtomicBool>,
    needs_fetch: Arc<AtomicBool>,
    last_local_scan: std::time::Instant,

    // Icon Cache
    icon_textures: Arc<Mutex<HashMap<String, egui::TextureHandle>>>,
    loading_icons: Arc<Mutex<HashSet<String>>>,
}

#[derive(Clone)]
struct CharFormData {
    login: String,
    password: String,
    character: String,
    vocation: String,
    exe: String,
    use_launcher: bool,
    seed: String,
    window_name: String,
}

impl Default for CharFormData {
    fn default() -> Self {
        Self {
            login: "".to_string(),
            password: "".to_string(),
            character: "".to_string(),
            vocation: "Выбрать...".to_string(),
            exe: "elementclient.exe".to_string(),
            use_launcher: false,
            seed: "".to_string(),
            window_name: "".to_string(),
        }
    }
}

struct EditState {
    server: String,
    category: Option<String>,
    original_char: CharacterData,
    form: CharFormData,
}

impl LoginTab {
    pub fn new(
        process_monitor: Arc<Mutex<ProcessMonitor>>,
        shared_state: Arc<Mutex<SharedState>>,
        backend_url: &str,
    ) -> Self {
        let local_hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let data_source = DataSource::new(backend_url);
        Self {
            process_monitor,
            shared_state,
            data_source,
            local_hostname,
            server_order: Arc::new(Mutex::new(Vec::new())),
            error_message: Arc::new(Mutex::new(None)),
            loading: Arc::new(AtomicBool::new(false)),
            selected_server: None,
            selected_category: "Все".to_string(),
            search_query: String::new(),
            edit_state: None,
            show_add_char_modal: false,
            new_char_form: CharFormData::default(),
            adding_category_inline: false,
            new_category_name: String::new(),
            rename_cat_modal: None,
            rename_cat_input: String::new(),
            show_add_to_cat_popup: false,
            confirm_delete_char: None,
            confirm_delete_cat: None,
            active_sessions: Arc::new(Mutex::new(Vec::new())),
            local_sessions: Arc::new(Mutex::new(Vec::new())),
            listener_started: Arc::new(AtomicBool::new(false)),
            needs_fetch: Arc::new(AtomicBool::new(true)),
            last_local_scan: std::time::Instant::now() - std::time::Duration::from_secs(60),
            icon_textures: Arc::new(Mutex::new(HashMap::new())),
            loading_icons: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    fn update_local_sessions(&mut self) {
        if self.last_local_scan.elapsed().as_secs() < 2 {
            return;
        }
        let mut monitor = self.process_monitor.lock().unwrap();
        let processes = monitor.refresh();
        let mut local = Vec::new();
        for p in processes.values() {
            if let Some(role) = &p.role {
                local.push(ActiveSession {
                    hostname: Some(self.local_hostname.clone()),
                    character_name: Some(role.clone()),
                    server_name: None, // Will match by char name in UI
                    pid: Some(p.pid),
                });
            }
        }
        *self.local_sessions.lock().unwrap() = local;
        self.last_local_scan = std::time::Instant::now();
    }

    fn fetch_data_async(&self, ctx: egui::Context) {
        if self.loading.swap(true, Ordering::SeqCst) {
            return;
        }
        let data_source = self.data_source.clone();
        let shared_state = self.shared_state.clone();
        let server_order = self.server_order.clone();
        let error_message = self.error_message.clone();
        let loading = self.loading.clone();
        let active_sessions = self.active_sessions.clone();

        std::thread::spawn(move || {
            // Fetch characters
            let res = data_source.fetch_data("", "");
            match res {
                Ok((order, data)) => {
                    *server_order.lock().unwrap() = order;
                    let mut state = shared_state.lock().unwrap();
                    state.app_data.servers = data;
                    *error_message.lock().unwrap() = None;
                }
                Err(e) => {
                    *error_message.lock().unwrap() = Some(format!("Ошибка загрузок: {}", e));
                }
            }
            
            // Fetch active sessions as fallback/sync
            if let Ok(sessions) = data_source.fetch_active_sessions() {
                *active_sessions.lock().unwrap() = sessions;
            }

            loading.store(false, Ordering::SeqCst);
            ctx.request_repaint();
        });
    }

    fn start_remote_listener(&self, ctx: egui::Context) {
        if self.listener_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let active_sessions = self.active_sessions.clone();
        let needs_fetch = self.needs_fetch.clone();
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            // FIX: Removed /ws because server usually listens at root for WS upgrades
            let ws_url = "ws://auth.comeback-links.ru/ws";
            loop {
                match tungstenite::connect(ws_url) {
                    Ok((mut socket, _)) => {
                        loop {
                            match socket.read() {
                                Ok(tungstenite::Message::Text(text)) => {
                                    if let Ok(msg) = serde_json::from_str::<WsMessage>(&text) {
                                        match msg.msg_type.as_str() {
                                            "ACTIVE_SESSIONS_UPDATE" => {
                                                if let Some(data) = msg.data {
                                                    if let Ok(sessions) = serde_json::from_value::<Vec<ActiveSession>>(data) {
                                                        *active_sessions.lock().unwrap() = sessions;
                                                        ctx_clone.request_repaint();
                                                    }
                                                }
                                            }
                                            "DATA_UPDATE" => {
                                                needs_fetch.store(true, Ordering::SeqCst);
                                                ctx_clone.request_repaint();
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                Ok(_) => {} 
                                Err(_) => {
                                    break;
                                } 
                            }
                        }
                    }
                    Err(_) => {
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });
    }

    fn get_icon(&self, ui: &mut egui::Ui, vocation: &str) -> Option<egui::TextureHandle> {
        let v_lower = vocation.to_lowercase();
        let mut file_name = None;
        for (key, val) in VOCATION_FILE_MAP {
            if v_lower.contains(key) {
                file_name = Some(*val);
                break;
            }
        }
        let file_name = file_name?;
        let name_str = file_name.to_string();
        {
            let textures = self.icon_textures.lock().unwrap();
            if let Some(handle) = textures.get(&name_str) { return Some(handle.clone()); }
        }
        let mut loading = self.loading_icons.lock().unwrap();
        if loading.contains(&name_str) { return None; }
        loading.insert(name_str.clone());
        let ctx = ui.ctx().clone();
        let icon_textures = self.icon_textures.clone();
        let loading_icons = self.loading_icons.clone();
        let name_clone = name_str.clone();
        std::thread::spawn(move || {
            let url = format!("http://auth.comeback-links.ru/vocation/{}.png", name_clone);
            if let Ok(resp) = reqwest::blocking::get(&url) {
                if let Ok(bytes) = resp.bytes() {
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        let size = [img.width() as usize, img.height() as usize];
                        let rgba = img.to_rgba8();
                        let pixels = rgba.as_flat_samples();
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                        let mut textures = icon_textures.lock().unwrap();
                        let handle = ctx.load_texture(&name_clone, color_image, Default::default());
                        textures.insert(name_clone.clone(), handle);
                        ctx.request_repaint();
                    }
                }
            }
            loading_icons.lock().unwrap().remove(&name_clone);
        });
        None
    }

    fn normalize_session_name(name: &str) -> String {
        name.chars()
            .map(|ch| match ch {
                '\u{00A0}' => ' ',
                _ => ch,
            })
            .collect::<String>()
            .trim()
            .to_lowercase()
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.start_remote_listener(ui.ctx().clone());
        if self.needs_fetch.swap(false, Ordering::SeqCst) {
            self.fetch_data_async(ui.ctx().clone());
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1)); // More frequent for local scan

        self.update_local_sessions();
        let mut active_sessions = self.active_sessions.lock().unwrap().clone();
        
        // Merge local sessions - avoid duplicates
        {
            let local = self.local_sessions.lock().unwrap();
            for ls in local.iter() {
                // If this local character is already in active_sessions (from server), 
                // update it with local info (like PID) instead of adding a duplicate
                if let Some(existing) = active_sessions.iter_mut().find(|s| {
                    Self::normalize_session_name(s.character_name.as_deref().unwrap_or(""))
                        == Self::normalize_session_name(ls.character_name.as_deref().unwrap_or(""))
                }) {
                    existing.pid = ls.pid;
                    existing.hostname = Some(self.local_hostname.clone());
                } else {
                    active_sessions.push(ls.clone());
                }
            }
        }

        // Final dedupe: same character can arrive twice with slightly different source data.
        let mut deduped: Vec<ActiveSession> = Vec::new();
        for session in active_sessions.into_iter() {
            let key_name = Self::normalize_session_name(session.character_name.as_deref().unwrap_or(""));
            let key_host = session.hostname.as_deref().unwrap_or("").trim().to_lowercase();

            if let Some(existing) = deduped.iter_mut().find(|s| {
                Self::normalize_session_name(s.character_name.as_deref().unwrap_or("")) == key_name
                    && s.hostname.as_deref().unwrap_or("").trim().to_lowercase() == key_host
            }) {
                if existing.pid.is_none() {
                    existing.pid = session.pid;
                }
                if existing.server_name.is_none() || existing.server_name.as_deref() == Some("Unknown") {
                    existing.server_name = session.server_name.clone();
                }
            } else {
                deduped.push(session);
            }
        }
        active_sessions = deduped;

        // STABLE SORTING to prevent jumping
        active_sessions.sort_by(|a, b| {
            let host_a = a.hostname.as_deref().unwrap_or("").to_lowercase();
            let host_b = b.hostname.as_deref().unwrap_or("").to_lowercase();
            
            // Prioritize local machine
            let is_local_a = host_a == self.local_hostname.to_lowercase();
            let is_local_b = host_b == self.local_hostname.to_lowercase();
            
            if is_local_a != is_local_b {
                return is_local_b.cmp(&is_local_a); // Local first
            }
            
            // Then by character name
            let name_a = a.character_name.as_deref().unwrap_or("").to_lowercase();
            let name_b = b.character_name.as_deref().unwrap_or("").to_lowercase();
            name_a.cmp(&name_b)
        });

        let app_data = self.shared_state.lock().unwrap().app_data.clone();
        let server_order = self.server_order.lock().unwrap().clone();
        let error_message = self.error_message.lock().unwrap().clone();
        let loading = self.loading.load(Ordering::SeqCst);

        #[cfg(debug_assertions)]
        {
            if let Some(srv) = &self.selected_server {
                if let Some(cats_map) = app_data.servers.get(srv) {
                    for (cat_name, chars) in cats_map {
                        for c in chars {
                            println!("[LoginTab][DBG] srv={} cat={} char={:?}", srv, cat_name, c.character);
                        }
                    }
                }
            }
        }

        if self.selected_server.is_none() && !server_order.is_empty() {
            if server_order.iter().any(|s| s == "Main") {
                self.selected_server = Some("Main".to_string());
            } else {
                self.selected_server = Some(server_order[0].clone());
            }
        }

        self.show_modals(ui, &app_data);

        // --- HEADER ---
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("Персонажи").size(22.0).color(egui::Color32::from_gray(200)));
            if loading { ui.spinner(); }
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for srv in server_order.iter().rev() {
                    let is_selected = self.selected_server.as_ref() == Some(srv);
                    let fill = if is_selected { egui::Color32::from_rgb(0, 100, 150) } else { egui::Color32::from_rgb(45, 45, 45) };
                    if ui.add(egui::Button::new(srv).fill(fill).rounding(2.0)).clicked() {
                        self.selected_server = Some(srv.clone());
                        self.selected_category = "Все".to_string();
                    }
                }
                if ui.button("🔄").clicked() { self.fetch_data_async(ui.ctx().clone()); }
            });
        });
        if let Some(err) = error_message { ui.label(egui::RichText::new(err).color(egui::Color32::LIGHT_RED)); }
        ui.add_space(8.0);

        // --- CATEGORIES ---
        ui.horizontal(|ui| {
            let mut categories_list = vec!["Все".to_string()];
            if let Some(srv) = &self.selected_server {
                if let Some(cats_map) = app_data.servers.get(srv) {
                    for cat in cats_map.keys() {
                        if cat != "__GLOBAL__" { categories_list.push(cat.clone()); }
                    }
                }
            }

            for cat in &categories_list {
                let is_selected = self.selected_category == *cat;
                let color = if is_selected { egui::Color32::from_rgb(60, 150, 255) } else { egui::Color32::from_gray(180) };
                if ui.add(egui::Button::new(egui::RichText::new(cat).color(color).size(14.0)).frame(false)).clicked() {
                    self.selected_category = cat.clone();
                }
            }

            if self.selected_category != "Все" {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("✏")).clicked() {
                        self.rename_cat_modal = Some(self.selected_category.clone());
                        self.rename_cat_input = self.selected_category.clone();
                    }
                    if ui.add(egui::Button::new("❌")).clicked() {
                        if let Some(srv) = &self.selected_server {
                            self.confirm_delete_cat = Some((srv.clone(), self.selected_category.clone()));
                        }
                    }
                    let cat_list_without_all: Vec<String> = categories_list.iter().skip(1).cloned().collect();
                    if let Some(pos) = cat_list_without_all.iter().position(|c| c == &self.selected_category) {
                        if pos > 0 { if ui.add(egui::Button::new("←")).clicked() { self.move_category(pos, pos - 1); } }
                        if pos < cat_list_without_all.len() - 1 { if ui.add(egui::Button::new("→")).clicked() { self.move_category(pos, pos + 1); } }
                    }
                });
            }

            if self.adding_category_inline {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.new_category_name).hint_text("Новая категория").desired_width(120.0));
                    if ui.button("✔").clicked() {
                        if !self.new_category_name.is_empty() {
                            if let Some(srv) = &self.selected_server {
                                let _ = self.data_source.create_category(srv, &self.new_category_name);
                                self.fetch_data_async(ui.ctx().clone());
                            }
                        }
                        self.adding_category_inline = false;
                        self.new_category_name = String::new();
                    }
                    if ui.button("✖").clicked() { self.adding_category_inline = false; self.new_category_name = String::new(); }
                });
            } else if ui.add(egui::Button::new("+").fill(egui::Color32::from_rgb(45, 45, 45))).clicked() {
                self.adding_category_inline = true;
            }
        });
        ui.add_space(8.0);

        // --- SEARCH ---
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(egui::TextEdit::singleline(&mut self.search_query).desired_width(220.0));
        });
        ui.add_space(16.0);

        // --- TWO COLUMNS ---
        ui.columns(2, |cols| {
            // == LEFT COLUMN (ДОСТУПНЫЕ) ==
            cols[0].horizontal(|ui| {
                ui.heading(egui::RichText::new("ДОСТУПНЫЕ").color(egui::Color32::from_gray(180)).size(16.0));
                if ui.add(egui::Button::new("+").fill(egui::Color32::from_rgb(50, 50, 50))).clicked() {
                    if self.selected_category == "Все" { self.show_add_char_modal = true; self.new_char_form = CharFormData::default(); }
                    else { self.show_add_to_cat_popup = true; }
                }
            });
            cols[0].add_space(10.0);

            let mut display_chars = Vec::new();
            if let Some(srv) = &self.selected_server {
                if let Some(cats_map) = app_data.servers.get(srv) {
                    if self.selected_category == "Все" {
                        if let Some(global_chars) = cats_map.get("__GLOBAL__") {
                            display_chars = global_chars.clone();
                        } else {
                            let mut unique = std::collections::HashMap::new();
                            for chars in cats_map.values() {
                                for c in chars {
                                    unique.insert(Self::normalize_session_name(&c.character), c.clone());
                                }
                            }
                            display_chars = unique.into_values().collect();
                        }
                        display_chars.sort_by(|a, b| a.character.cmp(&b.character));
                    } else if let Some(chars) = cats_map.get(&self.selected_category) {
                        display_chars = chars.clone();
                    }
                }
            }
            let filtered: Vec<CharacterData> = display_chars.into_iter().filter(|c| {
                // Search filter
                if !self.search_query.is_empty() {
                    let q = self.search_query.to_lowercase();
                    if !c.character.to_lowercase().contains(&q) && !c.login.to_lowercase().contains(&q) {
                        return false;
                    }
                }
                // Hide if active only on the selected server, never by name across all servers.
                let is_active = active_sessions.iter().any(|s| {
                    let session_name = Self::normalize_session_name(s.character_name.as_deref().unwrap_or(""));
                    let char_name = Self::normalize_session_name(&c.character);
                    let session_server = s.server_name.as_deref().unwrap_or("").trim().to_lowercase();
                    let selected_server = self.selected_server.as_deref().unwrap_or("").trim().to_lowercase();
                    let server_match = if session_server.is_empty() || session_server == "unknown" {
                        if let Some(srv_data) = app_data.servers.get(self.selected_server.as_deref().unwrap_or("")) {
                            srv_data.values().any(|chars| chars.iter().any(|x| Self::normalize_session_name(&x.character) == char_name))
                        } else {
                            false
                        }
                    } else {
                        session_server == selected_server
                    };
                    session_name == char_name && server_match
                });
                !is_active
            }).collect();

            egui::ScrollArea::vertical().id_source("left_scroll").show(&mut cols[0], |ui| {
                for char_data in &filtered {
                    let session_info = active_sessions.iter().find(|s| {
                        Self::normalize_session_name(s.character_name.as_deref().unwrap_or(""))
                            == Self::normalize_session_name(&char_data.character)
                    });
                    let is_active = session_info.is_some();
                    let is_local = session_info.map(|s| {
                        s.hostname.as_deref().unwrap_or("").to_lowercase() == self.local_hostname.to_lowercase()
                    }).unwrap_or(false);

                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(33, 33, 33))
                        .rounding(4.0)
                        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let stripe_color = if is_active { if is_local { egui::Color32::from_rgb(60, 180, 60) } else { egui::Color32::from_rgb(60, 60, 180) } } else { egui::Color32::from_gray(120) };
                                let (rect, _) = ui.allocate_exact_size(egui::vec2(2.0, 16.0), egui::Sense::hover());
                                ui.painter().rect_filled(rect, 1.0, stripe_color);
                                ui.add_space(4.0);

                                if ui.add(egui::Button::new(egui::RichText::new("❌").color(egui::Color32::from_rgb(180, 50, 50)).size(10.0)).frame(false)).clicked() {
                                    if let Some(srv) = &self.selected_server {
                                        self.confirm_delete_char = Some((srv.clone(), char_data.character.clone(), self.selected_category == "Все"));
                                    }
                                }
                                if ui.add(egui::Button::new(egui::RichText::new("✏").color(egui::Color32::GRAY).size(10.0)).frame(false)).clicked() {
                                    if let Some(srv) = &self.selected_server {
                                        let cat = if self.selected_category == "Все" { None } else { Some(self.selected_category.clone()) };
                                        self.edit_state = Some(EditState {
                                            server: srv.clone(), 
                                            category: cat,
                                            original_char: char_data.clone(),
                                            form: CharFormData {
                                                login: char_data.login.clone(), password: char_data.password.clone(),
                                                character: char_data.character.clone(), vocation: char_data.vocation.clone(),
                                                exe: char_data.exe.clone(), use_launcher: char_data.use_launcher,
                                                seed: char_data.seed.clone(), window_name: char_data.window_name.clone(),
                                            },
                                        });
                                    }
                                }
                                
                                if let Some(tex) = self.get_icon(ui, &char_data.vocation) { ui.add(egui::Image::new(&tex).max_size(egui::vec2(18.0, 18.0))); }
                                else { ui.label("👤"); }
                                ui.label(egui::RichText::new(&char_data.character).color(egui::Color32::WHITE).size(14.0));

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let btn = egui::Button::new(egui::RichText::new("▶").size(14.0)).fill(egui::Color32::from_rgb(55, 55, 55));
                                    let has_local_pid = self.local_sessions.lock().unwrap().iter().any(|s| {
                                        s.pid.is_some()
                                            && s.character_name.as_deref().unwrap_or("").trim().eq_ignore_ascii_case(char_data.character.trim())
                                    });
                                    let can_launch = !has_local_pid;
                                    if ui.add_enabled(can_launch, btn).clicked() {
                                        let srv = self.selected_server.as_deref().unwrap_or("Main").trim();
                                        // Launcher & Seed ONLY work in specific categories
                                        let use_launcher = char_data.use_launcher && self.selected_category != "Все";
                                        let seed = if self.selected_category == "Все" { "" } else { &char_data.seed };
                                        let _ = Launcher::launch(&char_data.exe, &char_data.login, &char_data.password, &char_data.character, seed, use_launcher, srv);
                                    }
                                });
                            });
                        });
                    ui.add_space(6.0);
                }
            });

            // == RIGHT COLUMN (АКТИВНЫЕ) ==
            cols[1].heading(egui::RichText::new("АКТИВНЫЕ").color(egui::Color32::from_gray(180)).size(16.0));
            cols[1].add_space(10.0);
            egui::ScrollArea::vertical().id_source("right_scroll").show(&mut cols[1], |ui| {
                for s in &active_sessions {
                    let char_name = s.character_name.as_deref().unwrap_or("Unknown").trim();
                    let srv_name = s.server_name.as_deref().unwrap_or("Unknown").trim();
                    
                    // If a server is selected, only show characters from that server
                    if let Some(selected_srv) = &self.selected_server {
                        let selected_srv = selected_srv.trim();
                        let belongs_to_srv = if srv_name.eq_ignore_ascii_case("unknown") {
                            if let Some(srv_data) = app_data.servers.get(selected_srv) {
                                let normalized_char = Self::normalize_session_name(char_name);
                                srv_data.values().any(|chars| chars.iter().any(|c| Self::normalize_session_name(&c.character) == normalized_char))
                            } else {
                                false
                            }
                        } else {
                            srv_name.to_lowercase() == selected_srv.to_lowercase()
                        };
                        if !belongs_to_srv { continue; }

                        // Additionally filter by current category if not "Все"
                        if self.selected_category != "Все" {
                            let mut belongs_to_cat = false;
                            if let Some(srv_data) = app_data.servers.get(selected_srv) {
                                if let Some(chars) = srv_data.get(&self.selected_category) {
                                    let normalized_char = Self::normalize_session_name(char_name);
                                    if chars.iter().any(|c| Self::normalize_session_name(&c.character) == normalized_char) {
                                        belongs_to_cat = true;
                                    }
                                }
                            }
                            if !belongs_to_cat { continue; }
                        }
                    }

                    let hostname = s.hostname.as_deref().unwrap_or("Unknown").trim();
                    let is_local = hostname.to_lowercase() == self.local_hostname.to_lowercase();
                    ui.push_id(format!("{}_{}", hostname, char_name), |ui| {
                        let mut kill_clicked = false;
                        let frame_res = egui::Frame::none()
                            .fill(egui::Color32::from_rgb(20, 25, 20))
                            .rounding(2.0)
                            .stroke(egui::Stroke::new(1.0, if is_local { egui::Color32::from_rgb(60, 180, 60) } else { egui::Color32::from_rgb(60, 60, 180) }))
                            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let stripe_color = if is_local { egui::Color32::from_rgb(60, 180, 60) } else { egui::Color32::from_rgb(60, 60, 180) };
                                    let (rect, _) = ui.allocate_exact_size(egui::vec2(2.0, 16.0), egui::Sense::hover());
                                    ui.painter().rect_filled(rect, 1.0, stripe_color);
                                    ui.add_space(4.0);
                                    
                                    let mut vocation = String::new();
                                    for (_s_name, srv_data) in &app_data.servers {
                                        for cat_chars in srv_data.values() {
                                            if let Some(c) = cat_chars.iter().find(|c| c.character.to_lowercase().trim() == char_name.to_lowercase()) {
                                                vocation = c.vocation.clone();
                                                break;
                                            }
                                        }
                                        if !vocation.is_empty() { break; }
                                    }

                                    if !vocation.is_empty() {
                                        if let Some(tex) = self.get_icon(ui, &vocation) {
                                            ui.add(egui::Image::new(&tex).max_size(egui::vec2(16.0, 16.0)));
                                        } else {
                                            ui.label("👤");
                                        }
                                    } else {
                                        ui.label("👤");
                                    }
                                    ui.label(egui::RichText::new(char_name).color(egui::Color32::WHITE).size(14.0));
                                    
                                    if !is_local {
                                        ui.label(egui::RichText::new(format!("[{}]", hostname)).color(egui::Color32::from_gray(140)).size(12.0));
                                    }

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add_enabled(is_local, egui::Button::new(egui::RichText::new("❌").color(egui::Color32::from_rgb(180, 50, 50)).size(12.0)).frame(false)).clicked() {
                                            kill_clicked = true;
                                            if let Some(pid) = s.pid {
                                                let _ = self.process_monitor.lock().unwrap().kill_process(pid);
                                            }
                                        }
                                    });
                                });
                            });

                        let interact_res = ui.interact(frame_res.response.rect, ui.id().with(s.pid), egui::Sense::click());
                        if interact_res.clicked() && is_local && !kill_clicked {
                            if let Some(pid) = s.pid {
                                self.process_monitor.lock().unwrap().switch_to_window(pid);
                            }
                        }
                        if interact_res.hovered() {
                            ui.painter().rect_stroke(frame_res.response.rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_gray(100)));
                        }
                    });
                    ui.add_space(6.0);
                }
            });
        });

        if self.show_add_to_cat_popup {
            egui::Window::new(format!("Добавить во '{}'", self.selected_category)).collapsible(false).show(ui.ctx(), |ui| {
               ui.set_width(320.0);
               ui.horizontal(|ui| { ui.add_space(ui.available_width() - 24.0); if ui.button("✖").clicked() { self.show_add_to_cat_popup = false; } });
               let mut all_chars = Vec::new();
               if let Some(srv) = &self.selected_server {
                   if let Some(cats_map) = app_data.servers.get(srv) {
                       let mut unique = std::collections::HashMap::new();
                       if let Some(global_chars) = cats_map.get("__GLOBAL__") {
                           for c in global_chars {
                               unique.insert(Self::normalize_session_name(&c.character), c.clone());
                           }
                       } else {
                           for chars in cats_map.values() {
                               for c in chars {
                                   unique.insert(Self::normalize_session_name(&c.character), c.clone());
                               }
                           }
                       }
                       if let Some(current_chars) = cats_map.get(&self.selected_category) {
                           let current_names: HashSet<String> = current_chars.iter().map(|c| Self::normalize_session_name(&c.character)).collect();
                           all_chars = unique.into_values().filter(|c| !current_names.contains(&Self::normalize_session_name(&c.character))).collect();
                       } else { all_chars = unique.into_values().collect(); }
                       all_chars.sort_by(|a, b| a.character.cmp(&b.character));
                   }
               }
               egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                   for c in all_chars {
                       ui.horizontal(|ui| {
                           if let Some(tex) = self.get_icon(ui, &c.vocation) { ui.add(egui::Image::new(&tex).max_size(egui::vec2(16.0, 16.0))); }
                           else { ui.label("👤"); }
                           ui.label(&c.character);
                           ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                               if ui.button("+").clicked() {
                                   if let Some(srv) = &self.selected_server {
                                       let _ = self.data_source.add_to_category(srv, &self.selected_category, &c.character);
                                       self.fetch_data_async(ui.ctx().clone());
                                   }
                               }
                           });
                       });
                       ui.separator();
                   }
               });
            });
        }
    }

    fn move_category(&self, old_idx: usize, new_idx: usize) {
        if let Some(srv) = &self.selected_server {
            let state = self.shared_state.lock().unwrap();
            if let Some(cats_map) = state.app_data.servers.get(srv) {
                let mut order: Vec<String> = cats_map.keys().filter(|k| *k != "__GLOBAL__").cloned().collect();
                if old_idx < order.len() && new_idx < order.len() {
                    let item = order.remove(old_idx);
                    order.insert(new_idx, item);
                    let _ = self.data_source.reorder_categories(srv, order);
                }
            }
        }
    }

    fn show_modals(&mut self, ui: &mut egui::Ui, _app_data: &crate::data::AppData) {
        let mut save_payload = None;
        let mut cancel_edit = false;

        if self.show_add_char_modal {
            let mut close_modal = false;
            egui::Window::new("Новый персонаж").collapsible(false).show(ui.ctx(), |ui| {
                ui.horizontal(|ui| { ui.label("▼"); ui.heading("Новый персонаж"); ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| { if ui.button("✖").clicked() { close_modal = true; } }); });
                ui.separator();
                egui::Grid::new("add_char_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    ui.label("Ник:"); ui.text_edit_singleline(&mut self.new_char_form.character); ui.end_row();
                    ui.label("Логин:"); ui.text_edit_singleline(&mut self.new_char_form.login); ui.end_row();
                    ui.label("Пароль:"); ui.text_edit_singleline(&mut self.new_char_form.password); ui.end_row();
                    ui.label("Файл:"); ui.text_edit_singleline(&mut self.new_char_form.exe); ui.end_row();
                    ui.label("Профа:");
                    egui::ComboBox::from_id_source("vocation_select").selected_text(&self.new_char_form.vocation).show_ui(ui, |ui| {
                        for v in VOCATIONS { ui.selectable_value(&mut self.new_char_form.vocation, v.to_string(), *v); }
                    });
                    ui.end_row();
                });
                /* Launcher/Seed ONLY in categories, and show_add_char_modal is only for 'Все' */
                /*
                ui.checkbox(&mut self.new_char_form.use_launcher, "Использовать Launcher");
                if self.new_char_form.use_launcher {
                    ui.horizontal(|ui| { ui.label("Seed:"); ui.text_edit_singleline(&mut self.new_char_form.seed); });
                }
                */
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Сохранить").clicked() {
                        if let Some(srv) = &self.selected_server {
                            let data = CharacterData {
                                login: self.new_char_form.login.clone(), character: self.new_char_form.character.clone(),
                                password: self.new_char_form.password.clone(), exe: self.new_char_form.exe.clone(),
                                vocation: self.new_char_form.vocation.clone(), use_launcher: self.new_char_form.use_launcher,
                                seed: self.new_char_form.seed.clone(), window_name: self.new_char_form.window_name.clone(),
                                categories: vec![],
                            };
                            save_payload = Some((srv.clone(), data, None));
                            close_modal = true;
                        }
                    }
                    if ui.button("Отмена").clicked() { close_modal = true; }
                });
            });
            if close_modal { self.show_add_char_modal = false; }
        }

        if let Some(edit) = &mut self.edit_state {
            let is_all = edit.category.is_none();
            egui::Window::new("Редактировать").collapsible(false).show(ui.ctx(), |ui| {
                egui::Grid::new("edit_char_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    ui.label("Ник:"); ui.text_edit_singleline(&mut edit.form.character); ui.end_row();
                    ui.label("Логин:"); ui.text_edit_singleline(&mut edit.form.login); ui.end_row();
                    ui.label("Пароль:"); ui.text_edit_singleline(&mut edit.form.password); ui.end_row();
                    ui.label("Профа:");
                    egui::ComboBox::from_id_source("edit_vocation_select").selected_text(&edit.form.vocation).show_ui(ui, |ui| {
                        for v in VOCATIONS { ui.selectable_value(&mut edit.form.vocation, v.to_string(), *v); }
                    });
                    ui.end_row();
                    ui.label("Файл:");
                    ui.text_edit_singleline(&mut edit.form.exe);
                    ui.end_row();
                });
                
                if !is_all {
                    ui.checkbox(&mut edit.form.use_launcher, "Использовать Launcher");
                    if edit.form.use_launcher { ui.horizontal(|ui| { ui.label("Seed:"); ui.text_edit_singleline(&mut edit.form.seed); }); }
                }

                ui.horizontal(|ui| {
                    if ui.button("Сохранить").clicked() {
                        let mut data = edit.original_char.clone();
                        data.login = edit.form.login.clone(); data.password = edit.form.password.clone();
                        data.character = edit.form.character.clone(); data.vocation = edit.form.vocation.clone();
                        data.exe = edit.form.exe.clone();
                        
                        if !is_all {
                            data.use_launcher = edit.form.use_launcher; data.seed = edit.form.seed.clone();
                        }
                        
                        save_payload = Some((edit.server.clone(), data, edit.category.clone()));
                    }
                    if ui.button("Отмена").clicked() { cancel_edit = true; }
                });
            });
        }
        if cancel_edit { self.edit_state = None; }
        if let Some((srv, data, cat)) = save_payload {
            let _ = self.data_source.save_character(&srv, &data, cat.as_deref());
            self.edit_state = None;
            self.fetch_data_async(ui.ctx().clone());
        }

        if let Some(old_name) = self.rename_cat_modal.clone() {
            egui::Window::new("Переименовать категорию").show(ui.ctx(), |ui| {
                ui.text_edit_singleline(&mut self.rename_cat_input);
                ui.horizontal(|ui| {
                    if ui.button("Переименовать").clicked() {
                        if let Some(srv) = &self.selected_server {
                            let _ = self.data_source.rename_category(srv, &old_name, &self.rename_cat_input);
                            self.selected_category = self.rename_cat_input.clone();
                            self.fetch_data_async(ui.ctx().clone());
                        }
                        self.rename_cat_modal = None;
                    }
                    if ui.button("Отмена").clicked() { self.rename_cat_modal = None; }
                });
            });
        }

        if let Some((srv, name, from_all)) = self.confirm_delete_char.clone() {
            egui::Window::new("Подтверждение").show(ui.ctx(), |ui| {
                ui.label(format!("Вы уверены, что хотите удалить {}?", name));
                ui.horizontal(|ui| {
                    if ui.button("Да").clicked() {
                        if from_all { let _ = self.data_source.delete_character(&srv, &name); }
                        else { let _ = self.data_source.remove_from_category(&srv, &self.selected_category, &name); }
                        self.confirm_delete_char = None;
                        self.fetch_data_async(ui.ctx().clone());
                    }
                    if ui.button("Нет").clicked() { self.confirm_delete_char = None; }
                });
            });
        }

        if let Some((srv, name)) = self.confirm_delete_cat.clone() {
            egui::Window::new("Удалить категорию?").show(ui.ctx(), |ui| {
                ui.label(format!("Вы уверены, что хотите удалить категорию '{}'?", name));
                ui.horizontal(|ui| {
                    if ui.button("Да").clicked() {
                        let _ = self.data_source.delete_category(&srv, &name);
                        self.selected_category = "Все".to_string();
                        self.confirm_delete_cat = None;
                        self.fetch_data_async(ui.ctx().clone());
                    }
                    if ui.button("Нет").clicked() { self.confirm_delete_cat = None; }
                });
            });
        }
    }
}
