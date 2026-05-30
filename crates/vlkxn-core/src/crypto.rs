use std::path::PathBuf;

use ed25519_dalek::{SecretKey, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPair {
    pub secret: Vec<u8>,
    public: Vec<u8>,
}

impl KeyPair {
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let mut seed = [0u8; 32];
        use rand::RngCore;
        RngCore::fill_bytes(&mut csprng, &mut seed);
        let secret = SecretKey::from(seed);
        let signing_key = SigningKey::from_bytes(&secret);
        let verifying_key = signing_key.verifying_key();
        Self {
            secret: signing_key.to_bytes().to_vec(),
            public: verifying_key.to_bytes().to_vec(),
        }
    }

    pub fn public_key(&self) -> [u8; 32] {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&self.public);
        arr
    }

    pub fn signing_key(&self) -> anyhow::Result<SigningKey> {
        let secret = SecretKey::from(<[u8; 32]>::try_from(self.secret.as_slice())?);
        Ok(SigningKey::from_bytes(&secret))
    }

    pub fn verifying_key(&self) -> anyhow::Result<VerifyingKey> {
        let bytes: [u8; 32] = self.public[..32].try_into()?;
        Ok(VerifyingKey::from_bytes(&bytes)?)
    }
}

pub struct KeyManager {
    keypair: KeyPair,
}

impl KeyManager {
    pub fn load_or_generate(keys_path: &PathBuf) -> anyhow::Result<Self> {
        let keypair = if keys_path.exists() {
            let content = std::fs::read_to_string(keys_path)?;
            serde_json::from_str(&content)?
        } else {
            let kp = KeyPair::generate();
            if let Some(parent) = keys_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(keys_path, serde_json::to_string_pretty(&kp)?)?;
            kp
        };
        Ok(Self { keypair })
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.keypair.public_key()
    }

    pub fn signing_key(&self) -> anyhow::Result<SigningKey> {
        self.keypair.signing_key()
    }

    pub fn verifying_key(&self) -> anyhow::Result<VerifyingKey> {
        self.keypair.verifying_key()
    }

    pub fn keypair(&self) -> &KeyPair {
        &self.keypair
    }
}

pub fn virtual_ip_from_public_key(public_key: &[u8; 32]) -> std::net::IpAddr {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(public_key);
    let suffix = u16::from_be_bytes([hash[0], hash[1]]);
    let ip = std::net::Ipv4Addr::new(10, 144, (suffix >> 8) as u8, (suffix & 0xff) as u8);
    std::net::IpAddr::V4(ip)
}
