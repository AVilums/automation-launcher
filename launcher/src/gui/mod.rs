mod state;
mod tabs;

use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::artifact::ArtifactManifest;
use crate::commands;
use crate::config::LauncherConfig;
use crate::services::cache::{CacheEntry, CacheManager};
use crate::services::favorites::Favorites;

use state::{BgMessage, SharedState};
use tabs::Tab;

pub struct LauncherApp {
    pub(crate) config: LauncherConfig,
    pub(crate) manifest: Option<ArtifactManifest>,
    pub(crate) cache_entries: Vec<CacheEntry>,
    pub(crate) favorites: Favorites,
    pub(crate) favorites_path: PathBuf,
    pub(crate) search_query: String,
    pub(crate) status_message: String,
    current_tab: Tab,
    shared: Arc<Mutex<SharedState>>,
    pub(crate) runtime: tokio::runtime::Handle,
    pub(crate) loading: bool,
    pub(crate) running_processes: HashMap<String, u32>,
    pub(crate) settings_cache_mb: String,
    pub(crate) settings_telemetry: bool,
    pub(crate) settings_offline: bool,
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
            shared: SharedState::new(),
            runtime,
            loading: false,
            running_processes: HashMap::new(),
            settings_cache_mb,
            settings_telemetry,
            settings_offline,
        }
    }

    pub(crate) fn refresh_manifest(&mut self) {
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

    pub(crate) fn trigger_download(&mut self, tool_name: &str, version: &str) {
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

    pub(crate) fn trigger_run(&mut self, tool_name: &str, version: &str) {
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

    pub(crate) fn stop_process(&mut self, tool_name: &str, version: &str) {
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

    pub(crate) fn is_running(&self, tool_name: &str, version: &str) -> bool {
        let key = format!("{} v{}", tool_name, version);
        self.running_processes.contains_key(&key)
    }

    pub(crate) fn refresh_cache(&mut self) {
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

    pub(crate) fn draw_run_stop_button(&mut self, ui: &mut egui::Ui, name: &str, version: &str) {
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
