use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::artifact::{Artifact, ArtifactManifest};
use crate::services::cache::{CacheEntry, CacheManager};
use crate::config::LauncherConfig;
use crate::services::favorites::Favorites;

/// Messages sent from background tasks to the GUI.
enum BgMessage {
    ManifestLoaded(ArtifactManifest),
    Error(String),
    DownloadComplete(String, String),
}

/// Shared state between the GUI and background tasks.
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
    // Settings fields
    settings_cache_mb: String,
    settings_telemetry: bool,
    settings_offline: bool,
}

impl LauncherApp {
    pub fn new(
        config: LauncherConfig,
        runtime: tokio::runtime::Handle,
    ) -> Self {
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
            settings_cache_mb,
            settings_telemetry,
            settings_offline,
        }
    }

    fn refresh_manifest(&mut self) {
        if self.loading || self.config.offline_mode {
            if self.config.offline_mode {
                // Try loading cached manifest
                let path = self.config.metadata_dir().join("manifest.json");
                if path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(m) = serde_json::from_str::<ArtifactManifest>(&content) {
                            self.manifest = Some(m);
                            self.status_message = "Loaded cached manifest (offline)".into();
                        }
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
            let result = fetch_manifest_async(&config).await;
            if let Ok(mut state) = shared.lock() {
                match result {
                    Ok(manifest) => state.messages.push(BgMessage::ManifestLoaded(manifest)),
                    Err(e) => state.messages.push(BgMessage::Error(e.to_string())),
                }
            }
        });
    }

    fn trigger_download(&mut self, tool_name: &str, version: &str) {
        self.status_message = format!("Downloading {} v{}...", tool_name, version);
        self.loading = true;

        let shared = self.shared.clone();
        let config = self.config.clone();
        let name = tool_name.to_string();
        let ver = version.to_string();

        self.runtime.spawn(async move {
            let result = download_async(&config, &name, &ver).await;
            if let Ok(mut state) = shared.lock() {
                match result {
                    Ok(()) => state
                        .messages
                        .push(BgMessage::DownloadComplete(name, ver)),
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

            let executable_path = cached_path
                .parent()
                .unwrap_or(&cached_path)
                .join(cached_path.file_name().unwrap_or_default());

            let launch_config = crate::artifact::LaunchConfig {
                executable: cached_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                args: None,
                env: None,
            };

            let exec = crate::services::execution::ExecutionManager::new();
            match exec.execute(&executable_path, &launch_config) {
                Ok(result) => {
                    self.status_message =
                        format!("Started {} v{} (PID {})", tool_name, version, result.pid);
                }
                Err(e) => {
                    self.status_message = format!("Failed to run: {}", e);
                }
            }
        } else {
            self.status_message = format!("{} v{} not cached — download first", tool_name, version);
        }
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
                    // Cache manifest for offline use
                    let metadata_dir = self.config.metadata_dir();
                    let _ = std::fs::create_dir_all(&metadata_dir);
                    if let Ok(json) = serde_json::to_string_pretty(&manifest) {
                        let _ = std::fs::write(metadata_dir.join("manifest.json"), json);
                    }
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
                                if ui.button("▶ Run").clicked() {
                                    let name = artifact.name.clone();
                                    let v = ver.version.clone();
                                    self.trigger_run(&name, &v);
                                }
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

                        // Find latest cached version
                        let cached = self
                            .cache_entries
                            .iter()
                            .find(|e| &e.tool_name == name);

                        if let Some(entry) = cached {
                            ui.label(format!("v{} (cached)", entry.version));
                            if ui.button("▶ Run").clicked() {
                                let n = name.clone();
                                let v = entry.version.clone();
                                self.trigger_run(&n, &v);
                            }
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
                    if ui.button("▶ Run").clicked() {
                        to_run = Some((entry.tool_name.clone(), entry.version.clone()));
                    }
                });
            }
        });

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

        ui.label(format!("Config file: {}", self.config.base_dir.join("config").join("launcher.json").display()));
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

        // Request repaint while loading
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

/// Launch the GUI window.
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

async fn fetch_manifest_async(
    config: &LauncherConfig,
) -> Result<ArtifactManifest, crate::error::LauncherError> {
    use crate::config::ProviderConfig;
    use crate::providers::{ArtifactProvider, GitHubProvider, HttpProvider};

    match &config.provider {
        ProviderConfig::GitHub { owner, repo } => {
            let provider = GitHubProvider::new(owner.clone(), repo.clone());
            provider.fetch_manifest().await
        }
        ProviderConfig::Http { base_url } => {
            let provider = HttpProvider::new(base_url.clone());
            provider.fetch_manifest().await
        }
    }
}

async fn download_async(
    config: &LauncherConfig,
    tool_name: &str,
    version: &str,
) -> Result<(), crate::error::LauncherError> {
    use crate::providers::{ArtifactProvider, GitHubProvider, HttpProvider};
    use crate::config::ProviderConfig;

    let manifest = match &config.provider {
        ProviderConfig::GitHub { owner, repo } => {
            let provider = GitHubProvider::new(owner.clone(), repo.clone());
            provider.fetch_manifest().await?
        }
        ProviderConfig::Http { base_url } => {
            let provider = HttpProvider::new(base_url.clone());
            provider.fetch_manifest().await?
        }
    };

    let artifact = manifest
        .find_artifact(tool_name)
        .ok_or_else(|| crate::error::LauncherError::ArtifactNotFound(tool_name.into()))?;

    let ver = artifact
        .find_version(version)
        .ok_or_else(|| crate::error::LauncherError::VersionNotFound {
            tool: tool_name.into(),
            version: version.into(),
        })?;

    let dm = crate::services::download::DownloadManager::new(config.downloads_dir());
    let downloaded = dm.download(&ver.download_url, &ver.sha256).await?;

    let cache = CacheManager::new(config.cache_dir(), config.cache.max_size_mb);
    cache.init()?;

    // If it's a zip, extract first
    if crate::services::extract::is_zip(&downloaded) {
        let extract_dir = config.cache_dir().join(tool_name).join(version);
        let files = crate::services::extract::extract_zip(&downloaded, &extract_dir)?;
        // Store the first executable found, or the first file
        if let Some(exe) = files.iter().find(|f| {
            f.extension()
                .map(|e| e == "exe" || e == "ps1" || e == "bat")
                .unwrap_or(false)
        }) {
            cache.store(tool_name, version, exe)?;
        } else if let Some(first) = files.first() {
            cache.store(tool_name, version, first)?;
        }
    } else {
        cache.store(tool_name, version, &downloaded)?;
    }

    let _ = std::fs::remove_file(&downloaded);
    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
