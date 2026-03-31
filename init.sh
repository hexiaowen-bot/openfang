#!/bin/bash
# Agent-Swarm 环境初始化脚本 (Unix/macOS)
# 该脚本用于初始化 Rust 开发环境并安装质量工具

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_NAME="agent-swarm"

echo "========================================="
echo "Initializing $PROJECT_NAME environment..."
echo "========================================="

# 检查 Rust 是否安装
echo ""
echo "[1/5] Checking Rust installation..."
if ! command -v rustc &> /dev/null; then
    echo "ERROR: Rust is not installed. Please install Rust first:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

RUST_VERSION=$(rustc --version | cut -d' ' -f2)
echo "✓ Rust $RUST_VERSION installed"

# 检查 Cargo
echo ""
echo "[2/5] Checking Cargo..."
if ! command -v cargo &> /dev/null; then
    echo "ERROR: Cargo not found. Please ensure Rust is properly installed."
    exit 1
fi
CARGO_VERSION=$(cargo --version | cut -d' ' -f2)
echo "✓ Cargo $CARGO_VERSION installed"

# 检查 Rust 版本 >= 1.75
echo ""
echo "[3/5] Checking Rust version (>= 1.75)..."
REQUIRED_VERSION="1.75.0"
CURRENT_VERSION=$(rustc --version | cut -d' ' -f2)

# 版本比较函数
version_ge() {
    [ "$1" = "$2" ] && return 0
    [ "$(printf '%s\n' "$1" "$2" | sort -V | head -n1)" = "$2" ] && return 0
    return 1
}

if version_ge "$CURRENT_VERSION" "$REQUIRED_VERSION"; then
    echo "✓ Rust version $CURRENT_VERSION meets requirement (>= $REQUIRED_VERSION)"
else
    echo "WARNING: Rust version $CURRENT_VERSION is older than required $REQUIRED_VERSION"
    echo "  Please update: rustup update"
fi

# 安装质量工具
echo ""
echo "[4/5] Installing quality tools..."

# 检查并安装 cargo-tarpaulin
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "Installing cargo-tarpaulin..."
    cargo install cargo-tarpaulin
else
    echo "✓ cargo-tarpaulin already installed"
fi

# 检查并安装 cargo-mutants
if ! command -v cargo-mutants &> /dev/null; then
    echo "Installing cargo-mutants..."
    cargo install cargo-mutants
else
    echo "✓ cargo-mutants already installed"
fi

# 验证项目结构
echo ""
echo "[5/5] Verifying project structure..."
cd "$SCRIPT_DIR"

if [ ! -f "Cargo.toml" ]; then
    echo "ERROR: Cargo.toml not found. Are you in the project root?"
    exit 1
fi
echo "✓ Cargo.toml found"

if [ ! -d "crates" ]; then
    echo "WARNING: crates/ directory not found"
else
    echo "✓ crates/ directory exists"
fi

# 尝试编译项目
echo ""
echo "Verifying project compiles..."
if cargo check --all 2>&1 | head -20; then
    echo "✓ Project compiles successfully"
else
    echo "WARNING: Project has compilation errors (expected for new features)"
fi

# 创建必需目录
echo ""
echo "Creating required directories..."
mkdir -p "$HOME/.openfang/swarms"
echo "✓ Created $HOME/.openfang/swarms"

# 设置环境变量提示
echo ""
echo "========================================="
echo "Environment setup complete!"
echo "========================================="
echo ""
echo "Add the following to your shell profile (.bashrc/.zshrc):"
echo "  export OPENFANG_HOME=\$HOME/.openfang"
echo ""
echo "Quick start:"
echo "  cargo test              # Run tests"
echo "  cargo tarpaulin         # Generate coverage report"
echo "  cargo mutants           # Run mutation tests"
echo ""
