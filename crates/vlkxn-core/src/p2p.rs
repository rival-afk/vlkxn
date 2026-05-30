use std::collections::HashMap;

use async_trait::async_trait;
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use futures::StreamExt;
use libp2p::{
    dcutr, identify, kad, mdns, noise, ping, relay,
    request_response::{self, Codec, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, PeerId, StreamProtocol, Swarm, SwarmBuilder,
};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::crypto::KeyManager;
use crate::types::*;

pub const VLKXN_PROTOCOL: StreamProtocol = StreamProtocol::new("/vlkxn/data/1.0.0");

#[derive(Clone, Default)]
pub struct BytesCodec;

#[async_trait]
impl Codec for BytesCodec {
    type Protocol = StreamProtocol;
    type Request = Vec<u8>;
    type Response = Vec<u8>;

    async fn read_request<T>(&mut self, _protocol: &Self::Protocol, io: &mut T) -> std::io::Result<Self::Request>
    where T: AsyncRead + Unpin + Send,
    {
        let mut len_buf = [0u8; 4];
        io.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf);
        let mut buf = vec![0u8; len as usize];
        io.read_exact(&mut buf).await?;
        Ok(buf)
    }

    async fn read_response<T>(&mut self, _protocol: &Self::Protocol, io: &mut T) -> std::io::Result<Self::Response>
    where T: AsyncRead + Unpin + Send,
    {
        let mut len_buf = [0u8; 4];
        io.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf);
        let mut buf = vec![0u8; len as usize];
        io.read_exact(&mut buf).await?;
        Ok(buf)
    }

    async fn write_request<T>(&mut self, _protocol: &Self::Protocol, io: &mut T, req: Self::Request) -> std::io::Result<()>
    where T: AsyncWrite + Unpin + Send,
    {
        let len = (req.len() as u32).to_be_bytes();
        io.write_all(&len).await?;
        io.write_all(&req).await?;
        Ok(())
    }

    async fn write_response<T>(&mut self, _protocol: &Self::Protocol, io: &mut T, res: Self::Response) -> std::io::Result<()>
    where T: AsyncWrite + Unpin + Send,
    {
        let len = (res.len() as u32).to_be_bytes();
        io.write_all(&len).await?;
        io.write_all(&res).await?;
        Ok(())
    }
}

#[derive(NetworkBehaviour)]
pub struct VlkxnBehaviour {
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub mdns: mdns::tokio::Behaviour,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub relay: relay::client::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub data: request_response::Behaviour<BytesCodec>,
}

pub struct P2pNode {
    pub swarm: Swarm<VlkxnBehaviour>,
    pub peer_id: PeerId,
    pub event_tx: mpsc::UnboundedSender<NetworkEvent>,
    pub packet_tx: mpsc::UnboundedSender<Vec<u8>>,
    pub packet_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    peers: HashMap<PeerId, PeerInfo>,
    _room: String,
    virtual_ip: std::net::IpAddr,
    pending_requests: HashMap<request_response::OutboundRequestId, PeerId>,
}

impl P2pNode {
    pub async fn new(
        key_manager: &KeyManager,
        _room: String,
        virtual_ip: std::net::IpAddr,
    ) -> anyhow::Result<(Self, mpsc::UnboundedReceiver<NetworkEvent>)> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (packet_tx, packet_rx) = mpsc::unbounded_channel();

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
        let dcutr = dcutr::Behaviour::new(peer_id);
        let data = request_response::Behaviour::new(
            vec![(VLKXN_PROTOCOL, ProtocolSupport::Full)],
            request_response::Config::default(),
        );

        let mut swarm = SwarmBuilder::with_existing_identity(libp2p_keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_relay_client(noise::Config::new, yamux::Config::default)?
            .with_behaviour(|_key, relay| VlkxnBehaviour {
                kademlia,
                mdns,
                identify,
                ping,
                relay,
                dcutr,
                data,
            })?
            .build();

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        let node = Self {
            swarm,
            peer_id,
            event_tx,
            packet_tx,
            packet_rx,
            peers: HashMap::new(),
            _room,
            virtual_ip,
            pending_requests: HashMap::new(),
        };

        Ok((node, event_rx))
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    self.handle_event(event).await;
                }
                Some(packet) = self.packet_rx.recv() => {
                    self.broadcast_data(packet);
                }
            }
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
                    Err(e) => debug!("Ping failed to {}: {e}", ping_event.peer),
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
            SwarmEvent::Behaviour(VlkxnBehaviourEvent::Relay(event)) => {
                match event {
                    relay::client::Event::ReservationReqAccepted { relay_peer_id, .. } => {
                        info!("Relay reservation accepted by {relay_peer_id}");
                    }
                    relay::client::Event::OutboundCircuitEstablished { relay_peer_id, .. } => {
                        info!("Outbound circuit via {relay_peer_id}");
                    }
                    relay::client::Event::InboundCircuitEstablished { src_peer_id, .. } => {
                        info!("Inbound circuit from {src_peer_id}");
                    }
                }
            }
            SwarmEvent::Behaviour(VlkxnBehaviourEvent::Dcutr(event)) => {
                debug!("DCUtR: {event:?}");
            }
            SwarmEvent::Behaviour(VlkxnBehaviourEvent::Data(event)) => {
                self.handle_data_event(event);
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {address}");
            }
            SwarmEvent::IncomingConnection { connection_id, local_addr, send_back_addr } => {
                debug!("Incoming connection: {connection_id} from {send_back_addr} on {local_addr}");
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                info!("Connection: {peer_id} via {}", endpoint.get_remote_address());
                let conn_type = if endpoint.is_relayed() { ConnectionType::Relay } else { ConnectionType::Direct };
                if !self.peers.contains_key(&peer_id) {
                    self.add_peer(peer_id, conn_type);
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

    fn handle_data_event(&mut self, event: request_response::Event<Vec<u8>, Vec<u8>>) {
        match event {
            request_response::Event::Message { peer, message } => {
                match message {
                    request_response::Message::Request { request, channel, .. } => {
                        let n = request.len();
                        debug!("Data from {peer}: {n} bytes");
                        let _ = self.swarm.behaviour_mut().data.send_response(channel, Vec::new());
                        let _ = self.event_tx.send(NetworkEvent::PacketReceived(PacketData {
                            from: peer.to_bytes(),
                            data: request,
                        }));
                    }
                    request_response::Message::Response { request_id, response: _ } => {
                        debug!("Data response {request_id}");
                        self.pending_requests.remove(&request_id);
                    }
                }
            }
            request_response::Event::InboundFailure { peer, request_id: _, error } => {
                warn!("Inbound failure from {peer}: {error}");
            }
            request_response::Event::OutboundFailure { peer, request_id, error } => {
                warn!("Outbound failure to {peer}: {error}");
                self.pending_requests.remove(&request_id);
            }
            request_response::Event::ResponseSent { peer, request_id } => {
                debug!("Response sent to {peer}: {request_id}");
            }
        }
    }

    pub fn send_data(&mut self, peer: &PeerId, data: Vec<u8>) -> anyhow::Result<()> {
        let request_id = self.swarm.behaviour_mut().data.send_request(peer, data);
        self.pending_requests.insert(request_id, *peer);
        Ok(())
    }

    pub fn broadcast_data(&mut self, data: Vec<u8>) {
        let peers: Vec<PeerId> = self.peers.keys().copied().collect();
        for peer in peers {
            let _ = self.send_data(&peer, data.clone());
        }
    }

    fn add_peer(&mut self, peer_id: PeerId, conn_type: ConnectionType) {
        if self.peers.contains_key(&peer_id) {
            if let Some(peer) = self.peers.get_mut(&peer_id) {
                peer.connection_type = conn_type;
            }
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
