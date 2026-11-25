#!/usr/bin/env sh
# clin-tauri installer — downloads the pre-built Tauri bundle for your platform.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/your-org/clin-tauri/main/install.sh | sh
#   sh install.sh [--version 0.1.0] [--desktop]
#
# On Linux : installs the binary to ~/.local/bin and registers an XDG .desktop
#            entry in ~/.local/share/applications/ (app menu in GNOME/KDE/etc.).
#            Pass --desktop to also drop a shortcut on ~/Desktop.
# On macOS : copies the .app bundle to /Applications (Spotlight/Launchpad pick
#            it up automatically — no extra shortcut needed).

set -e

REPO="your-org/clin-tauri"
REQUESTED_VERSION="${CLIN_VERSION:-latest}"
ADD_DESKTOP_SHORTCUT=0

if [ -t 1 ]; then
  GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'
else
  GREEN=''; YELLOW=''; RED=''; NC=''
fi
info()  { printf "${GREEN}[clin]${NC} %s\n" "$*"; }
warn()  { printf "${YELLOW}[clin]${NC} %s\n" "$*"; }
error() { printf "${RED}[clin]${NC} %s\n" "$*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
while [ $# -gt 0 ]; do
  case "$1" in
    --version)  REQUESTED_VERSION="$2"; shift 2 ;;
    --desktop)  ADD_DESKTOP_SHORTCUT=1; shift ;;
    -h|--help)
      printf 'Usage: install.sh [--version VERSION] [--desktop]\n'
      printf '\n'
      printf '  --version VERSION   Install a specific release tag (default: latest).\n'
      printf '  --desktop           Also create a shortcut on ~/Desktop (Linux only).\n'
      exit 0
      ;;
    *) error "Unknown flag: $1" ;;
  esac
done

# ---------------------------------------------------------------------------
# Platform detection
# ---------------------------------------------------------------------------
detect_os() {
  uname -s
}

detect_arch() {
  uname -m
}

# Returns the GitHub release asset name for the current platform, or empty
# string if the platform is not supported.
detect_asset() {
  _os="$(detect_os)"
  _arch="$(detect_arch)"
  case "$_os" in
    Linux)
      case "$_arch" in
        x86_64)  printf 'clin-tauri_amd64.AppImage' ;;
        aarch64) printf 'clin-tauri_aarch64.AppImage' ;;
        *)       printf '' ;;
      esac ;;
    Darwin)
      case "$_arch" in
        x86_64)  printf 'clin-tauri_x64.dmg' ;;
        arm64)   printf 'clin-tauri_aarch64.dmg' ;;
        *)       printf '' ;;
      esac ;;
    *)
      printf '' ;;
  esac
}

# ---------------------------------------------------------------------------
# Version resolution
# ---------------------------------------------------------------------------
resolve_version() {
  if [ "$REQUESTED_VERSION" = "latest" ]; then
    _ver="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
      | grep '"tag_name"' \
      | sed 's/.*"tag_name": *"\(.*\)".*/\1/')"
    [ -z "$_ver" ] && error "Could not resolve latest release version from GitHub API."
    printf '%s' "$_ver"
  else
    printf '%s' "$REQUESTED_VERSION"
  fi
}

# ---------------------------------------------------------------------------
# Linux install
# ---------------------------------------------------------------------------
install_linux() {
  _asset="$1"
  _version="$2"
  _url="https://github.com/$REPO/releases/download/${_version}/${_asset}"
  _bin_dir="$HOME/.local/bin"
  _app_dir="$HOME/.local/share/applications"
  _icon_dir="$HOME/.local/share/icons/hicolor/256x256/apps"
  _tmp="$(mktemp -d)"

  info "Downloading clin-tauri $_version for Linux..."
  if ! curl -fsSL "$_url" -o "$_tmp/clin-tauri.AppImage"; then
    rm -rf "$_tmp"
    error "Download failed: $_url"
  fi

  chmod +x "$_tmp/clin-tauri.AppImage"
  mkdir -p "$_bin_dir"
  mv "$_tmp/clin-tauri.AppImage" "$_bin_dir/clin-tauri"
  rm -rf "$_tmp"
  info "Binary installed to $_bin_dir/clin-tauri"

  # PATH hint
  case ":$PATH:" in
    *":$_bin_dir:"*) ;;
    *)
      warn "$_bin_dir is not on your PATH."
      warn "Add it with:  export PATH=\"\$PATH:$_bin_dir\""
      for _rc in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
        if [ -f "$_rc" ] && ! grep -qF "$_bin_dir" "$_rc" 2>/dev/null; then
          printf '\n# clin-tauri\nexport PATH="$PATH:%s"\n' "$_bin_dir" >> "$_rc"
          info "Added PATH entry to $_rc"
          break
        fi
      done ;;
  esac

  # -- XDG .desktop entry (app-menu integration, always created) -----------
  mkdir -p "$_app_dir"
  mkdir -p "$_icon_dir"

  # Try to extract the bundled icon from the AppImage; ignore errors silently.
  if command -v wrestool >/dev/null 2>&1; then
    wrestool -x --output="$_icon_dir/clin-tauri.png" "$_bin_dir/clin-tauri" 2>/dev/null || true
  fi

  cat > "$_app_dir/clin-tauri.desktop" <<DESKTOP
[Desktop Entry]
Version=1.0
Type=Application
Name=clin
GenericName=Markdown Vault
Comment=clin desktop companion — native wrapper around the clin markdown vault
Exec=$_bin_dir/clin-tauri %u
Icon=clin-tauri
Categories=Utility;Office;
StartupNotify=true
MimeType=text/markdown;
Keywords=notes;markdown;vault;
DESKTOP

  info "XDG .desktop entry written to $_app_dir/clin-tauri.desktop"

  # Refresh the desktop database so the entry appears in app menus immediately.
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$_app_dir" 2>/dev/null && \
      info "Updated desktop application database."
  fi

  # -- Optional Desktop shortcut (only with --desktop) ---------------------
  if [ "$ADD_DESKTOP_SHORTCUT" = "1" ]; then
    _desktop_dir="$HOME/Desktop"
    if [ -d "$_desktop_dir" ]; then
      cp "$_app_dir/clin-tauri.desktop" "$_desktop_dir/clin-tauri.desktop"
      chmod +x "$_desktop_dir/clin-tauri.desktop"
      info "Desktop shortcut created at $_desktop_dir/clin-tauri.desktop"
    else
      warn "~/Desktop does not exist — skipping desktop shortcut."
    fi
  fi
}

# ---------------------------------------------------------------------------
# macOS install
# ---------------------------------------------------------------------------
install_macos() {
  _asset="$1"
  _version="$2"
  _url="https://github.com/$REPO/releases/download/${_version}/${_asset}"
  _tmp="$(mktemp -d)"

  info "Downloading clin-tauri $_version for macOS..."
  if ! curl -fsSL "$_url" -o "$_tmp/clin-tauri.dmg"; then
    rm -rf "$_tmp"
    error "Download failed: $_url"
  fi

  info "Mounting disk image..."
  _mount="$(hdiutil attach "$_tmp/clin-tauri.dmg" -nobrowse -quiet | awk 'END{print $NF}')"
  if [ -z "$_mount" ]; then
    rm -rf "$_tmp"
    error "Could not mount $_tmp/clin-tauri.dmg"
  fi

  _app="$(find "$_mount" -maxdepth 1 -name '*.app' | head -n 1)"
  if [ -z "$_app" ]; then
    hdiutil detach "$_mount" -quiet 2>/dev/null || true
    rm -rf "$_tmp"
    error "No .app bundle found inside the DMG."
  fi

  info "Copying $(basename "$_app") to /Applications..."
  cp -R "$_app" /Applications/

  hdiutil detach "$_mount" -quiet 2>/dev/null || true
  rm -rf "$_tmp"

  info "clin has been installed to /Applications/$(basename "$_app")"
  info "Tip: drag clin from /Applications to your Dock to pin it."
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
  _os="$(detect_os)"
  _asset="$(detect_asset)"

  if [ -z "$_asset" ]; then
    error "Unsupported platform: $_os / $(detect_arch). Please download a release manually from https://github.com/$REPO/releases"
  fi

  _version="$(resolve_version)"
  info "Installing clin-tauri $_version..."

  case "$_os" in
    Linux)  install_linux  "$_asset" "$_version" ;;
    Darwin) install_macos  "$_asset" "$_version" ;;
    *)      error "Unsupported OS: $_os" ;;
  esac

  info ""
  info "Done. Run: clin-tauri"
  info "Docs: https://github.com/$REPO"
}

main
