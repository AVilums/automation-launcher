use eframe::egui;

use super::super::LauncherApp;
use super::super::systems_data::{RunnerNode, RunnerStatus, RunnerTask};

impl LauncherApp {
    pub fn draw_systems_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Connected Runners");
            if ui.button("🔄 Refresh").clicked() {
                self.refresh_runners();
            }
            if ui.button("🧪 Load Demo Data").clicked() {
                self.load_demo_runners();
            }
            ui.separator();
            let online = self.runner_nodes.iter().filter(|n| n.status == RunnerStatus::Online).count();
            let busy = self.runner_nodes.iter().filter(|n| n.status == RunnerStatus::Busy).count();
            let offline = self.runner_nodes.iter().filter(|n| n.status == RunnerStatus::Offline).count();
            ui.label(format!(
                "🟢 {} online  🟡 {} busy  🔴 {} offline",
                online, busy, offline,
            ));
        });

        ui.separator();

        if self.runner_nodes.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label("No runners connected.");
                ui.add_space(10.0);
                ui.label("Configure a systems endpoint in Settings, or add runners manually.");
                ui.add_space(10.0);
                ui.label("Use 🧪 Load Demo Data above to preview the UI with sample runners.");
                ui.add_space(20.0);
                if ui.button("➕ Add Runner Manually").clicked() {
                    self.show_add_runner_dialog = true;
                }
            });
        }

        // Add runner dialog
        if self.show_add_runner_dialog {
            self.draw_add_runner_dialog(ui);
        }

        if self.runner_nodes.is_empty() {
            return;
        }

        // Node cards grid + detail panel
        let selected = self.selected_runner.clone();

        ui.columns(2, |cols| {
            // Left: node cards
            egui::ScrollArea::vertical()
                .id_salt("runner_cards")
                .show(&mut cols[0], |ui| {
                    let nodes = self.runner_nodes.clone();
                    for node in &nodes {
                        let is_selected = selected.as_ref() == Some(&node.machine_name);
                        let response = ui.group(|ui| {
                            Self::draw_node_card(ui, node);
                        }).response;

                        if response.interact(egui::Sense::click()).clicked() {
                            self.selected_runner = Some(node.machine_name.clone());
                        }

                        if is_selected {
                            ui.painter().rect_stroke(
                                response.rect,
                                4.0,
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 149, 237)),
                                egui::StrokeKind::Outside,
                            );
                        }
                    }
                });

            // Right: detail panel
            if let Some(ref name) = self.selected_runner.clone() {
                if let Some(node) = self.runner_nodes.iter().find(|n| &n.machine_name == name) {
                    let node = node.clone();
                    Self::draw_runner_detail(&mut cols[1], &node);
                }
            } else {
                cols[1].vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label("Click a runner to view details.");
                });
            }
        });
    }

    fn draw_node_card(ui: &mut egui::Ui, node: &RunnerNode) {
        ui.horizontal(|ui| {
            let status_icon = match node.status {
                RunnerStatus::Online => "🟢",
                RunnerStatus::Busy => "🟡",
                RunnerStatus::Offline => "🔴",
            };
            ui.label(status_icon);
            ui.strong(&node.machine_name);
        });
        ui.label(&node.os);
        ui.label(format!("Runner v{}", node.runner_version));
        ui.label(format!(
            "Task: {}",
            node.current_task.as_deref().unwrap_or("idle")
        ));
        ui.label(format!("♥ {}", node.last_heartbeat_display()));
    }

    fn draw_runner_detail(ui: &mut egui::Ui, node: &RunnerNode) {
        egui::ScrollArea::vertical()
            .id_salt("runner_detail")
            .show(ui, |ui| {
                ui.heading(&node.machine_name);
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Status:");
                    let (icon, text) = match node.status {
                        RunnerStatus::Online => ("🟢", "Online"),
                        RunnerStatus::Busy => ("🟡", "Busy"),
                        RunnerStatus::Offline => ("🔴", "Offline"),
                    };
                    ui.label(format!("{} {}", icon, text));
                });
                ui.label(format!("OS: {}", node.os));
                ui.label(format!("Runner version: {}", node.runner_version));
                ui.label(format!("Last heartbeat: {}", node.last_heartbeat_display()));

                if let Some(ref task) = node.current_task {
                    ui.label(format!("Current task: {}", task));
                }

                // Runtime capabilities
                if !node.runtimes.is_empty() {
                    ui.separator();
                    ui.strong("Runtime Capabilities");
                    for rt in &node.runtimes {
                        ui.label(format!("  • {}", rt));
                    }
                }

                // Recent tasks
                ui.separator();
                ui.strong("Recent Tasks");
                if node.recent_tasks.is_empty() {
                    ui.label("No recent tasks.");
                } else {
                    egui::Grid::new("recent_tasks_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Task");
                            ui.strong("Status");
                            ui.strong("Duration");
                            ui.strong("Time");
                            ui.end_row();

                            for task in &node.recent_tasks {
                                ui.label(&task.name);
                                ui.label(&task.status);
                                ui.label(&task.duration);
                                ui.label(&task.timestamp);
                                ui.end_row();
                            }
                        });
                }

                // Logs
                ui.separator();
                ui.strong("Logs");
                if node.logs.is_empty() {
                    ui.label("No logs available.");
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("runner_logs")
                        .max_height(200.0)
                        .show(ui, |ui| {
                            ui.monospace(node.logs.join("\n"));
                        });
                }
            });
    }

    fn draw_add_runner_dialog(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.group(|ui| {
            ui.heading("Add Runner");
            ui.horizontal(|ui| {
                ui.label("Endpoint URL:");
                ui.text_edit_singleline(&mut self.add_runner_url);
            });
            ui.horizontal(|ui| {
                if ui.button("Add").clicked() && !self.add_runner_url.is_empty() {
                    // Add a placeholder node that will be resolved on next refresh
                    let node = RunnerNode {
                        machine_name: self.add_runner_url.clone(),
                        os: "Unknown".into(),
                        runner_version: "?".into(),
                        status: RunnerStatus::Offline,
                        current_task: None,
                        last_heartbeat_secs: 999,
                        runtimes: Vec::new(),
                        recent_tasks: Vec::new(),
                        logs: Vec::new(),
                        endpoint: self.add_runner_url.clone(),
                    };
                    self.runner_nodes.push(node);
                    self.add_runner_url.clear();
                    self.show_add_runner_dialog = false;
                    self.status_message = "Runner added (will resolve on refresh)".into();
                }
                if ui.button("Cancel").clicked() {
                    self.show_add_runner_dialog = false;
                }
            });
        });
    }

    pub(crate) fn refresh_runners(&mut self) {
        for node in &mut self.runner_nodes {
            node.last_heartbeat_secs += 5;
            if node.last_heartbeat_secs > 60 {
                node.status = RunnerStatus::Offline;
            }
        }
        self.status_message = format!("Refreshed {} runners", self.runner_nodes.len());
    }

    fn load_demo_runners(&mut self) {
        self.runner_nodes = vec![
            RunnerNode {
                machine_name: "PROD-SVR-01".into(),
                os: "Windows 11 Pro".into(),
                runner_version: "0.1.0".into(),
                status: RunnerStatus::Online,
                current_task: None,
                last_heartbeat_secs: 2,
                runtimes: vec!["native".into(), "PowerShell 7.4".into(), ".NET 8".into()],
                recent_tasks: vec![
                    RunnerTask { name: "free-resource".into(), status: "✅ Success".into(), duration: "4.2s".into(), timestamp: "2026-03-13 23:50".into() },
                    RunnerTask { name: "daily-report".into(), status: "✅ Success".into(), duration: "12.1s".into(), timestamp: "2026-03-13 23:00".into() },
                    RunnerTask { name: "data-sync".into(), status: "⚠ Timeout".into(), duration: "300s".into(), timestamp: "2026-03-13 22:00".into() },
                ],
                logs: vec![
                    "[23:50:04] Starting free-resource v1.0.4".into(),
                    "[23:50:04] Cleaning temp directory...".into(),
                    "[23:50:08] Freed 1.2 GB of disk space".into(),
                    "[23:50:08] Task completed successfully".into(),
                ],
                endpoint: "http://prod-svr-01:8090".into(),
            },
            RunnerNode {
                machine_name: "PROD-SVR-02".into(),
                os: "Ubuntu 22.04 LTS".into(),
                runner_version: "0.1.0".into(),
                status: RunnerStatus::Busy,
                current_task: Some("data-sync v3.0.1".into()),
                last_heartbeat_secs: 5,
                runtimes: vec!["native".into(), "Python 3.11".into(), "Node 20".into()],
                recent_tasks: vec![
                    RunnerTask { name: "data-sync".into(), status: "🔄 Running".into(), duration: "45s...".into(), timestamp: "2026-03-14 00:50".into() },
                    RunnerTask { name: "daily-report".into(), status: "✅ Success".into(), duration: "8.3s".into(), timestamp: "2026-03-13 23:00".into() },
                ],
                logs: vec![
                    "[00:50:12] Starting data-sync v3.0.1".into(),
                    "[00:50:12] Connecting to database...".into(),
                    "[00:50:15] Syncing 1,247 records...".into(),
                ],
                endpoint: "http://prod-svr-02:8090".into(),
            },
            RunnerNode {
                machine_name: "DEV-PC-03".into(),
                os: "Windows 10 Pro".into(),
                runner_version: "0.0.9".into(),
                status: RunnerStatus::Offline,
                current_task: None,
                last_heartbeat_secs: 320,
                runtimes: vec!["native".into(), "PowerShell 5.1".into()],
                recent_tasks: vec![
                    RunnerTask { name: "free-resource".into(), status: "❌ Failed".into(), duration: "0.5s".into(), timestamp: "2026-03-13 19:30".into() },
                ],
                logs: vec![
                    "[19:30:01] Starting free-resource v1.0.4".into(),
                    "[19:30:01] ERROR: Access denied to C:\\Windows\\Temp".into(),
                    "[19:30:01] Task failed with exit code 1".into(),
                ],
                endpoint: "http://dev-pc-03:8090".into(),
            },
        ];
        self.selected_runner = Some("PROD-SVR-01".into());
        self.status_message = "Loaded 3 demo runners".into();
    }
}
