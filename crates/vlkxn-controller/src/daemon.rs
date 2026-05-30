use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{error, info};
use vlkxn_core::config::Config;
use vlkxn_core::crypto::KeyManager;
use vlkxn_core::p2p::P2pNode;
use vlkxn_core::tun::TunInterface;
use vlkxn_core::types::*;

pub struct Daemon {
    pub config: Config,
    pub key_manager: KeyManager,
    p2p: Option<Arc<Mutex<P2pNode>>>,
    tun: Option<Arc<Mutex<TunInterface>>>,
    running: bool,
}

impl Daemon {
    pub async fn new() -> anyhow::Result<Self> {
        let config = Config::load()?;
        let keys_path = Config::keys_path()?;
        let key_manager = KeyManager::load_or_generate(&keys_path)?;

        Ok(Self {
            config,
            key_manager,
            p2p: None,
            tun: None,
            running: false,
        })
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        info!("Starting Vlkxn daemon...");

        let virtual_ip =
            vlkxn_core::crypto::virtual_ip_from_public_key(&self.key_manager.public_key());

        let mut tun = TunInterface::new("vlkxn0");
        tun.create(virtual_ip, 16).await?;
        info!("TUN interface created with IP: {virtual_ip}");

        let (p2p_node, event_rx) = P2pNode::new(
            &self.key_manager,
            self.config.network.room.clone(),
            virtual_ip,
        )
        .await?;

        info!("P2P node started with PeerId: {}", p2p_node.peer_id());
        info!("Virtual IP: {virtual_ip}");

        let p2p = Arc::new(Mutex::new(p2p_node));
        let tun = Arc::new(Mutex::new(tun));

        let p2p_clone = p2p.clone();
        tokio::spawn(async move {
            let mut node = p2p_clone.lock().await;
            node.run().await;
        });

        tokio::spawn(async move {
            let mut event_rx = event_rx;
            while let Some(event) = event_rx.recv().await {
                match event {
                    NetworkEvent::PeerConnected(info) => {
                        info!("Peer connected: {} (IP: {})", info.nickname, info.virtual_ip);
                    }
                    NetworkEvent::PeerDisconnected(node_id) => {
                        info!("Peer disconnected: {:?}", &node_id[..4]);
                    }
                    NetworkEvent::VirtualIpAssigned(ip) => {
                        info!("Virtual IP assigned: {ip}");
                    }
                    NetworkEvent::PacketReceived(data) => {
                        let _ = data;
                    }
                }
            }
        });

        let p2p_clone3 = p2p.clone();
        let tun_clone = tun.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65535];
            loop {
                let n = match tun_clone.lock().await.read_packet(&mut buf).await {
                    Ok(n) => n,
                    Err(e) => {
                        if would_block(&e) {
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                            continue;
                        }
                        error!("TUN read error: {e}");
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        continue;
                    }
                };

                let _packet = buf[..n].to_vec();
                let p2p_guard = p2p_clone3.lock().await;
                let _peer_count = p2p_guard.peer_count();
                drop(p2p_guard);
            }
        });

        self.p2p = Some(p2p);
        self.tun = Some(tun);
        self.running = true;

        info!("Vlkxn daemon started successfully");
        Ok(())
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        info!("Stopping Vlkxn daemon...");
        self.p2p = None;
        self.tun = None;
        self.running = false;
        info!("Vlkxn daemon stopped");
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn status(&self) -> String {
        if self.running {
            let peer_count = self
                .p2p
                .as_ref()
                .map(|_| "active".to_string())
                .unwrap_or_else(|| "initializing".to_string());
            format!(
                "Vlkxn is running\nRoom: {}\nPeers: {peer_count}",
                self.config.network.room,
            )
        } else {
            "Vlkxn is not running".to_string()
        }
    }
}

fn would_block(e: &anyhow::Error) -> bool {
    e.downcast_ref::<std::io::Error>()
        .map(|ioe| ioe.kind() == std::io::ErrorKind::WouldBlock)
        .unwrap_or(false)
}
