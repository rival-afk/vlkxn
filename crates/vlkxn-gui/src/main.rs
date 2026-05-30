use eframe::egui;

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
}

impl Default for VlkxnApp {
    fn default() -> Self {
        Self {
            state: AppState::Disconnected,
            room: "public".into(),
            nickname: format!("Player{}", rand::random::<u16>()),
            status_msg: String::new(),
            peers: Vec::new(),
        }
    }
}

impl VlkxnApp {
    fn toggle_vpn(&mut self) {
        match &self.state {
            AppState::Disconnected | AppState::Error(_) => {
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
}

impl eframe::App for VlkxnApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🌋 Vlkxn");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match &self.state {
                        AppState::Disconnected => {
                            ui.label(egui::RichText::new("● Disconnected").color(egui::Color32::GRAY));
                        }
                        AppState::Connecting => {
                            ui.label(egui::RichText::new("● Connecting...").color(egui::Color32::YELLOW));
                        }
                        AppState::Connected { vip } => {
                            ui.label(egui::RichText::new("● Connected").color(egui::Color32::GREEN));
                            ui.label(format!("VIP: {vip}"));
                            ui.label(format!("Peers: {}", self.peers.len()));
                        }
                        AppState::Error(e) => {
                            ui.label(egui::RichText::new(format!("● Error: {e}")).color(egui::Color32::RED));
                        }
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
                    .min_size(egui::vec2(200.0, 48.0));

                if ui.add_enabled(btn_enabled, btn).clicked() {
                    self.toggle_vpn();
                }
            });

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(10.0);

            egui::Grid::new("config_grid")
                .num_columns(2)
                .spacing([8.0, 8.0])
                .show(ui, |ui| {
                    let editable = matches!(self.state, AppState::Disconnected | AppState::Error(_));
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
                ui.label(&self.status_msg);
            }
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([480.0, 520.0])
            .with_min_inner_size([400.0, 400.0])
            .with_title("Vlkxn — P2P VPN for Gaming"),
        ..Default::default()
    };

    eframe::run_native(
        "Vlkxn",
        options,
        Box::new(|_cc| Ok(Box::<VlkxnApp>::default())),
    )
}
