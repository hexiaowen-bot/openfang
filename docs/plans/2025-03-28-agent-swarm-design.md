# Agent-Swarm 设计文档

**版本**: 1.0.0  
**日期**: 2025-03-28  
**状态**: 设计中  

---

## 1. 概述

### 1.1 设计目标

Agent-Swarm 是 OpenFang Hand 系统的扩展层，旨在实现多个 Hand 的自动化协作。通过声明式配置定义工作流，让不同的 Hand（如 Collector、Researcher、Twitter 等）能够按序或并行执行，共享上下文，完成复杂的多阶段任务。

### 1.2 核心原则

- **复用优先**: 完全复用现有的 Hand 基础设施，不修改 Hand 本身
- **配置驱动**: 通过 `Swarm.toml` 声明工作流，无需编写代码
- **显式数据流**: 步骤间的数据传递通过显式映射定义
- **可观测性**: 工作流执行状态、中间结果全程可追踪

---

## 2. 架构设计

### 2.1 系统架构图

```
┌─────────────────────────────────────────────────────────┐
│                     用户接口层                            │
│              CLI: openfang swarm run <config>           │
└────────────────────┬────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│                  Swarm 编排器                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   解析器      │  │   调度器      │  │   监控器      │  │
│  │ (Swarm.toml) │  │ (步骤执行)    │  │ (状态跟踪)    │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└────────────────────┬────────────────────────────────────┘
                     │
        ┌────────────┼────────────┐
        ▼            ▼            ▼
   ┌─────────┐  ┌─────────┐  ┌─────────┐
   │ Hand A  │  │ Hand B  │  │ Hand C  │
   │(任意类型)│  │(任意类型)│  │(任意类型)│
   └────┬────┘  └────┬────┘  └────┬────┘
        │            │            │
        └────────────┼────────────┘
                     ▼
           ┌─────────────────┐
           │   共享知识图谱    │
           │ (工作流上下文)    │
           └─────────────────┘
```

### 2.2 组件清单

| 组件 | 类型 | 职责 | 文件位置 |
|------|------|------|----------|
| `Swarm.toml` | 配置文件 | 定义工作流结构、步骤、依赖、映射 | `swarms/<name>/Swarm.toml` |
| Swarm 解析器 | 库 | 解析和验证 Swarm.toml | `crates/openfang-swarm/src/parser.rs` |
| Swarm 调度器 | 库 | 执行步骤调度（拓扑排序、并行执行） | `crates/openfang-swarm/src/scheduler.rs` |
| Swarm 执行器 | 库 | 调用 Hand Registry 激活 Hands | `crates/openfang-swarm/src/executor.rs` |
| 数据映射引擎 | 库 | 处理步骤间的数据传递 | `crates/openfang-swarm/src/mapping.rs` |
| 条件引擎 | 库 | 评估步骤执行条件 | `crates/openfang-swarm/src/condition.rs` |
| Swarm CLI | 二进制 | 命令行接口 | `crates/openfang-cli/src/commands/swarm.rs` |

---

## 3. 核心机制

### 3.1 步骤依赖图（DAG）

工作流步骤通过 `depends_on` 定义依赖关系，形成有向无环图（DAG）。调度器使用拓扑排序确定执行顺序，无依赖的步骤可并行执行。

**示例**: 市场情报工作流

```
collect ───→ research ───→ report
    │
    └──────→ notify (并行)
```

### 3.2 数据映射机制

步骤间的数据传递通过显式映射定义，支持以下源：

| 源类型 | 语法 | 示例 |
|--------|------|------|
| 工作流输入 | `input.<key>` | `input.target_company` |
| 步骤输出 | `steps.<step_id>.<output_key>` | `steps.collect.entities` |
| 环境变量 | `env.<VAR_NAME>` | `env.OPENFANG_HOME` |
| 常量 | `"literal value"` | `"standard"` |

**映射规则**:

- `from`: 数据源路径
- `to`: 目标 Hand 的输入路径（settings、prompt、context）
- `transform`: 可选的转换函数（如 `json`, `join`, `extract`）

### 3.3 条件执行

步骤可通过 `condition` 字段定义执行条件，使用表达式引擎评估：

```toml
condition = "steps.collect.metrics.data_points > 5 && input.depth == 'deep'"
```

支持的运算符：
- 比较：`>`, `<`, `>=`, `<=`, `==`, `!=`
- 逻辑：`&&`, `||`, `!`
- 存在检查：`has(steps.collect.output)`

### 3.4 错误处理策略

全局和步骤级别可配置错误处理：

| 策略 | 行为 |
|------|------|
| `fail` | 立即停止工作流，报告错误 |
| `continue` | 记录错误，继续执行后续步骤 |
| `skip` | 跳过当前步骤，继续执行 |
| `retry` | 重试当前步骤（需配置重试次数和延迟）|

---

## 4. Swarm.toml 配置规范

### 4.1 完整配置示例

```toml
# 基本元信息
id = "market-intelligence"
name = "市场情报监控"
description = "自动收集市场情报、深度分析并生成报告"
version = "1.0.0"
category = "workflow"
icon = "📊"

# 工作流输入参数定义
[input]
required = ["target_company", "focus_areas"]
optional = { 
    depth = "standard",
    report_format = "markdown",
    enable_notification = false
}

# 输入验证规则
[input.validation]
target_company = { min_length = 1, max_length = 100 }
focus_areas = { type = "array", min_items = 1, max_items = 5 }
depth = { enum = ["quick", "standard", "deep"] }

# 步骤定义
[[steps]]
id = "collect"
name = "情报收集"
description = "使用 Collector 手收集目标公司的公开情报"
hand = "collector"
depends_on = []

[steps.input_mapping]
"settings.target_subject" = "input.target_company"
"settings.focus_area" = "input.focus_areas"
"settings.collection_depth" = "input.depth"

[steps.output_mapping]
entities = "knowledge.entities"
metrics = "dashboard.metrics"

[[steps]]
id = "research"
name = "深度研究"
description = "对收集到的实体进行深度研究"
hand = "researcher"
depends_on = ["collect"]
condition = "steps.collect.metrics.data_points > 5"

[steps.input_mapping]
"settings.research_depth" = "input.depth"
"prompt.context" = "steps.collect.entities"

[steps.output_mapping]
report = "report.content"
relations = "knowledge.relations"

[steps.retry]
max_attempts = 3
delay_seconds = 5
backoff = "exponential"  # linear, exponential, fixed

[[steps]]
id = "report"
name = "生成报告"
description = "整合研究结果生成最终报告"
hand = "technical-writer"
depends_on = ["research"]

[steps.input_mapping]
"prompt.research_data" = "steps.research.report"
"settings.output_format" = "input.report_format"

[[steps]]
id = "notify"
name = "结果通知"
description = "将报告摘要发布到社交媒体"
hand = "twitter"
depends_on = ["report"]
condition = "input.enable_notification == true"

[steps.input_mapping]
"prompt.content" = "steps.report.summary"

# 全局错误处理
[error_handling]
default_strategy = "fail"
max_retries = 3
retry_delay_seconds = 5

# 全局设置
[settings]
timeout_minutes = 30
max_parallel_steps = 3
shared_knowledge_namespace = "swarm.market-intel"
persist_intermediate_results = true
cleanup_on_success = false

# 仪表盘指标定义
[dashboard.metrics]
steps_completed = { label = "已完成步骤", format = "integer" }
total_duration_sec = { label = "总耗时(秒)", format = "duration" }
success_rate = { label = "成功率", format = "percentage" }

# 事件定义
[[events]]
name = "swarm.completed"
description = "工作流成功完成时触发"
payload = ["execution_id", "duration", "output_summary"]

[[events]]
name = "swarm.failed"
description = "工作流失败时触发"
payload = ["execution_id", "failed_step", "error_message"]
```

### 4.2 配置字段说明

#### 根级别字段

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `id` | string | 是 | 工作流唯一标识符 |
| `name` | string | 是 | 人类可读名称 |
| `description` | string | 否 | 工作流描述 |
| `version` | string | 是 | 语义化版本 |
| `category` | string | 否 | 分类标签 |
| `icon` | string | 否 | 图标（emoji 或路径）|
| `input` | table | 否 | 输入参数定义 |
| `steps` | array | 是 | 步骤列表 |
| `error_handling` | table | 否 | 错误处理配置 |
| `settings` | table | 否 | 全局设置 |
| `dashboard` | table | 否 | 仪表盘指标 |
| `events` | array | 否 | 事件定义 |

#### 步骤字段

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `id` | string | 是 | 步骤唯一标识 |
| `name` | string | 是 | 步骤名称 |
| `description` | string | 否 | 步骤描述 |
| `hand` | string | 是 | 使用的 Hand ID |
| `depends_on` | array | 否 | 依赖的步骤 ID 列表 |
| `input_mapping` | table | 否 | 输入数据映射 |
| `output_mapping` | table | 否 | 输出数据映射 |
| `condition` | string | 否 | 执行条件表达式 |
| `retry` | table | 否 | 重试配置 |

---

## 5. 数据模型

### 5.1 工作流执行上下文

```rust
pub struct SwarmContext {
    /// 工作流定义
    pub definition: SwarmDefinition,
    
    /// 工作流输入参数
    pub input: HashMap<String, Value>,
    
    /// 步骤执行结果
    pub step_results: HashMap<String, StepResult>,
    
    /// 共享知识图谱命名空间
    pub knowledge_namespace: String,
    
    /// 执行状态
    pub status: SwarmStatus,
    
    /// 开始时间
    pub started_at: DateTime<Utc>,
    
    /// 结束时间
    pub completed_at: Option<DateTime<Utc>>,
}

pub struct StepResult {
    /// 步骤 ID
    pub step_id: String,
    
    /// 执行状态
    pub status: StepStatus,
    
    /// Hand 实例 ID
    pub hand_instance_id: Option<Uuid>,
    
    /// 输出数据
    pub output: HashMap<String, Value>,
    
    /// 执行时长
    pub duration_ms: u64,
    
    /// 错误信息（如果失败）
    pub error: Option<String>,
    
    /// 重试次数
    pub retry_count: u32,
}

pub enum SwarmStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
    Cancelled,
}
```

### 5.2 共享数据结构

工作流中的 Hands 通过知识图谱共享数据：

```rust
// 工作流命名空间下的实体
pub struct SwarmEntity {
    pub id: String,
    pub swarm_id: String,
    pub step_id: String,
    pub entity_type: String,
    pub name: String,
    pub properties: Value,
    pub created_at: DateTime<Utc>,
}

// 工作流命名空间下的关系
pub struct SwarmRelation {
    pub id: String,
    pub swarm_id: String,
    pub source_entity: String,
    pub relation_type: String,
    pub target_entity: String,
    pub properties: Value,
    pub confidence: f32,
}
```

---

## 6. CLI 接口设计

### 6.1 命令列表

```bash
# 列出所有可用的 Swarm 配置
openfang swarm list

# 查看 Swarm 配置详情
openfang swarm show <swarm-id>

# 运行 Swarm
openfang swarm run <swarm-id> --input key=value

# 运行 Swarm（从文件加载输入）
openfang swarm run <swarm-id> --input-file params.json

# 查看执行状态
openfang swarm status <execution-id>

# 查看执行日志
openfang swarm logs <execution-id> [--step <step-id>]

# 取消执行
openfang swarm cancel <execution-id>

# 重试失败的执行
openfang swarm retry <execution-id>

# 验证 Swarm 配置
openfang swarm validate <swarm-id>
```

### 6.2 运行示例

```bash
# 运行市场情报工作流
openfang swarm run market-intelligence \
  --input target_company="OpenAI" \
  --input focus_areas='["产品发布", "融资动态", "竞争对手"]' \
  --input depth="deep" \
  --input enable_notification=true

# 输出
✓ Swarm "市场情报监控" 启动成功
  执行 ID: swarm-2025-03-28-001
  
[1/4] 情报收集 (collector) ...... ✓ 12s
[2/4] 深度研究 (researcher) ..... ✓ 45s
[3/4] 生成报告 (writer) ......... ✓ 8s
[4/4] 结果通知 (twitter) ........ ✓ 3s

✓ 工作流完成 (总计: 68s)
  报告已保存: ~/.openfang/swarms/market-intelligence/outputs/swarm-2025-03-28-001-report.md
```

---

## 7. 典型使用场景

### 7.1 场景一：市场情报监控

**需求**: 每日自动监控竞争对手动态，生成简报并邮件通知

**Swarm 结构**:
1. Collector 手收集新闻、财报、招聘信息
2. Researcher 手深度分析重大变化
3. Technical Writer 生成执行摘要
4. （条件）Twitter 手发布关键发现

### 7.2 场景二：内容生产流水线

**需求**: 从热点发现到内容发布的自动化流程

**Swarm 结构**:
1. Collector 监控行业热点
2. Researcher 深度研究热点话题
3. 并行: Technical Writer 撰写长文 + Designer 生成配图
4. Twitter 发布内容

### 7.3 场景三：交易信号处理

**需求**: 多源信号收集 → 分析 → 决策

**Swarm 结构**:
1. Collector 收集市场新闻、社交情绪
2. Predictor 生成价格预测
3. Trader 综合信号执行交易（分析/模拟/实盘）
4. Technical Writer 生成交易报告

---

## 8. 安全与权限

### 8.1 Hand 权限继承

Swarm 中的每个 Hand 实例继承该 Hand 定义的权限限制：

- 工具白名单 (`allowed_tools`)
- 审批门控 (`requires_approval`)
- 执行策略 (`exec_policy`)

### 8.2 数据隔离

- 每个工作流执行拥有独立的共享命名空间
- 工作流之间数据隔离（除非显式配置共享）
- 敏感输入支持加密存储

### 8.3 审批机制

- 涉及敏感操作的步骤可启用人工审批
- 审批请求通过 CLI/Web 界面推送
- 支持批量审批和自动审批规则

---

## 9. 性能考虑

### 9.1 并发执行

- 无依赖的步骤自动并行执行
- 最大并行度可配置 (`max_parallel_steps`)
- Hand 实例池化复用（如支持）

### 9.2 资源管理

- 步骤超时控制（防止无限等待）
- 内存限制（大数据集流式处理）
- 临时文件自动清理

### 9.3 缓存策略

- 中间结果可配置持久化
- 相同输入的 Hand 调用可缓存
- 知识图谱查询结果缓存

---

## 10. 实现路线图

### Phase 1: MVP (2 周)

- [ ] Swarm.toml 解析器
- [ ] 顺序步骤执行
- [ ] 基础数据映射
- [ ] CLI `run` 和 `status` 命令

### Phase 2: 核心功能 (2 周)

- [ ] DAG 调度和并行执行
- [ ] 条件执行引擎
- [ ] 错误处理和重试
- [ ] 完整的 CLI 命令集

### Phase 3: 增强功能 (2 周)

- [ ] 可视化工作流编辑器
- [ ] 实时监控仪表盘
- [ ] 工作流模板库
- [ ] 执行历史分析

---

## 11. 附录

### 11.1 术语表

| 术语 | 定义 |
|------|------|
| **Swarm** | 工作流配置和执行实例 |
| **Step** | 工作流中的一个执行单元，对应一个 Hand |
| **DAG** | 有向无环图，表示步骤依赖关系 |
| **Mapping** | 数据从源到目标的映射规则 |
| **Namespace** | 知识图谱中的隔离空间 |

### 11.2 相关文档

- [Hand System 概览](../自主手系统概览.md)
- [Collector Hand](../Collector（情报收集）.md)
- [Researcher Hand](../Researcher（深度研究）.md)
- [HAND.toml 配置格式](../HAND.toml配置格式.md)

---

## 变更记录

| 版本 | 日期 | 变更内容 | 作者 |
|------|------|----------|------|
| 1.0.0 | 2025-03-28 | 初始版本 | Sisyphus |
