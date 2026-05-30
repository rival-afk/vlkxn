use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;
use vlkxn_controller::Daemon;

#[derive(Parser)]
#[command(name = "vlkxn", version, about = "Vlkxn - Decentralized P2P VPN for gaming")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the virtual network
    Up {
        /// Room name to join
        #[arg(long)]
        room: Option<String>,
        /// Display nickname
        #[arg(long)]
        nick: Option<String>,
    },
    /// Stop the virtual network
    Down,
    /// Show connection status
    Status,
    /// List online peers
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Up { room, nick } => {
            let mut daemon = Daemon::new().await?;
            if let Some(room) = room {
                daemon.config.network.room = room;
            }
            if let Some(nick) = nick {
                daemon.config.nickname.value = nick;
            }
            daemon.config.save()?;
            daemon.start().await?;

            println!("[✓] Virtual interface vlkxn0 created");
            println!("[✓] Connected to room: {}", daemon.config.network.room);
            println!("[✓] Virtual IP allocated");

            tokio::signal::ctrl_c().await?;
            daemon.stop().await?;
        }
        Commands::Down => {
            let mut daemon = Daemon::new().await?;
            daemon.stop().await?;
            println!("[✓] Interface removed");
        }
        Commands::Status => {
            let daemon = Daemon::new().await?;
            println!("{}", daemon.status());
        }
        Commands::List => {
            println!("Listing peers (not yet implemented)");
        }
    }

    Ok(())
}
