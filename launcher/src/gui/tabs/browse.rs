use eframe::egui;

use crate::artifact::Artifact;

use super::super::LauncherApp;

impl LauncherApp {
    pub fn draw_browse_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.search_query);
            if ui.button("🔄 Refresh").clicked() {
                self.refresh_manifest();
            }
        });

        ui.separator();

        let artifacts: Vec<Artifact> = match &self.manifest {
            Some(m) => {
                if self.search_query.is_empty() {
                    m.artifacts.clone()
                } else {
                    m.search(&self.search_query)
                        .into_iter()
                        .cloned()
                        .collect()
                }
            }
            None => Vec::new(),
        };

        if artifacts.is_empty() {
            if self.manifest.is_none() {
                ui.label("Click Refresh to load artifacts.");
            } else {
                ui.label("No artifacts found.");
            }
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for artifact in &artifacts {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(&artifact.name);
                        if let Some(v) = artifact.latest_version() {
                            ui.label(format!("v{}", v.version));
                        }
                        if !artifact.tags.is_empty() {
                            ui.label(format!("[{}]", artifact.tags.join(", ")));
                        }
                    });
                    ui.label(&artifact.description);

                    ui.horizontal(|ui| {
                        if let Some(ver) = artifact.latest_version() {
                            let is_cached = self.cache_entries.iter().any(|e| {
                                e.tool_name == artifact.name && e.version == ver.version
                            });

                            if is_cached {
                                self.draw_run_stop_button(ui, &artifact.name, &ver.version);
                            } else if ui.button("⬇ Download").clicked() {
                                let name = artifact.name.clone();
                                let v = ver.version.clone();
                                self.trigger_download(&name, &v);
                            }
                        }

                        let is_fav = self.favorites.contains(&artifact.name);
                        let fav_label = if is_fav { "★" } else { "☆" };
                        if ui.button(fav_label).clicked() {
                            if is_fav {
                                self.favorites.remove(&artifact.name);
                            } else {
                                self.favorites.add(&artifact.name, None);
                            }
                            let _ = self.favorites.save(&self.favorites_path);
                        }
                    });
                });
            }
        });
    }
}
