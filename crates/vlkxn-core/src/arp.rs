use std::collections::HashMap;
use std::net::IpAddr;



pub struct ArpProxy {
    ip_to_mac: HashMap<IpAddr, [u8; 6]>,
}

impl Default for ArpProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl ArpProxy {
    pub fn new() -> Self {
        Self {
            ip_to_mac: HashMap::new(),
        }
    }

    pub fn register_ip(&mut self, ip: IpAddr) -> [u8; 6] {
        let mac = self.ip_to_mac.entry(ip).or_insert_with(|| match ip {
            IpAddr::V4(v4) => {
                let octets = v4.octets();
                [0x02, 0x00, 0x00, octets[2], octets[3], octets[1]]
            }
            IpAddr::V6(_) => [0x02, 0x00, 0x00, 0x00, 0x00, 0x01],
        });
        *mac
    }

    pub fn resolve_mac(&self, ip: &IpAddr) -> Option<[u8; 6]> {
        self.ip_to_mac.get(ip).copied()
    }

    pub fn remove_ip(&mut self, ip: &IpAddr) {
        self.ip_to_mac.remove(ip);
    }

    pub fn handle_arp_request(&self, target_ip: IpAddr) -> Option<Vec<u8>> {
        if let Some(mac) = self.resolve_mac(&target_ip) {
            let mut packet = Vec::with_capacity(42);

            packet.extend_from_slice(&mac);
            packet.extend_from_slice(&[0x00; 4]);

            packet.extend_from_slice(&[0x08, 0x06]);
            packet.extend_from_slice(&[0x00, 0x01]);
            packet.extend_from_slice(&[0x08, 0x00]);
            packet.extend_from_slice(&[0x06]);
            packet.extend_from_slice(&[0x04]);
            packet.extend_from_slice(&[0x00, 0x02]);
            packet.extend_from_slice(&mac);
            packet.extend_from_slice(&[0x00; 4]);
            match target_ip {
                IpAddr::V4(v4) => packet.extend_from_slice(&v4.octets()),
                IpAddr::V6(_) => return None,
            };

            Some(packet)
        } else {
            None
        }
    }

    pub fn is_virtual_ip(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => v4.octets()[0] == 10 && v4.octets()[1] == 144,
            IpAddr::V6(_) => false,
        }
    }
}
