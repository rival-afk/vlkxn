<div align="center">
  <h1>🌋 Vlkxn (Вулкан)</h1>
  <p><strong>Decentralized P2P VPN for Gaming</strong></p>
  <p>Replace Hamachi / Radmin VPN with zero central server — just press a button and you're in the network.</p>

  <p>
    <img src="https://img.shields.io/badge/status-alpha-orange" alt="Status">
    <img src="https://img.shields.io/badge/platform-Linux%20%7C%20Windows%2011-blue" alt="Platform">
    <img src="https://img.shields.io/badge/license-GPLv3-green" alt="License">
    <img src="https://img.shields.io/github/v/release/rival-afk/vlkxn" alt="Release">
    <img src="https://img.shields.io/badge/Rust-1.95%2B-purple" alt="Rust">
  </p>
</div>

---

## 🔥 Features

- **One-click connect** — `vlkxn up --room MyGame` and you're in
- **P2P architecture** — no central servers, direct connections via UDP hole punching
- **Automatic virtual IP** — each node gets a unique IP in `10.144.0.0/16`
- **Peer discovery** — mDNS (LAN) + Kademlia DHT (WAN)
- **End-to-end encryption** — Noise Protocol (Noise IK) + Ed25519 identity keys
- **Virtual TUN adapter** — works with any game that supports LAN (Minecraft, Warcraft III, etc.)
- **Broadcast proxy** — LAN game discovery across the virtual network
- **Cross-platform** — Linux (CLI), Windows 11 (GUI planned)

## 🚀 Quick Start

### Linux

```bash
# Start the VPN and join a room
vlkxn up --room MyGame --nick Player1

# Check status
vlkxn status

# List online peers
vlkxn list

# Disconnect
vlkxn down
```

### Requirements

| Dependency | Linux | Windows |
|-----------|-------|---------|
| Rust 1.95+ | ✅ | ✅ |
| TUN/TAP | `/dev/net/tun` | wintun.dll |
| Permissions | `CAP_NET_ADMIN` | Admin rights |

## 🏗️ Architecture

```
┌──────────────────────────────────────────┐
│           CLI (vlkxn-cli)                │
└──────────────────┬───────────────────────┘
                   │ IPC
┌──────────────────▼───────────────────────┐
│       vlkxn-controller (daemon)          │
│  - lifecycle management                  │
│  - config & logging                      │
└──────────────────┬───────────────────────┘
                   │
┌──────────────────▼───────────────────────┐
│          vlkxn-core (library)            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐  │
│  │ P2P      │ │ TUN/TAP  │ │ Crypto   │  │
│  │ (libp2p) │ │ (Linux)  │ │ (Noise)  │  │
│  │ DHT      │ │ wintun   │ │ Ed25519  │  │
│  │ mDNS     │ │ (Win)    │ │ Key mgmt │  │
│  └──────────┘ └──────────┘ └──────────┘  │
│  ┌─────────────────────────────────────┐ │
│  │ ARP / Broadcast Proxy               │ │
│  └─────────────────────────────────────┘ │
└──────────────────────────────────────────┘
```

## 📦 Project Structure

```
vlkxn/
├── crates/
│   ├── vlkxn-core/        # Core library (P2P, TUN, crypto, config)
│   ├── vlkxn-controller/  # Background daemon process
│   └── vlkxn-cli/         # Command-line interface
├── Cargo.toml             # Workspace definition
└── README.md
```

## 🛠️ Build from Source

```bash
git clone https://github.com/rival-afk/vlkxn.git
cd vlkxn
cargo build --release
./target/release/vlkxn-cli --help
```

## 📋 Development Roadmap

| Phase | Timeline | Status |
|-------|----------|--------|
| **Core** — libp2p DHT, TUN, crypto | Month 1–2 | 🚧 In Progress |
| **Windows & CLI** — wintun, cross-platform CLI | Month 2–3 | 📅 Planned |
| **Virtual LAN** — ARP, broadcast, relay fallback | Month 3–5 | 📅 Planned |
| **GUI** — WinUI 3 (Win), GTK4/egui (Linux) | Month 5–7 | 📅 Planned |
| **Distribution** — MSI, deb, rpm, AppImage | Month 7–8 | 📅 Planned |

## 🤝 Contributing

Contributions are welcome! Feel free to open issues and pull requests.

1. Create a feature branch (`git checkout -b feat/amazing`)
2. Commit your changes (`git commit -m 'Add amazing feature'`)
3. Push to the branch (`git push origin feat/amazing`)
4. Open a Pull Request

## 📄 License

[GNU General Public License v3.0](LICENSE) — see [LICENSE](LICENSE) for details.
