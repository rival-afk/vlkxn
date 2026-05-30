#!/bin/bash
set -euo pipefail

REF_NAME="$1"
VERSION="${REF_NAME#v}"
TARGET="${2:-x86_64-unknown-linux-gnu}"
ASSET_NAME="${3:-linux-x86_64}"
PKG_ROOT="vlkxn_${VERSION}_amd64"

mkdir -p "${PKG_ROOT}/DEBIAN"
mkdir -p "${PKG_ROOT}/usr/bin"
mkdir -p "${PKG_ROOT}/usr/share/applications"
mkdir -p "${PKG_ROOT}/usr/share/icons/hicolor/scalable/apps"

cat > "${PKG_ROOT}/DEBIAN/control" << CONTROL
Package: vlkxn
Version: ${VERSION}
Section: net
Priority: optional
Architecture: amd64
Maintainer: Vlkxn Contributors
Description: Decentralized P2P VPN for gaming
 Built with Rust + libp2p.
 Provides a virtual network adapter (TUN) for peer-to-peer
 gaming connections with automatic peer discovery.
CONTROL

cp "target/${TARGET}/release/vlkxn-cli" "${PKG_ROOT}/usr/bin/"
cp "target/${TARGET}/release/vlkxn-gui" "${PKG_ROOT}/usr/bin/" 2>/dev/null || true

cat > "${PKG_ROOT}/usr/share/applications/vlkxn.desktop" << DESKTOP
[Desktop Entry]
Name=Vlkxn
Comment=Decentralized P2P VPN for Gaming
Exec=/usr/bin/vlkxn-gui
Icon=vlkxn
Terminal=false
Type=Application
Categories=Network;Game;
DESKTOP

if [ -f scripts/vlkxn-icon.svg ]; then
  cp scripts/vlkxn-icon.svg "${PKG_ROOT}/usr/share/icons/hicolor/scalable/apps/"
fi

cat > "${PKG_ROOT}/DEBIAN/postinst" << 'POSTINST'
#!/bin/sh
set -e
if command -v setcap >/dev/null 2>&1; then
  setcap cap_net_admin+ep /usr/bin/vlkxn-cli 2>/dev/null || true
  setcap cap_net_admin+ep /usr/bin/vlkxn-gui 2>/dev/null || true
fi
update-desktop-database /usr/share/applications 2>/dev/null || true
exit 0
POSTINST
chmod 755 "${PKG_ROOT}/DEBIAN/postinst"

dpkg-deb --build "${PKG_ROOT}"
mv "vlkxn_${VERSION}_amd64.deb" "vlkxn-${REF_NAME}-${ASSET_NAME}.deb"
echo "DEB=vlkxn-${REF_NAME}-${ASSET_NAME}.deb" >> $GITHUB_ENV
