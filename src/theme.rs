use eframe::egui;

pub struct Theme;

impl Theme {
    pub const BG_MAIN: egui::Color32 = egui::Color32::from_rgb(18, 18, 18);
    pub const BG_SIDEBAR: egui::Color32 = egui::Color32::from_rgb(25, 25, 25);
    pub const BG_CARD: egui::Color32 = egui::Color32::from_rgb(32, 32, 32);
    pub const BG_CARD_HOVER: egui::Color32 = egui::Color32::from_rgb(45, 45, 45);
    
    pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(100, 100, 255); // Indigo/Blueish
    
    pub const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(240, 240, 240);
    pub const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(160, 160, 160);
    pub const DANGER: egui::Color32 = egui::Color32::from_rgb(200, 60, 60);
    pub const STATUS_ONLINE: egui::Color32 = egui::Color32::from_rgb(76, 175, 80);
    pub const STATUS_OFFLINE: egui::Color32 = egui::Color32::from_rgb(100, 100, 100);
    pub const BORDER: egui::Color32 = egui::Color32::from_rgb(60, 60, 60);

    pub fn configure_style(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        
        // Visuals
        style.visuals.dark_mode = true;
        style.visuals.window_fill = Self::BG_MAIN;
        style.visuals.panel_fill = Self::BG_MAIN;
        style.visuals.widgets.noninteractive.bg_fill = Self::BG_CARD;
        
        // Fonts
        let mut fonts = egui::FontDefinitions::default();
        
        // Make a Japanese-capable font primary so symbols like ツ and ＋ render correctly.
        // Segoe UI stays as a fallback for Latin/Cyrillic and the rest of the UI.
        let proportional = fonts.families.entry(egui::FontFamily::Proportional).or_default();

        let mut push_font = |name: &str, path: &str| {
            if let Ok(data) = std::fs::read(path) {
                fonts.font_data.insert(name.to_owned(), egui::FontData::from_owned(data));
                Some(name.to_owned())
            } else {
                None
            }
        };

        let primary = [
            push_font("meiryo", "C:/Windows/Fonts/meiryo.ttc"),
            push_font("meiryo_ui", "C:/Windows/Fonts/meiryo.ttf"),
            push_font("yu_gothic", "C:/Windows/Fonts/yugothic.ttf"),
            push_font("ms_gothic", "C:/Windows/Fonts/msgothic.ttc"),
        ]
        .into_iter()
        .flatten()
        .next();

        if let Some(primary_name) = primary {
            proportional.push(primary_name);
        }

        for fallback in [
            ("segoe_ui", "C:/Windows/Fonts/segoeui.ttf"),
            ("segoe_symbol", "C:/Windows/Fonts/seguisym.ttf"),
            ("segoe_emoji", "C:/Windows/Fonts/seguiemj.ttf"),
            ("arial", "C:/Windows/Fonts/arial.ttf"),
        ] {
            if let Some(name) = push_font(fallback.0, fallback.1) {
                if proportional.iter().all(|f| f != &name) {
                    proportional.push(name);
                }
            }
        }

        if proportional.is_empty() {
            proportional.push("monospace".to_owned());
        }

        ctx.set_fonts(fonts);

        style.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(20.0));
        style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        style.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));

        ctx.set_style(style);
    }
}
