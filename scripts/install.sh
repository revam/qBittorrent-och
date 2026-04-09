#!/bin/bash
set -e

REPO="revam/qBittorrent-och"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Install qb-och binary for your system.

OPTIONS:
    --install-path <path>  Directory to install binary (default: ~/.local/bin)
    --force               Overwrite existing binary
    -h, --help            Show this help message

EXAMPLES:
    $0
    $0 --install-path /usr/local/bin
    $0 --force
EOF
    exit 0
}

FORCE=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --install-path)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --force)
            FORCE=true
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

detect_os_arch() {
    local os arch libc

    case "$(uname -s)" in
        Linux*)
            os="linux"
            ;;
        Darwin*)
            os="macos"
            ;;
        *)
            echo "Unsupported OS: $(uname -s)" >&2
            exit 1
            ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)
            arch="x64"
            ;;
        aarch64|arm64)
            arch="arm64"
            ;;
        *)
            echo "Unsupported architecture: $(uname -m)" >&2
            exit 1
            ;;
    esac

    if [[ "$os" == "linux" ]]; then
        if ldd --version 2>&1 | grep -q "musl"; then
            libc="musl"
        else
            libc="gnu"
        fi
    fi

    echo "$os $arch ${libc:-none}"
}

get_artifact_name() {
    local os=$1 arch=$2 libc=$3

    case "$os" in
        linux)
            echo "qb-och-linux-${arch}-${libc}.zip"
            ;;
        macos)
            echo "qb-och-macos-${arch}.zip"
            ;;
    esac
}

echo "Detecting system..."
read -r os arch libc <<< "$(detect_os_arch)"
echo "Detected: OS=$os, Arch=$arch, Libc=$libc"

ARTIFACT=$(get_artifact_name "$os" "$arch" "$libc")
echo "Using artifact: $ARTIFACT"

echo "Fetching latest release info..."
LATEST=$(curl -sL "https://api.github.com/repos/$REPO/releases/latest")
TAG=$(echo "$LATEST" | grep -o '"tag_name": "[^"]*' | cut -d'"' -f4)

echo "Latest version: $TAG"

URL="https://github.com/$REPO/releases/download/$TAG/$ARTIFACT"
echo "Downloading from: $URL"

TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

curl -sL -o "$TEMP_DIR/qb-och.zip" "$URL"

if [[ ! -s "$TEMP_DIR/qb-och.zip" ]]; then
    echo "Download failed or empty file" >&2
    exit 1
fi

mkdir -p "$INSTALL_DIR"

if [[ -f "$INSTALL_DIR/qb-och" && "$FORCE" != "true" ]]; then
    echo "Binary already exists at $INSTALL_DIR/qb-och. Use --force to overwrite." >&2
    exit 1
fi

unzip -o -j "$TEMP_DIR/qb-och.zip" -d "$INSTALL_DIR" qb-och 2>/dev/null || \
    unzip -o -j "$TEMP_DIR/qb-och.zip" -d "$TEMP_DIR" "qb-och.exe" 2>/dev/null

if [[ "$os" != "windows" ]]; then
    chmod +x "$INSTALL_DIR/qb-och"
fi

VERSION=$("$INSTALL_DIR/qb-och" --version 2>/dev/null | head -n1 || echo "unknown")

echo ""
echo "Installed qb-och $VERSION to $INSTALL_DIR/qb-och"
echo "Add $INSTALL_DIR to your PATH if not already included."