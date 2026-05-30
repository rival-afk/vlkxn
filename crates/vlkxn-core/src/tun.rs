use std::net::IpAddr;
use tracing::info;

#[cfg(unix)]
mod platform {
    use std::io::{Read, Write};
    use std::os::fd::{FromRawFd, IntoRawFd};
    use std::process::Command;

    use tokio::io::unix::AsyncFd;

    pub struct TunFd(AsyncFd<std::fs::File>);

    impl TunFd {
        pub fn create(name: &str) -> anyhow::Result<Self> {
            let owned_fd = nix::fcntl::open(
                "/dev/net/tun",
                nix::fcntl::OFlag::O_RDWR,
                nix::sys::stat::Mode::empty(),
            )?;
            let fd = owned_fd.into_raw_fd();

            let mut ifreq: libc::ifreq = unsafe { std::mem::zeroed() };

            let name_bytes = name.as_bytes();
            let len = name_bytes.len().min(15);
            for (i, &b) in name_bytes[..len].iter().enumerate() {
                ifreq.ifr_name[i] = b as i8;
            }

            ifreq.ifr_ifru.ifru_flags = (libc::IFF_TUN | libc::IFF_NO_PI) as i16;

            let res = unsafe { libc::ioctl(fd, libc::TUNSETIFF, &ifreq as *const libc::ifreq) };
            if res < 0 {
                let e = std::io::Error::last_os_error();
                unsafe { libc::close(fd) };
                anyhow::bail!("Failed to create TUN: {e}");
            }

            let file = unsafe { std::fs::File::from_raw_fd(fd) };
            Ok(Self(AsyncFd::new(file)?))
        }

        pub fn set_ip(name: &str, ip: &str, netmask: u8) -> anyhow::Result<()> {
            Command::new("ip")
                .args(["addr", "add", &format!("{ip}/{netmask}"), "dev", name])
                .status()?;
            Command::new("ip")
                .args(["link", "set", "dev", name, "up"])
                .status()?;
            Command::new("ip")
                .args(["route", "add", crate::types::VIRTUAL_NETWORK, "dev", name])
                .status()?;
            Ok(())
        }

        pub async fn read(&self, buf: &mut [u8]) -> anyhow::Result<usize> {
            let guard = self.0.readable().await?;
            Ok(guard.get_inner().read(buf)?)
        }

        pub async fn write(&self, buf: &[u8]) -> anyhow::Result<()> {
            let guard = self.0.writable().await?;
            guard.get_inner().write_all(buf)?;
            Ok(())
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::process::Command;
    use std::sync::Arc;

    use tokio::sync::Mutex;

    pub struct TunFd {
        _adapter: Arc<wintun::Adapter>,
        session: Arc<wintun::Session>,
        read_buf: Arc<Mutex<Vec<u8>>>,
        read_ready: Arc<tokio::sync::Notify>,
    }

    impl TunFd {
        pub fn create(name: &str) -> anyhow::Result<Self> {
            let wintun = unsafe { wintun::load() }
                .map_err(|e| anyhow::anyhow!("wintun.dll not found: {e}"))?;

            let guid = uuid::Uuid::new_v4().as_u128();
            let adapter = wintun::Adapter::create(&wintun, name, "Vlkxn", Some(guid))?;
            let session = Arc::new(adapter.start_session(wintun::MAX_RING_CAPACITY)?);

            let read_buf = Arc::new(Mutex::new(Vec::new()));
            let read_ready = Arc::new(tokio::sync::Notify::new());
            let s = session.clone();
            let rb = read_buf.clone();
            let rr = read_ready.clone();

            tokio::task::spawn_blocking(move || {
                loop {
                    match s.receive_blocking() {
                        Ok(packet) => {
                            let bytes = packet.bytes().to_vec();
                            let mut buf = rb.blocking_lock();
                            *buf = bytes;
                            rr.notify_one();
                        }
                        Err(_) => break,
                    }
                }
            });

            Ok(Self {
                _adapter: adapter,
                session,
                read_buf,
                read_ready,
            })
        }

        pub fn set_ip(name: &str, ip: &str, netmask: u8) -> anyhow::Result<()> {
            let mask = u32::MAX
                .checked_shl(32 - netmask as u32)
                .unwrap_or(0)
                .to_be_bytes();
            let mask_str = format!("{}.{}.{}.{}", mask[0], mask[1], mask[2], mask[3]);

            Command::new("netsh")
                .args([
                    "interface",
                    "ip",
                    "set",
                    "address",
                    &format!("name=\"{name}\""),
                    "static",
                    ip,
                    &mask_str,
                ])
                .status()?;

            Command::new("netsh")
                .args([
                    "interface",
                    "ip",
                    "set",
                    "interface",
                    &format!("name=\"{name}\""),
                    "admin=enabled",
                ])
                .status()?;

            Ok(())
        }

        pub async fn read(&self, buf: &mut [u8]) -> anyhow::Result<usize> {
            self.read_ready.notified().await;
            let mut rb = self.read_buf.lock().await;
            let n = rb.len().min(buf.len());
            buf[..n].copy_from_slice(&rb[..n]);
            rb.clear();
            Ok(n)
        }

        pub async fn write(&self, buf: &[u8]) -> anyhow::Result<()> {
            let mut packet = self.session.allocate_send_packet(buf.len() as u16)?;
            packet.bytes_mut().copy_from_slice(buf);
            self.session.send_packet(packet);
            Ok(())
        }
    }
}

pub struct TunInterface {
    fd: Option<platform::TunFd>,
    name: String,
    virtual_ip: IpAddr,
}

impl TunInterface {
    pub fn new(name: &str) -> Self {
        Self {
            fd: None,
            name: name.to_string(),
            virtual_ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
        }
    }

    pub async fn create(&mut self, ip: IpAddr, netmask: u8) -> anyhow::Result<()> {
        let ip_str = match ip {
            IpAddr::V4(v4) => v4.to_string(),
            IpAddr::V6(_) => anyhow::bail!("IPv6 not yet supported"),
        };

        let tun = platform::TunFd::create(&self.name)?;
        platform::TunFd::set_ip(&self.name, &ip_str, netmask)?;

        self.fd = Some(tun);
        self.virtual_ip = ip;

        info!("TUN {} created with IP {ip_str}/{netmask}", self.name);
        Ok(())
    }

    pub async fn read_packet(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        match self.fd.as_ref() {
            Some(fd) => fd.read(buf).await,
            None => anyhow::bail!("TUN not initialized"),
        }
    }

    pub async fn write_packet(&mut self, buf: &[u8]) -> anyhow::Result<()> {
        match self.fd.as_ref() {
            Some(fd) => fd.write(buf).await,
            None => anyhow::bail!("TUN not initialized"),
        }
    }

    pub fn virtual_ip(&self) -> IpAddr {
        self.virtual_ip
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
