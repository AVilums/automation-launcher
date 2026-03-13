use eframe::egui;

use config::LauncherConfig;

use super::super::LauncherApp;

impl LauncherApp {
    pub fn draw_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Cache size (MB):");
            ui.text_edit_singleline(&mut self.settings_cache_mb);
        });

        ui.checkbox(&mut self.settings_telemetry, "Enable telemetry");
        ui.checkbox(&mut self.settings_offline, "Offline mode");

        ui.separator();

        ui.label(format!(
            "Config file: {}",
            self.config.base_dir.join("config").join("launcher.json").display()
        ));
        ui.label(format!("Provider: {:?}", self.config.provider));
        ui.label(format!("Auth: {:?}", self.config.auth));

        ui.separator();

        if ui.button("💾 Save Settings").clicked() {
            if let Ok(mb) = self.settings_cache_mb.parse::<u64>() {
                self.config.cache.max_size_mb = mb;
            }
            self.config.telemetry.enabled = self.settings_telemetry;
            self.config.offline_mode = self.settings_offline;

            match self.config.save() {
                Ok(()) => self.status_message = "Settings saved".into(),
                Err(e) => self.status_message = format!("Failed to save: {}", e),
            }
        }

        if ui.button("🔄 Reset to Defaults").clicked() {
            match LauncherConfig::load() {
                Ok(c) => {
                    self.config = c;
                    self.settings_cache_mb = self.config.cache.max_size_mb.to_string();
                    self.settings_telemetry = self.config.telemetry.enabled;
                    self.settings_offline = self.config.offline_mode;
                    self.status_message = "Settings reset".into();
                }
                Err(e) => self.status_message = format!("Reset failed: {}", e),
            }
        }
    }
}
