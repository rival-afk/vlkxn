use clap::{Parser, Subcommand};
use std::process::Command;
use tracing_subscriber::EnvFilter;
use vlkxn_controller::Daemon;

#[derive(Parser)]
#[command(
    name = "vlkxn",
    version,
    about = "Vlkxn - Decentralized P2P VPN for gaming"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the virtual network
    Up {
        #[arg(long)]
        room: Option<String>,
        #[arg(long)]
        nick: Option<String>,
    },
    /// Stop the virtual network
    Down,
    /// Show connection status
    Status,
    /// List online peers
    List,
    /// Install permissions (Linux: setcap CAP_NET_ADMIN)
    Install,
}

fn check_linux_capabilities() -> bool {
    #[cfg(target_os = "linux")]
    {
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
            return true;
        }

        if let Ok(output) = Command::new("id").arg("-u").output()
            && let Ok(uid) = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u32>()
            && uid == 0
        {
            return true;
        }

        false
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

fn setup_linux_capabilities() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let self_path = std::env::current_exe()?;
        let status = Command::new("sudo")
            .args(["setcap", "cap_net_admin+ep", self_path.to_str().unwrap()])
            .status()?;

        if status.success() {
            println!("[✓] CAP_NET_ADMIN capability set!");
            println!("[✓] You can now run Vlkxn without sudo!");
            Ok(())
        } else {
            anyhow::bail!(
                "Failed to set capabilities. Try: sudo setcap cap_net_admin+ep {}",
                self_path.display()
            )
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        println!("Permission setup not needed on this platform");
        Ok(())
    }
}

fn print_welcome() {
    println!(
        "🌋 Vlkxn v{} — Decentralized P2P VPN for Gaming",
        env!("CARGO_PKG_VERSION")
    );
    println!();
}

fn print_permission_hint() {
    #[cfg(target_os = "linux")]
    {
        if !check_linux_capabilities() {
            println!("⚠ Linux: требуется CAP_NET_ADMIN для TUN адаптера");
            println!("  Запустите один раз:");
            println!("    sudo vlkxn install");
            println!("  или:");
            println!("    sudo setcap cap_net_admin+ep $(which vlkxn)");
            println!("  или запустите с sudo:");
            println!("    sudo vlkxn up --room MyGame");
            println!();
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Up { room, nick } => {
            print_welcome();
            print_permission_hint();

            let mut daemon = Daemon::new().await?;
            if let Some(room) = room {
                daemon.config.network.room = room;
            }
            if let Some(nick) = nick {
                daemon.config.nickname.value = nick;
            }
            daemon.config.save()?;

            match daemon.start().await {
                Ok(()) => {
                    let vip = vlkxn_core::crypto::virtual_ip_from_public_key(
                        &daemon.key_manager.public_key(),
                    );
                    println!("[✓] Connected to room: {}", daemon.config.network.room);
                    println!("[✓] Virtual IP: {vip}");
                    println!("[✓] Press Ctrl+C to stop");

                    tokio::signal::ctrl_c().await?;
                    daemon.stop().await?;
                    println!("[✓] Vlkxn stopped");
                }
                Err(e) => {
                    eprintln!("[✗] Error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Down => {
            let mut daemon = Daemon::new().await?;
            daemon.stop().await?;
            println!("[✓] Interface removed");
        }
        Commands::Status => {
            print_welcome();
            let daemon = Daemon::new().await?;
            if daemon.is_running() {
                println!("{}", daemon.status().await);
            } else {
                println!("Vlkxn is not running");
                println!("  Start with: vlkxn up --room <room>");
                println!("  Config: ~/.config/vlkxn/config.toml");
                let vip = vlkxn_core::crypto::virtual_ip_from_public_key(
                    &daemon.key_manager.public_key(),
                );
                println!("  Your virtual IP: {vip}");
                println!("  Room: {}", daemon.config.network.room);
            }
        }
        Commands::List => {
            print_welcome();
            let daemon = Daemon::new().await?;
            if daemon.is_running() {
                let peers = daemon.peer_list().await;
                if peers.is_empty() {
                    println!("No peers connected");
                } else {
                    println!("Peers ({}):", peers.len());
                    for p in &peers {
                        let conn = match p.connection_type {
                            vlkxn_core::types::ConnectionType::Direct => "direct",
                            vlkxn_core::types::ConnectionType::Relay => "relay",
                        };
                        println!(
                            "  {} (IP: {}, ping: {}ms, {conn})",
                            p.nickname, p.virtual_ip, p.ping_ms
                        );
                    }
                }
            } else {
                println!("Vlkxn is not running");
                println!("  Start with: vlkxn up --room <room>");
            }
        }
        Commands::Install => {
            #[cfg(target_os = "linux")]
            {
                println!("🔧 Setting up Vlkxn permissions...");
                match setup_linux_capabilities() {
                    Ok(()) => println!("[✓] Setup complete! Run `vlkxn up` to start."),
                    Err(e) => {
                        eprintln!("[✗] {e}");
                        std::process::exit(1);
                    }
                }
            }
            #[cfg(not(target_os = "linux"))]
            {
                println!("Permission setup is not needed on this platform.");
                println!("On Windows, run the installer as Administrator.");
            }
        }
    }

    Ok(())
}
