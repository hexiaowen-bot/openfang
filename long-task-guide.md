# Agent-Swarm 项目 Worker 指南

本指南专为 Agent-Swarm 项目的 Worker 会话设计，包含环境激活、测试执行、质量门禁等完整流程。

---

## Orient

每次会话开始时：
1. 读取 `feature-list.json` 了解当前项目状态和待办功能
2. 读取 `task-progress.md` 的 `## Current State` 了解最近进展
3. 确定本次会话要实施的功能（选择优先级高且依赖已满足的功能）

**项目结构：**
- `crates/openfang-swarm/` - Swarm 核心库（parser、scheduler、executor、mapping、condition）
- `crates/openfang-cli/src/commands/swarm.rs` - CLI 命令实现
- `swarms/` - Swarm 配置目录

---

## Bootstrap

### 环境激活

```bash
# Rust 环境（已通过 rustup 安装）
rustc --version  # 验证 Rust 版本 >= 1.75
cargo --version
```

### 安装质量工具

```bash
# 安装代码覆盖率工具
cargo install cargo-tarpaulin

# 安装变异测试工具
cargo install cargo-mutants
```

---

## Config Gate

### 检查必需配置

```bash
python scripts/check_configs.py feature-list.json
```

### 配置管理

本项目使用环境变量进行配置：
- **添加配置**: 在 `.env` 文件中添加 `KEY=value`，或在 shell 中执行 `export KEY=value`
- **系统环境**: 直接通过 `export` 设置，无需文件

**必需配置：**
- `OPENFANG_HOME` - OpenFang 系统主目录（默认: `$HOME/.openfang`）

---

## TDD Red

### 创建测试

1. 在对应模块的 `tests` 目录或 `#[cfg(test)]` 模块中创建测试
2. 测试命名: `test_<功能>_<场景>`
3. 使用 `tokio::test` 标记异步测试

```rust
#[tokio::test]
async fn test_parser_valid_config() {
    // Given
    let toml = r#"id = "test-swarm""#;
    
    // When
    let result = SwarmParser::parse(toml);
    
    // Then
    assert!(result.is_ok());
    assert_eq!(result.unwrap().id, "test-swarm");
}
```

### 运行测试（Red）

```bash
# 运行单个 crate 的测试
cd crates/openfang-swarm && cargo test test_parser

# 运行所有测试
cargo test
```

期望结果：测试失败（功能尚未实现）

---

## TDD Green

### 实现功能

1. 在对应模块中实现功能代码
2. 遵循 Rust 编码规范（已通过 `rustfmt.toml` 配置）
3. 添加必要的错误处理和日志

### 运行测试（Green）

```bash
# 运行测试直到通过
cargo test test_parser

# 验证代码格式
cargo fmt --check

# 运行 Clippy 检查
cargo clippy -- -D warnings
```

期望结果：所有测试通过，无 Clippy 警告

---

## Coverage Gate

### 运行覆盖率测试

```bash
# 生成覆盖率报告
cargo tarpaulin --out Html --output-dir target/tarpaulin

# 或生成终端报告
cargo tarpaulin --out Stdout
```

### 验证覆盖率阈值

```bash
# 检查是否满足 90% 行覆盖率、80% 分支覆盖率
python scripts/check_st_readiness.py feature-list.json
```

**处理低覆盖率：**
1. 查看 `target/tarpaulin/tarpaulin-report.html` 找出未覆盖代码
2. 添加针对性测试
3. 重新运行直到满足阈值

---

## TDD Refactor

### 重构检查清单

- [ ] 代码重复已消除（DRY 原则）
- [ ] 函数长度合理（< 50 行）
- [ ] 错误处理统一使用 `thiserror` 或 `anyhow`
- [ ] 异步代码正确使用 `tokio`
- [ ] 公共 API 有文档注释（`///`）

### 重构后验证

```bash
# 完整测试套件
cargo test

# 覆盖率不下降
cargo tarpaulin --out Stdout

# Clippy 无警告
cargo clippy -- -D warnings
```

---

## Mutation Gate

### 运行变异测试

```bash
# 运行变异测试
cargo mutants

# 查看结果
cargo mutants --list
```

### 处理变异存活

1. 查看 `mutants.out/` 目录中的变异报告
2. 对每个存活的变异，分析测试为何未捕获
3. 添加或强化测试
4. 重新运行直到变异得分 >= 80%

---

## Verification Enforcement

### 提交前强制检查

```bash
# 1. 完整测试通过
cargo test --all

# 2. 代码格式正确
cargo fmt --check

# 3. 无 Clippy 警告
cargo clippy --all -- -D warnings

# 4. 覆盖率满足阈值
cargo tarpaulin --fail-under 90

# 5. 变异测试通过（可选，耗时较长）
# cargo mutants
```

### 验证失败处理

- **测试失败**: 修复实现代码
- **格式错误**: 运行 `cargo fmt`
- **Clippy 警告**: 按警告提示修复
- **覆盖率不足**: 补充测试
- **变异存活**: 强化测试用例

---

## Code Review

### 自检查清单

- [ ] 测试覆盖所有边界条件
- [ ] 错误消息清晰有用
- [ ] 异步资源正确清理
- [ ] 无 `unwrap()` 或 `expect()` 在业务代码中
- [ ] 日志级别适当（trace/debug/info/warn/error）

### 调用 Review 技能

```
skill code-review
```

---

## Examples

### 测试示例

```rust
// crates/openfang-swarm/src/parser.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
id = "test-swarm"
name = "Test Swarm"
version = "1.0.0"

[[steps]]
id = "step1"
name = "Test Step"
hand = "test-hand"
"#;
        let result = SwarmParser::parse(toml);
        assert!(result.is_ok());
        
        let config = result.unwrap();
        assert_eq!(config.id, "test-swarm");
        assert_eq!(config.steps.len(), 1);
    }

    #[test]
    fn test_parse_invalid_config() {
        let toml = r#"invalid = "config""#;
        let result = SwarmParser::parse(toml);
        assert!(result.is_err());
    }
}
```

### 功能实现示例

```rust
pub struct SwarmParser;

impl SwarmParser {
    pub fn parse(toml_str: &str) -> Result<SwarmDefinition, ParseError> {
        let definition: SwarmDefinition = toml::from_str(toml_str)
            .map_err(|e| ParseError::InvalidToml(e.to_string()))?;
        
        definition.validate()?;
        Ok(definition)
    }
}
```

---

## Persist

### 更新进度

功能完成后：
1. 更新 `feature-list.json` 中对应功能的 `status` 为 `"passing"`
2. 在 `task-progress.md` 追加会话记录
3. 更新 `RELEASE_NOTES.md` 添加变更说明

### Git 提交

```bash
# 1. 添加变更
git add .

# 2. 提交（遵循约定式提交）
git commit -m "feat(swarm): 实现 Swarm.toml 解析器

- 支持完整的配置结构解析
- 包含输入验证
- 行覆盖率 95%

Closes #1"

# 3. 验证提交
git log --oneline -3
```

---

## Critical Rules

1. **必须 TDD**: 永远先写测试再实现
2. **必须满足质量门禁**: 覆盖率 90%/80%，变异得分 80%
3. **必须代码审查**: 提交前自我审查，关键变更请求 review
4. **必须更新进度**: 功能完成后立即更新 feature-list.json 和 task-progress.md
5. **不能跳过验证**: 任何提交前必须通过所有验证步骤
6. **不能降低覆盖率**: 新代码覆盖率不得低于现有水平

---

## Environment Commands

### 环境激活

无需额外激活（Rust 已通过 rustup 全局安装）

### 测试执行

```bash
# 运行所有测试
cargo test

# 运行指定 crate 测试
cd crates/openfang-swarm && cargo test

# 运行指定测试
cargo test test_parser

# 显示测试输出
cargo test -- --nocapture
```

### 覆盖率报告

```bash
# 生成 HTML 报告
cargo tarpaulin --out Html --output-dir target/tarpaulin

# 终端输出
cargo tarpaulin --out Stdout

# 强制阈值检查
cargo tarpaulin --fail-under 90
```

### 变异测试

```bash
# 运行变异测试
cargo mutants

# 仅检查未测试的代码
cargo mutants --list

# 查看详细结果
cat mutants.out/mutants.json
```

### 代码检查

```bash
# 格式化代码
cargo fmt

# 检查格式
cargo fmt --check

# 运行 Clippy
cargo clippy -- -D warnings
```

---

## Service Commands

本项目为库项目，无服务器进程。CLI 命令直接执行。

---

## Config Management

**添加/更新配置值：**

本项目使用系统环境变量或 `.env` 文件：

```bash
# 方法1: 系统环境变量
export OPENFANG_HOME=$HOME/.openfang

# 方法2: .env 文件（如果使用 dotenv crate）
echo "OPENFANG_HOME=$HOME/.openfang" >> .env
```

**检查配置是否生效：**

```bash
python scripts/check_configs.py feature-list.json
```
