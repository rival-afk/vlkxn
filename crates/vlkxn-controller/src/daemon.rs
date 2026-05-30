use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{error, info, warn};
use vlkxn_core::config::Config;
use vlkxn_core::crypto::KeyManager;
use vlkxn_core::p2p::P2pNode;
use vlkxn_core::tun::TunInterface;
use vlkxn_core::types::*;

pub struct Daemon {
    pub config: Config,
    pub key_manager: KeyManager,
    shutdown: Option<Arc<tokio::sync::Notify>>,
    handles: Option<Vec<tokio::task::JoinHandle<()>>>,
}

impl Daemon {
    pub async fn new() -> anyhow::Result<Self> {
        let config = Config::load()?;
        let keys_path = Config::keys_path()?;
        let key_manager = KeyManager::load_or_generate(&keys_path)?;

        Ok(Self {
            config,
            key_manager,
            shutdown: None,
            handles: None,
        })
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        info!("Starting Vlkxn daemon...");

        let virtual_ip =
            vlkxn_core::crypto::virtual_ip_from_public_key(&self.key_manager.public_key());

        let mut tun = TunInterface::new("vlkxn0");
        tun.create(virtual_ip, 16).await?;
        info!("TUN created with IP: {virtual_ip}");

        let (mut p2p_node, event_rx) = P2pNode::new(
            &self.key_manager,
            self.config.network.room.clone(),
            virtual_ip,
        )
        .await?;

        info!("P2P peer ID: {}", p2p_node.peer_id());

        let packet_tx = p2p_node.packet_tx.clone();
        let shutdown = Arc::new(tokio::sync::Notify::new());
        let mut handles = Vec::new();

        // Task 1: P2P event loop (owns p2p_node)
        let s1 = shutdown.clone();
        handles.push(tokio::spawn(async move {
            tokio::select! {
                _ = s1.notified() => info!("P2P loop shut down"),
                () = p2p_node.run() => {}
            }
        }));

        // Task 2: Event handler (P2P → TUN)
        let tun = Arc::new(Mutex::new(tun));
        let tun_ev = tun.clone();
        let s2 = shutdown.clone();
        handles.push(tokio::spawn(async move {
            let mut event_rx = event_rx;
            loop {
                tokio::select! {
                    _ = s2.notified() => { info!("Event handler shut down"); break; }
                    Some(event) = event_rx.recv() => match event {
                        NetworkEvent::PeerConnected(info) => {
                            info!("Peer connected: {} (IP: {})", info.nickname, info.virtual_ip);
                        }
                        NetworkEvent::PeerDisconnected(node_id) => {
                            info!("Peer disconnected: {:?}", &node_id[..4]);
                        }
                        NetworkEvent::VirtualIpAssigned(ip) => {
                            info!("Virtual IP assigned: {ip}");
                        }
                        NetworkEvent::PacketReceived(pkt) => {
                            if let Ok(mut tun) = tun_ev.try_lock()
                                && let Err(e) = tun.write_packet(&pkt.data).await {
                                    warn!("TUN write error: {e}");
                                }
                        }
                    }
                }
            }
        }));

        // Task 3: TUN reader (TUN → P2P)
        let tun_rd = tun.clone();
        let pt = packet_tx;
        let s3 = shutdown.clone();
        handles.push(tokio::spawn(async move {
            let mut buf = vec![0u8; 65535];
            loop {
                tokio::select! {
                    _ = s3.notified() => { info!("TUN reader shut down"); break; }
                    result = async {
                        let mut tun = tun_rd.lock().await;
                        tun.read_packet(&mut buf).await
                    } => {
                        match result {
                            Ok(n) => {
                                if n < 20 { continue; }
                                let packet = buf[..n].to_vec();
                                let _ = pt.send(packet);
                            }
                            Err(e) => {
                                if would_block(&e) {
                                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                                } else {
                                    error!("TUN read error: {e}");
                                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                                }
                            }
                        }
                    }
                }
            }
        }));

        self.shutdown = Some(shutdown);
        self.handles = Some(handles);
        info!("Vlkxn daemon started");
        Ok(())
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        info!("Stopping Vlkxn daemon...");
        if let Some(shutdown) = self.shutdown.take() {
            shutdown.notify_waiters();
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        self.handles = None;
        info!("Vlkxn daemon stopped");
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.shutdown.is_some()
    }

    pub fn status(&self) -> String {
        if self.is_running() {
            format!(
                "Vlkxn is running\nRoom: {}\nPeers: active",
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
