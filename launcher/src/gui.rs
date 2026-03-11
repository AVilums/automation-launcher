use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::artifact::{Artifact, ArtifactManifest};
use crate::commands;
use crate::config::LauncherConfig;
use crate::services::cache::{CacheEntry, CacheManager};
use crate::services::favorites::Favorites;
use crate::util::format_bytes;

enum BgMessage {
    ManifestLoaded(ArtifactManifest),
    Error(String),
    DownloadComplete(String, String),
}

struct SharedState {
    messages: Vec<BgMessage>,
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Browse,
    Favorites,
    Cache,
    Settings,
}

pub struct LauncherApp {
    config: LauncherConfig,
    manifest: Option<ArtifactManifest>,
    cache_entries: Vec<CacheEntry>,
    favorites: Favorites,
    favorites_path: PathBuf,
    search_query: String,
    status_message: String,
    current_tab: Tab,
    shared: Arc<Mutex<SharedState>>,
    runtime: tokio::runtime::Handle,
    loading: bool,
    running_processes: HashMap<String, u32>,
    settings_cache_mb: String,
    settings_telemetry: bool,
    settings_offline: bool,
}

impl LauncherApp {
    pub fn new(config: LauncherConfig, runtime: tokio::runtime::Handle) -> Self {
        let favorites_path = Favorites::file_path(&config.base_dir);
        let favorites = Favorites::load(&favorites_path).unwrap_or_default();

        let cache = CacheManager::new(config.cache_dir(), config.cache.max_size_mb);
        let _ = cache.init();
        let cache_entries = cache.list_entries().unwrap_or_default();

        let settings_cache_mb = config.cache.max_size_mb.to_string();
        let settings_telemetry = config.telemetry.enabled;
        let settings_offline = config.offline_mode;

        Self {
            config,
            manifest: None,
            cache_entries,
            favorites,
            favorites_path,
            search_query: String::new(),
            status_message: "Ready".into(),
            current_tab: Tab::Browse,
            shared: Arc::new(Mutex::new(SharedState {
                messages: Vec::new(),
            })),
            runtime,
            loading: false,
            running_processes: HashMap::new(),
            settings_cache_mb,
            settings_telemetry,
            settings_offline,
        }
    }

    fn refresh_manifest(&mut self) {
        if self.loading {
            return;
        }

        if self.config.offline_mode {
            let path = self.config.metadata_dir().join("manifest.json");
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(m) = serde_json::from_str::<ArtifactManifest>(&content) {
                        self.manifest = Some(m);
                        self.status_message = "Loaded cached manifest (offline)".into();
                    }
                }
            }
            return;
        }

        self.loading = true;
        self.status_message = "Fetching manifest...".into();

        let shared = self.shared.clone();
        let config = self.config.clone();

        self.runtime.spawn(async move {
            let result = commands::fetch_manifest(&config).await;
            if let Ok(mut state) = shared.lock() {
                match result {
                    Ok(manifest) => state.messages.push(BgMessage::ManifestLoaded(manifest)),
                    Err(e) => state.messages.push(BgMessage::Error(e.to_string())),
                }
            }
        });
    }

    fn trigger_download(&mut self, tool_name: &str, version: &str) {
        if self.loading {
            return;
        }

        self.loading = true;
        self.status_message = format!("Downloading {} v{}...", tool_name, version);

        let shared = self.shared.clone();
        let config = self.config.clone();
        let name = tool_name.to_string();
        let ver = version.to_string();

        self.runtime.spawn(async move {
            let manifest = match commands::fetch_manifest(&config).await {
                Ok(m) => m,
                Err(e) => {
                    if let Ok(mut state) = shared.lock() {
                        state.messages.push(BgMessage::Error(e.to_string()));
                    }
                    return;
                }
            };

            let result = match manifest.find_artifact(&name) {
                Some(artifact) => match artifact.find_version(&ver) {
                    Some(av) => {
                        commands::download::download_and_cache(
                            &config, &name, &ver, &av.download_url, &av.sha256,
                        )
                        .await
                    }
                    None => Err(crate::error::LauncherError::VersionNotFound {
                        tool: name.clone(),
                        version: ver.clone(),
                    }),
                },
                None => Err(crate::error::LauncherError::ArtifactNotFound(name.clone())),
            };

            if let Ok(mut state) = shared.lock() {
                match result {
                    Ok(()) => state.messages.push(BgMessage::DownloadComplete(name, ver)),
                    Err(e) => state.messages.push(BgMessage::Error(e.to_string())),
                }
            }
        });
    }

    fn trigger_run(&mut self, tool_name: &str, version: &str) {
        let cache = CacheManager::new(self.config.cache_dir(), self.config.cache.max_size_mb);
        let _ = cache.init();

        if let Some(cached_path) = cache.get_cached_path(tool_name, version) {
            let _ = cache.touch(tool_name, version);

            let launch_config = match commands::run::build_launch_config(
                tool_name, version, &cached_path, &[],
            ) {
                Ok(lc) => lc,
                Err(e) => {
                    self.status_message = format!("Failed to run: {}", e);
                    return;
                }
            };

            let executable_path = match commands::run::resolve_executable(
                &cached_path, &launch_config, tool_name,
            ) {
                Ok(p) => p,
                Err(e) => {
                    self.status_message = format!("Failed to run: {}", e);
                    return;
                }
            };

            let exec = crate::services::execution::ExecutionManager::new();
            match exec.execute(&executable_path, &launch_config) {
                Ok(result) => {
                    let key = format!("{} v{}", tool_name, version);
                    self.running_processes.insert(key.clone(), result.pid);
                    self.status_message = format!("Started {} (PID {})", key, result.pid);
                }
                Err(e) => {
                    self.status_message = format!("Failed to run: {}", e);
                }
            }
        } else {
            self.status_message = format!("{} v{} not cached — download first", tool_name, version);
        }
    }

    fn stop_process(&mut self, tool_name: &str, version: &str) {
        let key = format!("{} v{}", tool_name, version);
        if let Some(pid) = self.running_processes.remove(&key) {
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .output();
            }
            #[cfg(not(windows))]
            {
                unsafe { libc::kill(pid as i32, libc::SIGTERM); }
            }
            self.status_message = format!("Stopped {} (PID {})", key, pid);
        }
    }

    fn is_running(&self, tool_name: &str, version: &str) -> bool {
        let key = format!("{} v{}", tool_name, version);
        self.running_processes.contains_key(&key)
    }

    fn refresh_cache(&mut self) {
        let cache = CacheManager::new(self.config.cache_dir(), self.config.cache.max_size_mb);
        let _ = cache.init();
        self.cache_entries = cache.list_entries().unwrap_or_default();
    }

    fn process_messages(&mut self) {
        let messages: Vec<BgMessage> = {
            if let Ok(mut state) = self.shared.lock() {
                std::mem::take(&mut state.messages)
            } else {
                return;
            }
        };

        for msg in messages {
            match msg {
                BgMessage::ManifestLoaded(manifest) => {
                    let count = manifest.artifacts.len();
                    self.manifest = Some(manifest);
                    self.status_message = format!("Loaded {} artifacts", count);
                    self.loading = false;
                }
                BgMessage::Error(e) => {
                    self.status_message = format!("Error: {}", e);
                    self.loading = false;
                }
                BgMessage::DownloadComplete(name, ver) => {
                    self.status_message = format!("Downloaded {} v{}", name, ver);
                    self.loading = false;
                    self.refresh_cache();
                }
            }
        }
    }

    fn draw_run_stop_button(&mut self, ui: &mut egui::Ui, name: &str, version: &str) {
        if self.is_running(name, version) {
            if ui.button("⏹ Stop").clicked() {
                let n = name.to_string();
                let v = version.to_string();
                self.stop_process(&n, &v);
            }
        } else {
            if ui.button("▶ Run").clicked() {
                let n = name.to_string();
                let v = version.to_string();
                self.trigger_run(&n, &v);
            }
        }
    }

    fn draw_browse_tab(&mut self, ui: &mut egui::Ui) {
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

    fn draw_favorites_tab(&mut self, ui: &mut egui::Ui) {
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

    fn draw_cache_tab(&mut self, ui: &mut egui::Ui) {
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

    fn draw_settings_tab(&mut self, ui: &mut egui::Ui) {
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

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_messages();

        if self.loading {
            ctx.request_repaint();
        }

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, Tab::Browse, "📦 Browse");
                ui.selectable_value(&mut self.current_tab, Tab::Favorites, "★ Favorites");
                ui.selectable_value(&mut self.current_tab, Tab::Cache, "💾 Cache");
                ui.selectable_value(&mut self.current_tab, Tab::Settings, "⚙ Settings");
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.loading {
                    ui.spinner();
                }
                ui.label(&self.status_message);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.current_tab {
                Tab::Browse => self.draw_browse_tab(ui),
                Tab::Favorites => self.draw_favorites_tab(ui),
                Tab::Cache => self.draw_cache_tab(ui),
                Tab::Settings => self.draw_settings_tab(ui),
            }
        });
    }
}

pub fn run_gui(config: LauncherConfig, runtime: tokio::runtime::Handle) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Automation Launcher",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(LauncherApp::new(config, runtime)))
        }),
    )
    .map_err(|e| e.to_string())
}
