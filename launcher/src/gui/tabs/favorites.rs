use eframe::egui;

use super::super::LauncherApp;

impl LauncherApp {
    pub fn draw_favorites_tab(&mut self, ui: &mut egui::Ui) {
        if self.favorites.items.is_empty() {
            ui.label("No favorites yet. Star tools in the Browse tab.");
            return;
        }

        let fav_names: Vec<String> = self
            .favorites
            .items
            .iter()
            .map(|f| f.tool_name.clone())
            .collect();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for name in &fav_names {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(name);

                        let cached = self
                            .cache_entries
                            .iter()
                            .find(|e| &e.tool_name == name);

                        if let Some(entry) = cached {
                            let ver = entry.version.clone();
                            ui.label(format!("v{} (cached)", ver));
                            self.draw_run_stop_button(ui, name, &ver);
                        } else {
                            ui.label("not cached");
                        }

                        if ui.button("★ Remove").clicked() {
                            self.favorites.remove(name);
                            let _ = self.favorites.save(&self.favorites_path);
                        }
                    });
                });
            }
        });
    }
}
