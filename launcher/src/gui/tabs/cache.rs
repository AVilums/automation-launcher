use eframe::egui;

use storage::cache::CacheManager;
use domain::format_bytes;

use super::super::LauncherApp;

impl LauncherApp {
    pub fn draw_cache_tab(&mut self, ui: &mut egui::Ui) {
        if ui.button("🔄 Refresh").clicked() {
            self.refresh_cache();
        }

        let total: u64 = self.cache_entries.iter().map(|e| e.size_bytes).sum();
        let limit = self.config.cache.max_size_mb * 1024 * 1024;
        ui.label(format!(
            "Entries: {}  |  Used: {}  |  Limit: {}",
            self.cache_entries.len(),
            format_bytes(total),
            format_bytes(limit)
        ));

        ui.separator();

        if self.cache_entries.is_empty() {
            ui.label("Cache is empty.");
            return;
        }

        let mut to_remove: Option<(String, String)> = None;
        let mut to_run: Option<(String, String)> = None;
        let mut to_stop: Option<(String, String)> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            for entry in &self.cache_entries {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} v{} — {} — {}",
                        entry.tool_name,
                        entry.version,
                        format_bytes(entry.size_bytes),
                        entry.last_used.format("%Y-%m-%d %H:%M")
                    ));
                    if ui.button("🗑").clicked() {
                        to_remove = Some((entry.tool_name.clone(), entry.version.clone()));
                    }
                    if self.is_running(&entry.tool_name, &entry.version) {
                        if ui.button("⏹ Stop").clicked() {
                            to_stop = Some((entry.tool_name.clone(), entry.version.clone()));
                        }
                    } else if ui.button("▶ Run").clicked() {
                        to_run = Some((entry.tool_name.clone(), entry.version.clone()));
                    }
                });
            }
        });

        if let Some((name, version)) = to_stop {
            self.stop_process(&name, &version);
        }

        if let Some((name, version)) = to_run {
            self.trigger_run(&name, &version);
        }

        if let Some((name, version)) = to_remove {
            let cache =
                CacheManager::new(self.config.cache_dir(), self.config.cache.max_size_mb);
            let _ = cache.remove(&name, &version);
            self.refresh_cache();
            self.status_message = format!("Removed {} v{}", name, version);
        }
    }
}
