use std::collections::HashMap;

use futures::StreamExt;
use libp2p::{
    identify, kad, mdns,
    noise,
    ping,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, PeerId, StreamProtocol, Swarm, SwarmBuilder,
};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::crypto::KeyManager;
use crate::types::*;

pub const VLKXN_PROTOCOL: StreamProtocol = StreamProtocol::new("/vlkxn/data/1.0.0");

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
    pub packet_tx: mpsc::UnboundedSender<Vec<u8>>,
    peers: HashMap<PeerId, PeerInfo>,
    _room: String,
    virtual_ip: std::net::IpAddr,
}

impl P2pNode {
    pub async fn new(
        key_manager: &KeyManager,
    _room: String,
        virtual_ip: std::net::IpAddr,
    ) -> anyhow::Result<(Self, mpsc::UnboundedReceiver<NetworkEvent>)> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (packet_tx, _packet_rx) = mpsc::unbounded_channel();

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
            packet_tx,
            peers: HashMap::new(),
            _room: _room,
            virtual_ip,
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
                    self.add_peer(peer_id, ConnectionType::Direct);
                }
            }
            SwarmEvent::Behaviour(VlkxnBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                for (peer_id, _addr) in list {
                    info!("mDNS expired: {peer_id}");
                    self.remove_peer(&peer_id);
                }
            }
            SwarmEvent::Behaviour(VlkxnBehaviourEvent::Identify(
                identify::Event::Received { ref info, .. },
            )) => {
                debug!("Identified peer: {:?}", info.public_key);
            }
            SwarmEvent::Behaviour(VlkxnBehaviourEvent::Kademlia(
                kad::Event::RoutingUpdated { peer, .. },
            )) => {
                if !self.peers.contains_key(&peer) {
                    info!("Kademlia discovered new peer: {peer}");
                    self.add_peer(peer, ConnectionType::Direct);
                }
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {address}");
            }
            SwarmEvent::IncomingConnection { connection_id, local_addr, send_back_addr } => {
                debug!("Incoming connection: {connection_id} from {send_back_addr} on {local_addr}");
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                info!("Connection established: {peer_id} via {}", endpoint.get_remote_address());
                if !self.peers.contains_key(&peer_id) {
                    self.add_peer(peer_id, ConnectionType::Direct);
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                warn!("Connection closed: {peer_id}: {cause:?}");
                self.remove_peer(&peer_id);
            }
            _ => {
                debug!("Unhandled swarm event");
            }
        }
    }

    fn add_peer(&mut self, peer_id: PeerId, conn_type: ConnectionType) {
        if self.peers.contains_key(&peer_id) {
            return;
        }
        let info = PeerInfo {
            node_id: peer_id.to_bytes(),
            nickname: peer_id.to_string()[..8].to_string(),
            virtual_ip: self.virtual_ip,
            ping_ms: 0,
            connection_type: conn_type,
        };
        self.peers.insert(peer_id, info.clone());
        let _ = self.event_tx.send(NetworkEvent::PeerConnected(info));
    }

    fn remove_peer(&mut self, peer_id: &PeerId) {
        if self.peers.remove(peer_id).is_some() {
            let _ = self.event_tx.send(NetworkEvent::PeerDisconnected(peer_id.to_bytes()));
        }
    }

    pub fn peers(&self) -> &HashMap<PeerId, PeerInfo> {
        &self.peers
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn peer_list(&self) -> Vec<PeerInfo> {
        self.peers.values().cloned().collect()
    }
}
