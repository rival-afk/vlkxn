use eframe::egui;
use std::process::Command;

#[allow(dead_code)]
enum AppState {
    Disconnected,
    Connecting,
    Connected { vip: String },
    Error(String),
}

struct VlkxnApp {
    state: AppState,
    room: String,
    nickname: String,
    status_msg: String,
    peers: Vec<String>,
    needs_permissions: bool,
    show_setup: bool,
}

impl Default for VlkxnApp {
    fn default() -> Self {
        Self {
            state: AppState::Disconnected,
            room: "public".into(),
            nickname: format!("Player{}", rand::random::<u16>()),
            status_msg: String::new(),
            peers: Vec::new(),
            needs_permissions: check_capabilities(),
            show_setup: false,
        }
    }
}

fn check_capabilities() -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = Command::new("id").arg("-u").output()
            && let Ok(uid) = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u32>()
            && uid == 0
        {
            return false;
        }

        let cap_effective = std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines().find_map(|l| {
                    if l.starts_with("CapEff:") {
                        l.split(':').nth(1).map(|s| s.trim().to_string())
                    } else {
                        None
                    }
                })
            });

        if let Some(cap_eff) = cap_effective
            && let Ok(val) = u64::from_str_radix(&cap_eff, 16)
            && val & (1 << 12) != 0
        {
            return false;
        }

        true
    }

    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

fn setup_permissions() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let self_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let status = Command::new("pkexec")
            .args(["setcap", "cap_net_admin+ep", self_path.to_str().unwrap()])
            .status()
            .map_err(|e| format!("Failed to launch pkexec: {e}"))?;

        if status.success() {
            return Ok(());
        }

        let status = Command::new("sudo")
            .args(["setcap", "cap_net_admin+ep", self_path.to_str().unwrap()])
            .status()
            .map_err(|e| format!("Failed to launch sudo: {e}"))?;

        if status.success() {
            Ok(())
        } else {
            Err("Permission setup failed".into())
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        Err("Not supported on this platform".into())
    }
}

impl VlkxnApp {
    fn toggle_vpn(&mut self) {
        match &self.state {
            AppState::Disconnected | AppState::Error(_) => {
                if self.needs_permissions && !self.show_setup {
                    self.show_setup = true;
                    return;
                }

                self.state = AppState::Connecting;
                self.status_msg = "Connecting...".into();
                let room = self.room.clone();
                let nick = self.nickname.clone();

                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    match vlkxn_controller::Daemon::new().await {
                        Ok(mut daemon) => {
                            daemon.config.network.room = room;
                            daemon.config.nickname.value = nick;
                            let _ = daemon.config.save();
                            match daemon.start().await {
                                Ok(()) => tracing::info!("Connected"),
                                Err(e) => tracing::warn!("Start failed: {e}"),
                            }
                        }
                        Err(e) => tracing::warn!("Init failed: {e}"),
                    }
                });
            }
            AppState::Connecting | AppState::Connected { .. } => {
                self.state = AppState::Disconnected;
                self.status_msg = "Disconnected".into();
            }
        }
    }

    fn run_setup(&mut self) {
        match setup_permissions() {
            Ok(()) => {
                self.needs_permissions = false;
                self.show_setup = false;
                self.status_msg = "Permissions configured!".into();
            }
            Err(e) => {
                self.status_msg = format!("Setup failed: {e}");
            }
        }
    }
}

impl eframe::App for VlkxnApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🌋 Vlkxn");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (text, color) = match &self.state {
                        AppState::Disconnected => ("● Disconnected", egui::Color32::GRAY),
                        AppState::Connecting => ("● Connecting...", egui::Color32::YELLOW),
                        AppState::Connected { .. } => ("● Connected", egui::Color32::GREEN),
                        AppState::Error(_) => ("● Error", egui::Color32::RED),
                    };
                    ui.label(egui::RichText::new(text).color(color));
                    if let AppState::Connected { vip } = &self.state {
                        ui.label(format!("VIP: {vip}  Peers: {}", self.peers.len()));
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);

                let (btn_text, btn_enabled) = match &self.state {
                    AppState::Disconnected => ("▶  Включить Vlkxn", true),
                    AppState::Connecting => ("⏳  Подключение...", false),
                    AppState::Connected { .. } => ("⏹  Выключить Vlkxn", true),
                    AppState::Error(_) => ("▶  Включить Vlkxn", true),
                };

                let btn = egui::Button::new(egui::RichText::new(btn_text).size(18.0))
                    .min_size(egui::vec2(220.0, 48.0));

                if ui.add_enabled(btn_enabled, btn).clicked() {
                    self.toggle_vpn();
                }
            });

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(10.0);

            // Permission setup dialog
            if self.show_setup {
                egui::Window::new("🔧 Permission Setup")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(
                                "Vlkxn requires CAP_NET_ADMIN to create\na virtual network adapter."
                            ).size(14.0));
                            ui.add_space(10.0);
                            ui.label("Click below to grant permissions (sudo required):");
                            ui.add_space(10.0);

                            if ui.button("🔑 Grant Permissions").clicked() {
                                self.run_setup();
                            }

                            ui.add_space(5.0);
                            if ui.button("Cancel").clicked() {
                                self.show_setup = false;
                            }
                            ui.add_space(10.0);
                        });
                    });
            }

            if self.needs_permissions && !self.show_setup {
                ui.vertical_centered(|ui| {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "⚠ Permissions required for TUN adapter",
                    );
                    if ui.link("Click to setup").clicked() {
                        self.show_setup = true;
                    }
                });
                ui.add_space(10.0);
            }

            egui::Grid::new("config_grid")
                .num_columns(2)
                .spacing([8.0, 8.0])
                .show(ui, |ui| {
                    let editable =
                        matches!(self.state, AppState::Disconnected | AppState::Error(_));
                    ui.label("Room:");
                    ui.add_enabled(editable, egui::TextEdit::singleline(&mut self.room));
                    ui.end_row();

                    ui.label("Nickname:");
                    ui.add_enabled(editable, egui::TextEdit::singleline(&mut self.nickname));
                    ui.end_row();
                });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("Peers");
            if self.peers.is_empty() {
                ui.label("No peers connected");
            } else {
                for peer in &self.peers {
                    ui.label(peer);
                }
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(5.0);

            if !self.status_msg.is_empty() {
                ui.colored_label(
                    if self.status_msg.contains("failed") || self.status_msg.contains("Error") {
                        egui::Color32::RED
                    } else {
                        egui::Color32::LIGHT_BLUE
                    },
                    &self.status_msg,
                );
            }
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([480.0, 560.0])
            .with_min_inner_size([400.0, 450.0])
            .with_title("Vlkxn — P2P VPN for Gaming"),
        ..Default::default()
    };

    eframe::run_native(
        "Vlkxn",
        options,
        Box::new(|_cc| Ok(Box::<VlkxnApp>::default())),
    )
}
