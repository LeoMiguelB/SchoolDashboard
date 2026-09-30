#!/usr/bin/env bash
set -e

# Terminal styling
BOLD='\033[1m'
GREEN='\033[0;32m'
SKY='\033[0;36m'
PEACH='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${SKY}${BOLD}⚡ Installing School-Dashboard (schoodash)...${NC}"

# 1. Verify Rust & Cargo (auto-detect ~/.cargo/bin and mise if not in current subshell PATH)
if ! command -v cargo &> /dev/null; then
    if [ -x "$HOME/.cargo/bin/cargo" ]; then
        export PATH="$HOME/.cargo/bin:$PATH"
    elif [ -x "$HOME/.local/share/mise/shims/cargo" ]; then
        export PATH="$HOME/.local/share/mise/shims:$PATH"
    fi
fi

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Cargo / Rust is not found in PATH.${NC}"
    echo -e "Please install Rust via https://rustup.rs or your package manager:"
    echo -e "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# 2. Build optimized release binary
echo -e "${PEACH}Compiling optimized release binary with cargo...${NC}"
cargo build --release

# 3. Destination directory (~/.local/bin)
INSTALL_DIR="${HOME}/.local/bin"
mkdir -p "$INSTALL_DIR"

# 4. Install binary and alias
rm -f "$INSTALL_DIR/schoodash" "$INSTALL_DIR/schooldash"
cp -f target/release/schoodash "$INSTALL_DIR/schoodash"
chmod +x "$INSTALL_DIR/schoodash"
ln -sf "$INSTALL_DIR/schoodash" "$INSTALL_DIR/schooldash"

echo -e "${GREEN}${BOLD}✔ Successfully installed:${NC} ${INSTALL_DIR}/schoodash"
echo -e "${GREEN}${BOLD}✔ Created alias:${NC}      ${INSTALL_DIR}/schooldash"

# 5. Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo -e ""
    echo -e "${PEACH}${BOLD}Notice:${NC} ${INSTALL_DIR} is not in your current \$PATH."
    echo -e "Add this line to your ${BOLD}~/.bashrc${NC} or ${BOLD}~/.zshrc${NC}:"
    echo -e "  export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo -e "Then reload your shell: ${BOLD}source ~/.bashrc${NC}"
fi

echo -e ""
echo -e "${SKY}${BOLD}Done! You can now launch the dashboard from anywhere with:${NC}"
echo -e "  ${BOLD}schoodash${NC}"
