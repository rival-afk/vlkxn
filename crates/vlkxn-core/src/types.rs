use std::net::{IpAddr, SocketAddr};

pub type NodeId = Vec<u8>;
pub type RoomName = String;
pub type Nickname = String;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub nickname: Nickname,
    pub virtual_ip: IpAddr,
    pub public_endpoint: Option<SocketAddr>,
    pub is_relay: bool,
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub nickname: Nickname,
    pub virtual_ip: IpAddr,
    pub ping_ms: u16,
    pub connection_type: ConnectionType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Direct,
    Relay,
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    PeerConnected(PeerInfo),
    PeerDisconnected(NodeId),
    VirtualIpAssigned(IpAddr),
    PacketReceived(Vec<u8>),
}

pub const VIRTUAL_NETWORK: &str = "10.144.0.0/16";
pub const VIRTUAL_NETWORK_PREFIX: u8 = 16;
