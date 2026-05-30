use std::collections::HashMap;

use futures::StreamExt;
use libp2p::{
    identify, kad, mdns,
    noise,
    ping,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, PeerId, Swarm, SwarmBuilder,
};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::crypto::KeyManager;
use crate::types::*;

#[derive(NetworkBehaviour)]
pub struct VlkxnBehaviour {
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub mdns: mdns::tokio::Behaviour,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
}

pub struct P2pNode {
    pub swarm: Swarm<VlkxnBehaviour>,
    pub peer_id: PeerId,
    pub event_tx: mpsc::UnboundedSender<NetworkEvent>,
    peers: HashMap<PeerId, PeerInfo>,
    _room: RoomName,
}

impl P2pNode {
    pub async fn new(
        key_manager: &KeyManager,
        _room: RoomName,
    ) -> anyhow::Result<(Self, mpsc::UnboundedReceiver<NetworkEvent>)> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let keypair = key_manager.signing_key()?;
        let libp2p_keypair = libp2p::identity::Keypair::ed25519_from_bytes(keypair.to_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to create libp2p keypair: {e}"))?;

        let peer_id = PeerId::from(libp2p_keypair.public());

        let kad_store = kad::store::MemoryStore::new(peer_id);
        let kademlia = kad::Behaviour::new(peer_id, kad_store);
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?;
        let identify = identify::Behaviour::new(identify::Config::new(
            "/vlkxn/1.0.0".into(),
            libp2p_keypair.public(),
        ));
        let ping = ping::Behaviour::new(ping::Config::new());

        let behaviour = VlkxnBehaviour {
            kademlia,
            mdns,
            identify,
            ping,
        };

        let mut swarm = SwarmBuilder::with_existing_identity(libp2p_keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|_| behaviour)?
            .build();

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        let node = Self {
            swarm,
            peer_id,
            event_tx,
            peers: HashMap::new(),
            _room,
        };

        Ok((node, event_rx))
    }

    pub async fn run(&mut self) {
        loop {
            let event = self.swarm.select_next_some().await;
            self.handle_event(event).await;
        }
    }

    async fn handle_event(&mut self, event: SwarmEvent<VlkxnBehaviourEvent>) {
        match event {
            SwarmEvent::Behaviour(VlkxnBehaviourEvent::Ping(ping_event)) => {
                match ping_event.result {
                    Ok(rtt) => {
                        if let Some(peer_info) = self.peers.get_mut(&ping_event.peer) {
                            peer_info.ping_ms = rtt.as_millis() as u16;
                        }
                    }
                    Err(e) => {
                        debug!("Ping failed to {}: {e}", ping_event.peer);
                    }
                }
            }
            SwarmEvent::Behaviour(VlkxnBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, _addr) in list {
                    info!("mDNS discovered: {peer_id}");
                    let info = PeerInfo {
                        node_id: peer_id.to_bytes(),
                        nickname: peer_id.to_string(),
                        virtual_ip: "0.0.0.0".parse().unwrap(),
                        ping_ms: 0,
                        connection_type: ConnectionType::Direct,
                    };
                    self.peers.insert(peer_id, info.clone());
                    let _ = self.event_tx.send(NetworkEvent::PeerConnected(info));
                }
            }
            SwarmEvent::Behaviour(VlkxnBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                for (peer_id, _addr) in list {
                    info!("mDNS expired: {peer_id}");
                    self.peers.remove(&peer_id);
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::PeerDisconnected(peer_id.to_bytes()));
                }
            }
            SwarmEvent::Behaviour(VlkxnBehaviourEvent::Identify(
                identify::Event::Received { ref info, .. },
            )) => {
                info!("Identified peer: {:?}", info.public_key);
            }
            SwarmEvent::Behaviour(VlkxnBehaviourEvent::Kademlia(
                kad::Event::RoutingUpdated { peer, .. },
            )) => {
                info!("Kademlia routing updated: {peer}");
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {address}");
            }
            _ => {
                debug!("Unhandled swarm event: {:?}", event);
            }
        }
    }

    pub fn peers(&self) -> &HashMap<PeerId, PeerInfo> {
        &self.peers
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }
}
