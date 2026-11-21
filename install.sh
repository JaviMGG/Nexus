#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# NEXUS · instalador interactivo
# Uso: ./install.sh            → menú (nativa / web / ambas)
#      ./install.sh --uninstall → elimina binarios y accesos (conserva tus datos)
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$HOME/.local/bin"
APPS_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/256x256/apps"
DATA_DIR="${NEXUS_DATA_DIR:-$HOME/.local/share/nexus}"
PORT="${NEXUS_PORT:-47800}"

BOLD=$'\033[1m'; DIM=$'\033[2m'; ACCENT=$'\033[35m'; OK=$'\033[32m'; WARN=$'\033[33m'; ERR=$'\033[31m'; RESET=$'\033[0m'
info()  { echo "${ACCENT}▸${RESET} $*"; }
ok()    { echo "${OK}✔${RESET} $*"; }
warn()  { echo "${WARN}!${RESET} $*"; }
die()   { echo "${ERR}✘${RESET} $*" >&2; exit 1; }
ask()   { local r; read -rp "$1 [s/N] " r || true; [[ "$r" =~ ^[sSyY] ]]; }

echo
echo "${BOLD}  NEXUS${RESET} ${DIM}· centro de control personal inteligente${RESET}"
echo

# ── desinstalación ───────────────────────────────────────────────────────────
if [[ "${1:-}" == "--uninstall" ]]; then
    info "Desinstalando NEXUS (tus datos NO se borran)…"
    if command -v systemctl >/dev/null && systemctl --user list-unit-files 2>/dev/null | grep -q nexus-web; then
        systemctl --user disable --now nexus-web.service 2>/dev/null || true
        rm -f "$HOME/.config/systemd/user/nexus-web.service"
        systemctl --user daemon-reload 2>/dev/null || true
    fi
    pkill -x nexus 2>/dev/null || true
    pkill -x nexus-server 2>/dev/null || true
    rm -f "$BIN_DIR/nexus" "$BIN_DIR/nexus-server"
    rm -f "$APPS_DIR/nexus.desktop" "$ICON_DIR/nexus.png"
    rm -rf "$DATA_DIR/web"
    command -v update-desktop-database >/dev/null && update-desktop-database "$APPS_DIR" 2>/dev/null || true
    ok "NEXUS desinstalado. Datos conservados en: $DATA_DIR y ~/.local/share/app.nexus.desktop"
    exit 0
fi

# ── requisitos comunes ───────────────────────────────────────────────────────
command -v cargo >/dev/null || die "Falta Rust/cargo. Instálalo: https://rustup.rs (curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh)"
command -v node  >/dev/null || die "Falta Node.js 20+. Instálalo desde https://nodejs.org o con tu gestor de paquetes."
command -v npm   >/dev/null || die "Falta npm (suele venir con Node.js)."
ok "Rust $(cargo --version | cut -d' ' -f2) · Node $(node --version)"

install_native_deps() {
    local missing=0
    for lib in webkit2gtk-4.1 gtk+-3.0 libsoup-3.0 librsvg-2.0; do
        pkg-config --exists "$lib" 2>/dev/null || missing=1
    done
    [[ $missing -eq 0 ]] && return 0
    warn "Faltan dependencias de escritorio (webkit2gtk-4.1, gtk3, libsoup3, librsvg)."
    local cmd=""
    if command -v pacman >/dev/null; then
        cmd="sudo pacman -S --needed webkit2gtk-4.1 gtk3 libsoup3 librsvg libayatana-appindicator"
    elif command -v apt-get >/dev/null; then
        cmd="sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev librsvg2-dev libayatana-appindicator3-1 build-essential curl wget file libssl-dev"
    elif command -v dnf >/dev/null; then
        cmd="sudo dnf install -y webkit2gtk4.1-devel gtk3-devel libsoup3-devel librsvg2-devel libayatana-appindicator-gtk3"
    fi
    if [[ -n "$cmd" ]] && ask "¿Instalarlas ahora? ($cmd)"; then
        eval "$cmd"
    else
        die "Instala las dependencias manualmente y vuelve a ejecutar ./install.sh"
    fi
}

build_frontend() {
    info "Compilando frontend (npm install + build)…"
    (cd "$ROOT/frontend" && npm install --no-audit --no-fund && npm run build)
    ok "Frontend compilado"
}

setup_env() {
    if [[ ! -f "$ROOT/.env" ]]; then
        cp "$ROOT/.env.example" "$ROOT/.env"
        warn "Creado .env desde .env.example. Para activar la IA, edita QWEN_API_KEY/QWEN_MODEL"
        warn "  (o hazlo luego desde Ajustes → Inteligencia Artificial dentro de la app)"
    fi
}

install_native() {
    install_native_deps
    build_frontend
    info "Compilando app nativa (release, puede tardar unos minutos)…"
    (cd "$ROOT" && cargo build --release -p nexus-desktop)
    mkdir -p "$BIN_DIR" "$APPS_DIR" "$ICON_DIR"
    cp "$ROOT/target/release/nexus-desktop" "$BIN_DIR/nexus.new"
    mv -f "$BIN_DIR/nexus.new" "$BIN_DIR/nexus"
    cp "$ROOT/crates/nexus-desktop/icons/icon.png" "$ICON_DIR/nexus.png"
    cat > "$APPS_DIR/nexus.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=NEXUS
GenericName=Centro de control personal
Comment=Segundo cerebro personal con tareas, proyectos, calendario, conocimiento y agentes de IA
Exec=$BIN_DIR/nexus
Icon=nexus
Terminal=false
Categories=Office;ProjectManagement;
Keywords=tareas;notas;proyectos;calendario;ia;agentes;productividad;
StartupWMClass=nexus-desktop
StartupNotify=true
EOF
    command -v update-desktop-database >/dev/null && update-desktop-database "$APPS_DIR" 2>/dev/null || true
    setup_env
    ok "App nativa instalada: $BIN_DIR/nexus"
    ok "Ya aparece en tu lanzador de aplicaciones (busca «NEXUS»)"
    echo "   ${DIM}Datos: ~/.local/share/app.nexus.desktop/${RESET}"
    if ask "¿Abrir NEXUS ahora?"; then
        setsid nohup "$BIN_DIR/nexus" >/dev/null 2>&1 < /dev/null &
        ok "NEXUS abierto"
    fi
}

install_web() {
    build_frontend
    info "Compilando servidor web (release)…"
    (cd "$ROOT" && cargo build --release -p nexus-api)
    mkdir -p "$BIN_DIR" "$DATA_DIR/web"
    cp "$ROOT/target/release/nexus-server" "$BIN_DIR/nexus-server.new"
    mv -f "$BIN_DIR/nexus-server.new" "$BIN_DIR/nexus-server"
    cp -r "$ROOT/frontend/dist/." "$DATA_DIR/web/"
    setup_env
    cp "$ROOT/.env" "$DATA_DIR/.env" 2>/dev/null || true
    ok "Servidor web instalado: $BIN_DIR/nexus-server"
    echo "   ${DIM}Datos: $DATA_DIR · Frontend servido desde: $DATA_DIR/web${RESET}"

    local started=0
    if command -v systemctl >/dev/null && systemctl --user show-environment >/dev/null 2>&1; then
        if ask "¿Instalar como servicio de usuario (arranque automático con systemd)?"; then
            mkdir -p "$HOME/.config/systemd/user"
            cat > "$HOME/.config/systemd/user/nexus-web.service" <<EOF
[Unit]
Description=NEXUS - centro de control personal (web local)
After=network.target

[Service]
ExecStart=$BIN_DIR/nexus-server
WorkingDirectory=$DATA_DIR
EnvironmentFile=-$DATA_DIR/.env
Environment=NEXUS_DATA_DIR=$DATA_DIR
Environment=NEXUS_PORT=$PORT
Restart=on-failure
RestartSec=3

[Install]
WantedBy=default.target
EOF
            systemctl --user daemon-reload
            systemctl --user enable --now nexus-web.service
            ok "Servicio nexus-web activo y habilitado en el arranque"
            started=1
        fi
    fi
    if [[ $started -eq 0 ]]; then
        echo
        info "Para arrancarlo manualmente:"
        echo "   ${BOLD}$BIN_DIR/nexus-server${RESET}"
        if ask "¿Arrancarlo ahora en segundo plano?"; then
            (cd "$DATA_DIR" && setsid nohup "$BIN_DIR/nexus-server" > "$DATA_DIR/server.log" 2>&1 < /dev/null &)
            started=1
            ok "Servidor arrancado (log: $DATA_DIR/server.log)"
        fi
    fi
    [[ $started -eq 1 ]] && ok "Abre ${BOLD}http://127.0.0.1:$PORT${RESET} en tu navegador"
}

# ── menú ─────────────────────────────────────────────────────────────────────
echo "  ¿Cómo quieres instalar NEXUS?"
echo
echo "    ${BOLD}1)${RESET} Nativa ${DIM}(app de escritorio, recomendada)${RESET}"
echo "    ${BOLD}2)${RESET} Web    ${DIM}(servidor local + navegador, ideal para servidor propio)${RESET}"
echo "    ${BOLD}3)${RESET} Ambas"
echo
read -rp "  Elige [1/2/3] (1): " choice || true
choice="${choice:-1}"
echo
case "$choice" in
    1) install_native ;;
    2) install_web ;;
    3) install_native; echo; install_web ;;
    *) die "Opción no válida: $choice" ;;
esac
echo
ok "¡Listo! Disfruta de NEXUS."
