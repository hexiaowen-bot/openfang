> User-editable. Claude reads this file before managing services. Update when ports change or new services are added.

# Agent-Swarm 环境指南

本项目为 Rust 库/CLI 项目，**无服务器进程**。环境激活仅需验证 Rust 工具链。

---

## 环境激活

```bash
# 验证 Rust 已安装
rustc --version  # 期望: >= 1.75
cargo --version

# 安装质量工具（如尚未安装）
cargo install cargo-tarpaulin
cargo install cargo-mutants
```

---

## 必需配置

**OPENFANG_HOME** - OpenFang 系统主目录

```bash
export OPENFANG_HOME=$HOME/.openfang
mkdir -p $OPENFANG_HOME/swarms
```

---

## CLI 使用示例

```bash
# 运行 Swarm
openfang swarm run market-intelligence \
  --input target_company="OpenAI" \
  --input focus_areas='["产品发布", "融资动态"]' \
  --input depth="deep"

# 查看执行状态
openfang swarm status swarm-2025-03-28-001

# 列出所有 Swarm
openfang swarm list
```
