#!/usr/bin/env bash
set -euo pipefail

BOLD='\033[1m'
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BIN_DIR="${BIN_DIR:-/usr/local/bin}"
DATA_DIR="${DATA_DIR:-/usr/local/share/vlkxn}"
ICON_DIR="${ICON_DIR:-/usr/local/share/icons/hicolor/scalable/apps}"
APP_DIR="${APP_DIR:-/usr/local/share/applications}"
SYSTEMD_DIR="${SYSTEMD_DIR:-/usr/local/lib/systemd/user}"
CONFIG_DIR="${CONFIG_DIR:-$HOME/.config/vlkxn}"

print_banner() {
    echo -e "${RED}"
    echo '╔══════════════════════════════════╗'
    echo '║   🌋 Vlkxn Linux Installer       ║'
    echo '╚══════════════════════════════════╝'
    echo -e "${NC}"
}

check_root() {
    if [[ $EUID -ne 0 ]]; then
        echo -e "${YELLOW}⚠️  Некоторые шаги требуют root-прав. Используйте sudo.${NC}"
        echo -e "${CYAN}   Запустите: sudo $0${NC}"
        echo
        echo -e "${CYAN}   Или установите для пользователя: INSTALL_USER=1 $0${NC}"
        echo
        return 1
    fi
    return 0
}

install_binary() {
    echo -e "${YELLOW}>> Установка бинарников...${NC}"

    local bin_src="$PROJECT_DIR/target/release/vlkxn-cli"
    if [[ -f "$bin_src" ]]; then
        install -m 755 "$bin_src" "$BIN_DIR/vlkxn"
        echo -e "   ${GREEN}[+]${NC} vlkxn → $BIN_DIR/vlkxn"
    else
        echo -e "   ${RED}[!]${NC} Сначала соберите проект: cargo build --release"
        exit 1
    fi

    local gui_src="$PROJECT_DIR/target/release/vlkxn-gui"
    if [[ -f "$gui_src" ]]; then
        install -m 755 "$gui_src" "$BIN_DIR/vlkxn-gui"
        echo -e "   ${GREEN}[+]${NC} vlkxn-gui → $BIN_DIR/vlkxn-gui"
    fi
}

set_capabilities() {
    echo -e "${YELLOW}>> Установка capabilities (CAP_NET_ADMIN)...${NC}"
    
    if setcap cap_net_admin+ep "$BIN_DIR/vlkxn" 2>/dev/null; then
        echo -e "   ${GREEN}[+]${NC} CAP_NET_ADMIN установлен для vlkxn"
    else
        echo -e "   ${RED}[!]${NC} Не удалось установить capabilities."
        echo -e "   ${CYAN}   Попробуйте: sudo setcap cap_net_admin+ep $BIN_DIR/vlkxn${NC}"
    fi

    if [[ -f "$BIN_DIR/vlkxn-gui" ]]; then
        setcap cap_net_admin+ep "$BIN_DIR/vlkxn-gui" 2>/dev/null || true
    fi
}

install_icon() {
    echo -e "${YELLOW}>> Установка иконки...${NC}"
    
    local icon_src="$SCRIPT_DIR/vlkxn-icon.svg"
    if [[ -f "$icon_src" ]]; then
        mkdir -p "$ICON_DIR"
        cp "$icon_src" "$ICON_DIR/vlkxn.svg"
        echo -e "   ${GREEN}[+]${NC} Иконка установлена"
        gtk-update-icon-cache /usr/local/share/icons/hicolor/ 2>/dev/null || true
    fi
}

install_desktop() {
    echo -e "${YELLOW}>> Установка .desktop файла...${NC}"
    
    mkdir -p "$APP_DIR"
    cat > "$APP_DIR/vlkxn.desktop" << EOF
[Desktop Entry]
Name=Vlkxn
Comment=Decentralized P2P VPN for Gaming
Exec=$BIN_DIR/vlkxn-gui
Icon=vlkxn
Terminal=false
Type=Application
Categories=Network;Game;
StartupNotify=true
EOF
    echo -e "   ${GREEN}[+]${NC} .desktop файл установлен в $APP_DIR/vlkxn.desktop"
}

install_systemd() {
    echo -e "${YELLOW}>> Установка systemd user service...${NC}"
    
    mkdir -p "$SYSTEMD_DIR"
    cat > "$SYSTEMD_DIR/vlkxn-daemon.service" << EOF
[Unit]
Description=Vlkxn P2P VPN Daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=$BIN_DIR/vlkxn up
ExecStop=$BIN_DIR/vlkxn down
Restart=on-failure
RestartSec=5
AmbientCapabilities=CAP_NET_ADMIN

[Install]
WantedBy=default.target
EOF
    echo -e "   ${GREEN}[+]${NC} systemd юнит установлен"
    echo -e "   ${CYAN}   Активируйте: systemctl --user enable --now vlkxn-daemon${NC}"
}

create_config() {
    echo -e "${YELLOW}>> Создание конфигурации...${NC}"
    mkdir -p "$CONFIG_DIR"
    
    if [[ ! -f "$CONFIG_DIR/config.toml" ]]; then
        cat > "$CONFIG_DIR/config.toml" << 'EOF'
[nickname]
value = ""

[network]
room = "public"
autostart = false

[relay]
enable = true
max_relay_connections = 8

[bandwidth]
broadcast_limit = 10
max_peers = 64

[advanced]
virtual_ip_range = "10.144.0.0/16"
use_dht = true
hole_punch_timeout_sec = 5
EOF
        echo -e "   ${GREEN}[+]${NC} Конфиг создан: $CONFIG_DIR/config.toml"
    else
        echo -e "   ${CYAN}[~]${NC} Конфиг уже существует"
    fi
}

post_install_msg() {
    echo
    echo -e "${GREEN}╔══════════════════════════════════╗${NC}"
    echo -e "${GREEN}║   ✅ Vlkxn успешно установлен!   ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════╝${NC}"
    echo
    echo -e "${CYAN}Запустите:${NC}"
    echo -e "   ${BOLD}vlkxn${NC} --help        — CLI"
    echo -e "   ${BOLD}vlkxn-gui${NC}            — GUI"
    echo
    echo -e "${CYAN}Или найдите Vlkxn в списке приложений.${NC}"
    echo
    echo -e "${CYAN}Для автозапуска демона:${NC}"
    echo -e "   systemctl --user enable --now vlkxn-daemon"
    echo
}

# Main
print_banner

if [[ "${INSTALL_USER:-}" == "1" ]]; then
    BIN_DIR="$HOME/.local/bin"
    ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
    APP_DIR="$HOME/.local/share/applications"
    SYSTEMD_DIR="$HOME/.local/share/systemd/user"
    mkdir -p "$BIN_DIR" "$ICON_DIR" "$APP_DIR" "$SYSTEMD_DIR"
fi

install_binary
set_capabilities
install_icon
install_desktop
install_systemd
create_config
post_install_msg
