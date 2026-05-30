use std::io::{Read, Write};
use std::net::IpAddr;
use std::process::Command;

use tokio::io::unix::AsyncFd;
use tracing::info;

pub struct TunInterface {
    #[cfg(target_os = "linux")]
    fd: Option<AsyncFd<std::fs::File>>,
    #[cfg(not(target_os = "linux"))]
    fd: Option<()>,
    name: String,
    virtual_ip: IpAddr,
}

impl TunInterface {
    pub fn new(name: &str) -> Self {
        Self {
            #[cfg(target_os = "linux")]
            fd: None,
            #[cfg(not(target_os = "linux"))]
            fd: None,
            name: name.to_string(),
            virtual_ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
        }
    }

    #[cfg(target_os = "linux")]
    pub async fn create(&mut self, ip: IpAddr, netmask: u8) -> anyhow::Result<()> {
        let file = create_tun_fd(&self.name)?;
        let async_fd = AsyncFd::new(file)?;

        let ip_str = match ip {
            IpAddr::V4(v4) => v4.to_string(),
            IpAddr::V6(_) => anyhow::bail!("IPv6 not yet supported"),
        };
        let prefix = netmask;

        Command::new("ip")
            .args(["addr", "add", &format!("{ip_str}/{prefix}"), "dev", &self.name])
            .status()?;

        Command::new("ip")
            .args(["link", "set", "dev", &self.name, "up"])
            .status()?;

        self.fd = Some(async_fd);
        self.virtual_ip = ip;

        info!("TUN interface {} created with IP {ip_str}/{prefix}", self.name);
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub async fn create(&mut self, _ip: IpAddr, _netmask: u8) -> anyhow::Result<()> {
        anyhow::bail!("TUN is only supported on Linux")
    }

    pub async fn read_packet(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        #[cfg(target_os = "linux")]
        {
            if let Some(ref fd) = self.fd {
                let guard = fd.readable().await?;
                return guard.get_inner().read(buf).map_err(Into::into);
            }
        }
        anyhow::bail!("TUN not initialized")
    }

    pub async fn write_packet(&mut self, buf: &[u8]) -> anyhow::Result<()> {
        #[cfg(target_os = "linux")]
        {
            if let Some(ref fd) = self.fd {
                let guard = fd.writable().await?;
                guard.get_inner().write_all(buf)?;
                return Ok(());
            }
        }
        anyhow::bail!("TUN not initialized")
    }

    pub fn virtual_ip(&self) -> IpAddr {
        self.virtual_ip
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(target_os = "linux")]
fn create_tun_fd(name: &str) -> anyhow::Result<std::fs::File> {
    use std::mem;
    use std::os::fd::{FromRawFd, IntoRawFd};

    let owned_fd = nix::fcntl::open(
        "/dev/net/tun",
        nix::fcntl::OFlag::O_RDWR,
        nix::sys::stat::Mode::empty(),
    )?;
    let fd = owned_fd.into_raw_fd();

    let mut ifreq: libc::ifreq = unsafe { mem::zeroed() };

    let name_bytes = name.as_bytes();
    let len = name_bytes.len().min(15);
    for (i, &b) in name_bytes[..len].iter().enumerate() {
        ifreq.ifr_name[i] = b as i8;
    }

    ifreq.ifr_ifru.ifru_flags = (libc::IFF_TUN | libc::IFF_NO_PI) as i16;

    let res = unsafe { libc::ioctl(fd, libc::TUNSETIFF, &ifreq as *const libc::ifreq) };

    if res < 0 {
        let e = std::io::Error::last_os_error();
        unsafe { libc::close(fd); }
        anyhow::bail!("Failed to create TUN interface: {e}");
    }

    unsafe { Ok(std::fs::File::from_raw_fd(fd)) }
}
