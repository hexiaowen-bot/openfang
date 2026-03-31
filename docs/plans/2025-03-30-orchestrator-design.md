# OpenFang Orchestrator 设计文档

**版本**: MVP  
**日期**: 2025-03-30  
**作者**: OpenFang Team  

---

## 1. 概述

### 1.1 设计目标

基于现有 OpenFang 代码库，构建一个**智能编排器（Orchestrator）**，作为主 Agent 实现：

- **意图识别**: 自动分析任务类型和复杂度
- **智能路由**: 根据复杂度选择执行路径（Hands/Workflow/Swarm）
- **动态编排**: 为复杂任务自动创建和管理子 Agents
- **生命周期管理**: 智能固化高频 Agents，动态销毁低频 Agents
- **执行跟踪**: 实时跟踪子任务状态，支持动态调整
- **持续优化**: 记录执行数据，优化未来编排策略

### 1.2 核心原则

- **最大化复用**: 复用现有的 Workflow、Swarm、Hands、Agents 模块
- **最小化改动**: 新增代码控制在 2000 行以内
- **向后兼容**: 不破坏现有功能
- **渐进增强**: MVP 先实现核心功能，后续迭代优化

### 1.3 决策回顾

基于用户选择的设计决策：

| 决策项 | 选择 | 说明 |
|--------|------|------|
| 固化策略 | 1-C 智能固化 | 基于使用频率/成功率/资源消耗自动决策 |
| 子Session跟踪 | 2-A 事件回调 | 子任务主动上报状态 |
| 评估结果作用 | 3-B/C 实时调整+学习优化 | 动态调整 + 知识积累 |
| 简单vs复杂分界 | 4-D 自主判断 | 主Agent根据任务描述自主决策 |
| Channel配置 | 5-A 全局默认 | 系统启动时配置主Channel |

---

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Orchestrator Agent                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ Intent       │  │ Task Router  │  │ Lifecycle    │              │
│  │ Analyzer     │→ │              │→ │ Manager      │              │
│  │              │  │              │  │              │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│         ↓                  ↓                  ↓                     │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │                    Execution Engine                         │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────────────┐    │   │
│  │  │ Simple     │  │ Medium     │  │ Complex            │    │   │
│  │  │ Executor   │  │ Executor   │  │ Executor           │    │   │
│  │  │ (Hands)    │  │ (Workflow) │  │ (Swarm)            │    │   │
│  │  └────────────┘  └────────────┘  └────────────────────┘    │   │
│  └────────────────────────────────────────────────────────────┘   │
│         ↓                                                           │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │                    Event & Monitor Layer                    │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────────────┐    │   │
│  │  │ Event      │  │ Progress   │  │ Adaptive           │    │   │
│  │  │ Handler    │  │ Tracker    │  │ Adjustments        │    │   │
│  │  └────────────┘  └────────────┘  └────────────────────┘    │   │
│  └────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────┐
│                    Reuse Existing Modules                            │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐    │
│  │ Workflow   │  │ Swarm      │  │ Hands      │  │ Agents     │    │
│  │ Engine     │  │ Engine     │  │ Registry   │  │ Registry   │    │
│  │ (现有)     │  │ (现有)     │  │ (现有)     │  │ (现有)     │    │
│  └────────────┘  └────────────┘  └────────────┘  └────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 模块职责

| 模块 | 职责 | 状态 |
|------|------|------|
| **Intent Analyzer** | 解析任务描述，识别意图和预估复杂度 | 新增 |
| **Task Router** | 根据复杂度选择执行路径 | 新增 |
| **Lifecycle Manager** | 管理 Agents/Hands 的生命周期（固化/销毁） | 新增 |
| **Simple Executor** | 执行简单任务（单 Hands） | 复用 Hands |
| **Medium Executor** | 执行中等任务（Workflow） | 复用 Workflow |
| **Complex Executor** | 执行复杂任务（Swarm） | 复用 Swarm |
| **Event Handler** | 处理子任务状态事件 | 新增 |
| **Progress Tracker** | 跟踪任务进度和性能指标 | 新增 |
| **Adaptive Adjustments** | 实时调整编排策略 | 新增 |

---

## 3. 核心数据模型

### 3.1 编排任务 (OrchestrationTask)

```rust
/// 编排任务请求
pub struct OrchestrationRequest {
    /// 任务唯一ID
    pub id: TaskId,
    /// 任务描述（自然语言）
    pub description: String,
    /// 可选：期望的Channel（None表示使用默认）
    pub channel: Option<String>,
    /// 可选：截止时间
    pub deadline: Option<DateTime<Utc>>,
    /// 可选：用户指定的复杂度（覆盖自动判断）
    pub force_complexity: Option<ComplexityLevel>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// 复杂度级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplexityLevel {
    /// 简单：单步即可完成
    Simple,
    /// 中等：多步顺序/并行执行
    Medium,
    /// 复杂：需要动态编排和协调
    Complex,
}

/// 任务类型分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    Coding,
    Research,
    Writing,
    Analysis,
    DevOps,
    Communication,
    Mixed,
    Other,
}

/// 意图分析结果
pub struct IntentAnalysis {
    /// 任务类型
    pub task_type: TaskType,
    /// 预估复杂度
    pub complexity: ComplexityLevel,
    /// 置信度 (0.0-1.0)
    pub confidence: f64,
    /// 建议的Agents列表
    pub suggested_agents: Vec<String>,
    /// 建议的Hands列表
    pub suggested_hands: Vec<String>,
    /// 预估执行时间（秒）
    pub estimated_duration: u64,
    /// 关键关键词
    pub keywords: Vec<String>,
}
```

### 3.2 执行上下文 (ExecutionContext)

```rust
/// 任务执行上下文
pub struct ExecutionContext {
    /// 任务ID
    pub task_id: TaskId,
    /// 复杂度级别
    pub complexity: ComplexityLevel,
    /// 执行策略
    pub strategy: ExecutionStrategy,
    /// 使用的Agents
    pub agents: Vec<AgentId>,
    /// 使用的Hands
    pub hands: Vec<HandId>,
    /// 创建的Workflow（中等复杂度）
    pub workflow_id: Option<WorkflowId>,
    /// 创建的Swarm（高复杂度）
    pub swarm_id: Option<SwarmExecutionId>,
    /// 执行状态
    pub state: ExecutionState,
    /// 开始时间
    pub started_at: DateTime<Utc>,
    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,
}

/// 执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionState {
    Pending,
    Analyzing,
    Preparing,
    Running,
    Monitoring,
    Adjusting,
    Completing,
    Completed,
    Failed,
    Cancelled,
}

/// 执行策略
pub struct ExecutionStrategy {
    /// 执行路径
    pub path: ExecutionPath,
    /// 最大并行度
    pub max_parallelism: usize,
    /// 超时设置（秒）
    pub timeout_seconds: u64,
    /// 重试策略
    pub retry_policy: RetryPolicy,
    /// 错误处理模式
    pub error_mode: ErrorHandlingMode,
}

/// 执行路径
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionPath {
    /// 简单路径：直接使用Hand
    Simple { hand_id: String },
    /// 中等路径：预定义Workflow
    Medium { workflow_def: Workflow },
    /// 复杂路径：动态Swarm
    Complex { swarm_def: SwarmDefinition },
}
```

### 3.3 Agent 生命周期 (AgentLifecycle)

```rust
/// Agent实例信息
pub struct ManagedAgent {
    /// Agent ID
    pub agent_id: AgentId,
    /// Agent名称
    pub name: String,
    /// 模板ID（如果是从模板创建）
    pub template_id: Option<String>,
    /// Hand ID（如果是Hand实例）
    pub hand_id: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后使用时间
    pub last_used_at: DateTime<Utc>,
    /// 使用次数
    pub use_count: u64,
    /// 成功次数
    pub success_count: u64,
    /// 总Token消耗
    pub total_tokens: u64,
    /// 平均执行时间（毫秒）
    pub avg_duration_ms: u64,
    /// 状态
    pub status: ManagedAgentStatus,
    /// 固化状态
    pub retention: RetentionStatus,
}

/// 管理状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManagedAgentStatus {
    Active,
    Idle,
    Suspended,
    Destroying,
}

/// 固化状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetentionStatus {
    /// 永久固化（高频/高成功率）
    Permanent,
    /// 临时缓存（LRU）
    Cached,
    /// 动态（用完即销毁）
    Dynamic,
}

/// 固化决策
pub struct RetentionDecision {
    pub agent_id: AgentId,
    pub decision: RetentionStatus,
    /// 综合评分 (0-100)
    pub score: u64,
    /// 评分详情
    pub metrics: RetentionMetrics,
}

/// 固化指标
pub struct RetentionMetrics {
    /// 使用频率得分 (30%)
    pub frequency_score: u64,
    /// 成功率得分 (40%)
    pub success_rate_score: u64,
    /// 资源效率得分 (20%)
    pub efficiency_score: u64,
    /// 响应速度得分 (10%)
    pub speed_score: u64,
}
```

### 3.4 事件系统 (Event System)

```rust
/// 编排事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestrationEvent {
    /// 任务开始分析
    AnalysisStarted { task_id: TaskId },
    /// 分析完成
    AnalysisCompleted { 
        task_id: TaskId, 
        analysis: IntentAnalysis 
    },
    /// 执行开始
    ExecutionStarted { 
        task_id: TaskId, 
        strategy: ExecutionStrategy 
    },
    /// 进度更新
    ProgressUpdated {
        task_id: TaskId,
        progress: f64,  // 0.0-1.0
        message: String,
    },
    /// 里程碑达成
    MilestoneReached {
        task_id: TaskId,
        milestone: String,
        details: serde_json::Value,
    },
    /// Agent被创建
    AgentCreated {
        task_id: TaskId,
        agent_id: AgentId,
        reason: String,
    },
    /// 需要调整编排
    AdjustmentNeeded {
        task_id: TaskId,
        reason: String,
        suggested_action: AdjustmentAction,
    },
    /// 编排已调整
    AdjustmentApplied {
        task_id: TaskId,
        action: AdjustmentAction,
        new_strategy: ExecutionStrategy,
    },
    /// 任务完成
    TaskCompleted {
        task_id: TaskId,
        output: String,
        metrics: TaskMetrics,
    },
    /// 任务失败
    TaskFailed {
        task_id: TaskId,
        error: String,
        recoverable: bool,
    },
    /// 任务取消
    TaskCancelled { task_id: TaskId },
}

/// 调整动作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdjustmentAction {
    /// 更换Agent
    SwitchAgent { from: AgentId, to: AgentId },
    /// 拆分任务
    SplitTask { subtasks: Vec<String> },
    /// 增加重试
    IncreaseRetry { step_id: String, max_retries: u32 },
    /// 更换执行路径
    ChangePath { new_path: ExecutionPath },
    /// 并行化步骤
    Parallelize { step_ids: Vec<String> },
    /// 人工介入
    RequestHumanIntervention { reason: String },
}

/// 任务指标
pub struct TaskMetrics {
    /// 总耗时（毫秒）
    pub total_duration_ms: u64,
    /// 实际执行时间（毫秒）
    pub execution_duration_ms: u64,
    /// Token消耗
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// 使用的Agents数量
    pub agent_count: usize,
    /// 步骤数
    pub step_count: usize,
    /// 重试次数
    pub retry_count: u32,
    /// 调整次数
    pub adjustment_count: u32,
}
```

---

## 4. 核心组件设计

### 4.1 意图分析器 (IntentAnalyzer)

**职责**: 解析任务描述，识别意图和预估复杂度

**实现策略**: 
- MVP版本使用基于规则的分类器（关键词匹配 + 正则）
- 后续可升级为LLM-based分类器

```rust
pub struct IntentAnalyzer {
    /// 关键词词典
    keyword_db: HashMap<TaskType, Vec<String>>,
    /// 复杂度判定规则
    complexity_rules: Vec<ComplexityRule>,
    /// 历史学习数据
    learned_patterns: Vec<LearnedPattern>,
}

impl IntentAnalyzer {
    /// 分析任务描述
    pub async fn analyze(&self, description: &str) -> IntentAnalysis {
        // 1. 提取关键词
        let keywords = self.extract_keywords(description);
        
        // 2. 判定任务类型
        let task_type = self.classify_task_type(&keywords);
        
        // 3. 预估复杂度
        let (complexity, confidence) = self.estimate_complexity(description, &keywords);
        
        // 4. 推荐Agents和Hands
        let suggested_agents = self.suggest_agents(task_type, &keywords);
        let suggested_hands = self.suggest_hands(task_type, &keywords);
        
        // 5. 预估执行时间
        let estimated_duration = self.estimate_duration(complexity, task_type);
        
        IntentAnalysis {
            task_type,
            complexity,
            confidence,
            suggested_agents,
            suggested_hands,
            estimated_duration,
            keywords,
        }
    }
}
```

**复杂度判定规则**:

| 复杂度 | 判定条件 | 示例 |
|--------|----------|------|
| Simple | 单领域 + 明确输出 + 无依赖 | "格式化这段JSON" |
| Medium | 多步骤 + 顺序/并行 + 领域单一 | "分析代码并生成报告" |
| Complex | 跨领域 + 动态依赖 + 需协调 | "设计并实现一个微服务架构" |

### 4.2 任务路由器 (TaskRouter)

**职责**: 根据复杂度选择执行路径

```rust
pub struct TaskRouter {
    /// Hands注册表引用
    hands_registry: Arc<HandsRegistry>,
    /// Workflow引擎引用
    workflow_engine: Arc<WorkflowEngine>,
    /// Swarm引擎引用
    swarm_engine: Arc<SwarmEngine>,
    /// 模板注册表引用
    template_registry: Arc<AgentTemplateRegistry>,
}

impl TaskRouter {
    /// 路由任务到合适的执行器
    pub async fn route(
        &self, 
        request: &OrchestrationRequest,
        analysis: &IntentAnalysis
    ) -> Result<ExecutionStrategy, RouterError> {
        match analysis.complexity {
            ComplexityLevel::Simple => {
                self.build_simple_strategy(request, analysis).await
            }
            ComplexityLevel::Medium => {
                self.build_medium_strategy(request, analysis).await
            }
            ComplexityLevel::Complex => {
                self.build_complex_strategy(request, analysis).await
            }
        }
    }
    
    /// 构建简单执行策略（使用Hands）
    async fn build_simple_strategy(
        &self,
        request: &OrchestrationRequest,
        analysis: &IntentAnalysis
    ) -> Result<ExecutionStrategy, RouterError> {
        // 选择最佳Hand
        let hand_id = self.select_best_hand(analysis).await?;
        
        Ok(ExecutionStrategy {
            path: ExecutionPath::Simple { hand_id },
            max_parallelism: 1,
            timeout_seconds: 120,
            retry_policy: RetryPolicy::default(),
            error_mode: ErrorHandlingMode::FailFast,
        })
    }
    
    /// 构建中等执行策略（使用Workflow）
    async fn build_medium_strategy(
        &self,
        request: &OrchestrationRequest,
        analysis: &IntentAnalysis
    ) -> Result<ExecutionStrategy, RouterError> {
        // 根据任务类型生成或选择预设Workflow
        let workflow = self.generate_workflow(analysis).await?;
        
        Ok(ExecutionStrategy {
            path: ExecutionPath::Medium { workflow_def: workflow },
            max_parallelism: 3,
            timeout_seconds: 600,
            retry_policy: RetryPolicy::with_retries(2),
            error_mode: ErrorHandlingMode::RetryThenSkip,
        })
    }
    
    /// 构建复杂执行策略（使用Swarm）
    async fn build_complex_strategy(
        &self,
        request: &OrchestrationRequest,
        analysis: &IntentAnalysis
    ) -> Result<ExecutionStrategy, RouterError> {
        // 动态生成Swarm定义
        let swarm_def = self.generate_swarm_definition(analysis).await?;
        
        Ok(ExecutionStrategy {
            path: ExecutionPath::Complex { swarm_def },
            max_parallelism: 10,
            timeout_seconds: 3600,
            retry_policy: RetryPolicy::with_retries(3),
            error_mode: ErrorHandlingMode::Adaptive,
        })
    }
}
```

### 4.3 执行器 (Executors)

#### 4.3.1 简单执行器 (SimpleExecutor)

```rust
pub struct SimpleExecutor {
    hands_registry: Arc<HandsRegistry>,
    lifecycle_manager: Arc<LifecycleManager>,
}

impl SimpleExecutor {
    /// 执行简单任务
    pub async fn execute(
        &self,
        context: &mut ExecutionContext,
        hand_id: &str,
        input: &str,
        event_sender: EventSender,
    ) -> Result<String, ExecutionError> {
        // 1. 确保Hand已激活
        let hand_instance = self.lifecycle_manager
            .ensure_hand_active(hand_id)
            .await?;
        
        // 2. 发送执行事件
        event_sender.send(OrchestrationEvent::ExecutionStarted {
            task_id: context.task_id,
            strategy: context.strategy.clone(),
        }).await?;
        
        // 3. 调用Hand执行
        let start = Instant::now();
        let result = hand_instance.execute(input).await;
        let duration = start.elapsed();
        
        // 4. 更新指标
        self.lifecycle_manager.update_metrics(
            &hand_instance.agent_id,
            result.is_ok(),
            duration,
        ).await;
        
        // 5. 返回结果
        result
    }
}
```

#### 4.3.2 中等执行器 (MediumExecutor)

```rust
pub struct MediumExecutor {
    workflow_engine: Arc<WorkflowEngine>,
    agent_resolver: Arc<AgentResolver>,
    lifecycle_manager: Arc<LifecycleManager>,
}

impl MediumExecutor {
    /// 执行中等复杂度任务（Workflow）
    pub async fn execute(
        &self,
        context: &mut ExecutionContext,
        workflow: &Workflow,
        input: &str,
        event_sender: EventSender,
    ) -> Result<String, ExecutionError> {
        // 1. 注册Workflow
        let wf_id = self.workflow_engine.register(workflow.clone()).await;
        
        // 2. 创建运行实例
        let run_id = self.workflow_engine
            .create_run(wf_id, input.to_string())
            .await
            .ok_or(ExecutionError::WorkflowCreationFailed)?;
        
        // 3. 执行Workflow
        let result = self.workflow_engine
            .execute_run(
                run_id,
                |step_agent| self.agent_resolver.resolve(step_agent),
                |agent_id, prompt| self.execute_agent(agent_id, prompt, event_sender.clone()),
            )
            .await;
        
        // 4. 获取执行记录
        let run = self.workflow_engine.get_run(run_id).await;
        
        result
    }
}
```

#### 4.3.3 复杂执行器 (ComplexExecutor)

```rust
pub struct ComplexExecutor {
    swarm_engine: Arc<SwarmEngine>,
    lifecycle_manager: Arc<LifecycleManager>,
    event_sender: EventSender,
}

impl ComplexExecutor {
    /// 执行复杂任务（Swarm）
    pub async fn execute(
        &self,
        context: &mut ExecutionContext,
        swarm_def: &SwarmDefinition,
        input: HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, ExecutionError> {
        // 1. 加载Swarm定义
        let def_id = self.swarm_engine
            .load_definition(&toml::to_string(swarm_def).unwrap())
            .await?;
        
        // 2. 创建执行实例
        let exec_id = self.swarm_engine
            .create_execution(&def_id, input)
            .await?;
        
        // 3. 执行并监控
        let result = self.swarm_engine
            .execute(&exec_id, |hand, prompt| {
                self.execute_hand_with_events(hand, prompt, context.task_id)
            })
            .await;
        
        result
    }
}
```

### 4.4 生命周期管理器 (LifecycleManager)

```rust
pub struct LifecycleManager {
    /// Agent缓存池
    agent_pool: Arc<RwLock<HashMap<AgentId, ManagedAgent>>>,
    /// Hand实例缓存
    hand_pool: Arc<RwLock<HashMap<String, HandInstance>>>,
    /// LRU缓存（用于临时固化）
    lru_cache: Arc<RwLock<LruCache<AgentId, ()>>>,
    /// 模板注册表
    template_registry: Arc<AgentTemplateRegistry>,
    /// Hands注册表
    hands_registry: Arc<HandsRegistry>,
    /// 内核引用（用于创建agents）
    kernel: Arc<OpenFangKernel>,
    /// 配置
    config: LifecycleConfig,
}

pub struct LifecycleConfig {
    /// 最大常驻Agents数
    pub max_resident_agents: usize,
    /// LRU缓存大小
    pub lru_cache_size: usize,
    /// 动态Agent空闲超时（秒）
    pub dynamic_idle_timeout: u64,
    /// 固化评分阈值
    pub retention_thresholds: RetentionThresholds,
}

impl LifecycleManager {
    /// 获取或创建Agent
    pub async fn get_or_create_agent(
        &self,
        template_id: Option<&str>,
        hand_id: Option<&str>,
        name: &str,
    ) -> Result<AgentId, LifecycleError> {
        // 1. 检查是否已有合适的Agent
        if let Some(agent_id) = self.find_existing_agent(template_id, hand_id).await {
            self.touch_agent(&agent_id).await;
            return Ok(agent_id);
        }
        
        // 2. 创建新Agent
        let agent_id = if let Some(tid) = template_id {
            self.create_from_template(tid, name).await?
        } else if let Some(hid) = hand_id {
            self.create_from_hand(hid, name).await?
        } else {
            return Err(LifecycleError::NoTemplateOrHand);
        };
        
        // 3. 注册到管理池
        self.register_agent(agent_id, template_id, hand_id).await;
        
        Ok(agent_id)
    }
    
    /// 评估并执行固化决策
    pub async fn evaluate_retention(&self, agent_id: &AgentId) -> RetentionDecision {
        let agent = self.get_managed_agent(agent_id).await;
        
        // 计算各项指标得分
        let frequency_score = self.calc_frequency_score(&agent);
        let success_rate_score = self.calc_success_rate_score(&agent);
        let efficiency_score = self.calc_efficiency_score(&agent);
        let speed_score = self.calc_speed_score(&agent);
        
        // 计算综合得分
        let total_score = (
            frequency_score * 30 +
            success_rate_score * 40 +
            efficiency_score * 20 +
            speed_score * 10
        ) / 100;
        
        // 决策
        let decision = if total_score >= 80 {
            RetentionStatus::Permanent
        } else if total_score >= 60 {
            RetentionStatus::Cached
        } else {
            RetentionStatus::Dynamic
        };
        
        RetentionDecision {
            agent_id: *agent_id,
            decision,
            score: total_score,
            metrics: RetentionMetrics {
                frequency_score,
                success_rate_score,
                efficiency_score,
                speed_score,
            },
        }
    }
    
    /// 应用固化决策
    pub async fn apply_retention_decision(&self, decision: &RetentionDecision) {
        match decision.decision {
            RetentionStatus::Permanent => {
                // 永久固化，加入常驻池
                self.promote_to_permanent(&decision.agent_id).await;
            }
            RetentionStatus::Cached => {
                // 加入LRU缓存
                self.add_to_lru(&decision.agent_id).await;
            }
            RetentionStatus::Dynamic => {
                // 标记为动态，空闲时销毁
                self.mark_as_dynamic(&decision.agent_id).await;
            }
        }
    }
}
```

### 4.5 事件处理器 (EventHandler)

```rust
pub struct EventHandler {
    /// 事件订阅者
    subscribers: Arc<RwLock<Vec<EventSubscriber>>>,
    /// 任务状态存储
    task_states: Arc<RwLock<HashMap<TaskId, TaskState>>>,
    /// 调整处理器
    adjustment_handler: Arc<AdjustmentHandler>,
}

pub type EventSubscriber = Box<dyn Fn(OrchestrationEvent) -> BoxFuture<'static, ()> + Send + Sync>;

impl EventHandler {
    /// 订阅事件
    pub async fn subscribe(&self, subscriber: EventSubscriber) {
        self.subscribers.write().await.push(subscriber);
    }
    
    /// 发送事件
    pub async fn emit(&self, event: OrchestrationEvent) {
        // 1. 更新任务状态
        self.update_task_state(&event).await;
        
        // 2. 检查是否需要调整
        if let Some(adjustment) = self.check_adjustment_needed(&event).await {
            self.adjustment_handler.handle(adjustment).await;
        }
        
        // 3. 广播给订阅者
        let subscribers = self.subscribers.read().await.clone();
        for subscriber in subscribers {
            let event_clone = event.clone();
            tokio::spawn(async move {
                subscriber(event_clone).await;
            });
        }
    }
    
    /// 处理子任务状态事件
    pub async fn handle_subtask_event(
        &self,
        task_id: TaskId,
        subtask_id: String,
        status: SubtaskStatus,
    ) {
        let event = match status {
            SubtaskStatus::Progress(pct) => {
                OrchestrationEvent::ProgressUpdated {
                    task_id,
                    progress: pct,
                    message: format!("Subtask {} progress: {:.0}%", subtask_id, pct * 100.0),
                }
            }
            SubtaskStatus::Milestone(m) => {
                OrchestrationEvent::MilestoneReached {
                    task_id,
                    milestone: m.name,
                    details: m.details,
                }
            }
            SubtaskStatus::Completed(output) => {
                // 子任务完成，检查整体进度
                self.check_completion(task_id).await;
                return;
            }
            SubtaskStatus::Failed(error) => {
                OrchestrationEvent::TaskFailed {
                    task_id,
                    error,
                    recoverable: true,
                }
            }
        };
        
        self.emit(event).await;
    }
}
```

### 4.6 自适应调整器 (AdaptiveAdjuster)

```rust
pub struct AdaptiveAdjuster {
    /// 调整策略库
    strategies: Vec<Box<dyn AdjustmentStrategy>>,
    /// 执行上下文引用
    context_store: Arc<RwLock<HashMap<TaskId, ExecutionContext>>>,
}

#[async_trait]
pub trait AdjustmentStrategy: Send + Sync {
    /// 检查是否需要调整
    fn check(&self, event: &OrchestrationEvent, context: &ExecutionContext) -> Option<AdjustmentAction>;
    
    /// 应用调整
    async fn apply(&self, action: &AdjustmentAction, context: &mut ExecutionContext) -> Result<(), AdjustmentError>;
}

/// 进度停滞调整策略
pub struct StagnationStrategy;

#[async_trait]
impl AdjustmentStrategy for StagnationStrategy {
    fn check(&self, event: &OrchestrationEvent, context: &ExecutionContext) -> Option<AdjustmentAction> {
        // 如果进度长时间未更新，建议更换Agent
        if let OrchestrationEvent::ProgressUpdated { progress, .. } = event {
            if *progress < 0.1 && context.started_at.elapsed() > Duration::from_secs(60) {
                return Some(AdjustmentAction::SwitchAgent {
                    from: context.agents[0],
                    to: AgentId::new(), // 新Agent
                });
            }
        }
        None
    }
    
    async fn apply(&self, action: &AdjustmentAction, context: &mut ExecutionContext) -> Result<(), AdjustmentError> {
        // 实现Agent切换逻辑
        Ok(())
    }
}

/// 失败重试策略
pub struct RetryStrategy;

#[async_trait]
impl AdjustmentStrategy for RetryStrategy {
    fn check(&self, event: &OrchestrationEvent, context: &ExecutionContext) -> Option<AdjustmentAction> {
        if let OrchestrationEvent::TaskFailed { error, recoverable, .. } = event {
            if *recoverable && context.retry_count < 3 {
                return Some(AdjustmentAction::IncreaseRetry {
                    step_id: "current".to_string(),
                    max_retries: context.retry_count + 1,
                });
            }
        }
        None
    }
    
    async fn apply(&self, action: &AdjustmentAction, context: &mut ExecutionContext) -> Result<(), AdjustmentError> {
        context.retry_count += 1;
        Ok(())
    }
}
```

---

## 5. 执行流程

### 5.1 主执行流程

```
┌─────────────────────────────────────────────────────────────────┐
│  1. 接收任务                                                     │
│     输入: 自然语言描述                                           │
│     输出: OrchestrationRequest                                   │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  2. 意图分析 (IntentAnalyzer)                                    │
│     - 提取关键词                                                 │
│     - 分类任务类型                                               │
│     - 预估复杂度                                                 │
│     - 推荐Agents/Hands                                           │
│     输出: IntentAnalysis                                         │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  3. 任务路由 (TaskRouter)                                        │
│     根据复杂度选择路径:                                           │
│     ├─ Simple  → SimpleExecutor (Hands)                         │
│     ├─ Medium  → MediumExecutor (Workflow)                      │
│     └─ Complex → ComplexExecutor (Swarm)                        │
│     输出: ExecutionStrategy                                      │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  4. 准备执行                                                     │
│     - 获取/创建必要的Agents                                       │
│     - 注册到LifecycleManager                                     │
│     - 创建ExecutionContext                                       │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  5. 执行任务                                                     │
│     - 调用对应Executor                                           │
│     - 实时发送进度事件                                           │
│     - 监控执行状态                                               │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  6. 自适应调整 (AdaptiveAdjuster)                                │
│     - 监听执行事件                                               │
│     - 检测异常情况                                               │
│     - 应用调整策略                                               │
│     - 更新执行策略                                               │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  7. 完成处理                                                     │
│     - 收集执行结果                                               │
│     - 计算性能指标                                               │
│     - 执行固化决策                                               │
│     - 记录学习数据                                               │
│     输出: 任务结果 + TaskMetrics                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 5.2 简单任务执行流程

```rust
async fn execute_simple_task(
    &self,
    request: OrchestrationRequest,
    analysis: IntentAnalysis,
) -> Result<String, OrchestratorError> {
    // 1. 选择最佳Hand
    let hand_id = analysis.suggested_hands[0].clone();
    
    // 2. 确保Hand已激活（LifecycleManager管理）
    let hand = self.lifecycle_manager.ensure_hand_active(&hand_id).await?;
    
    // 3. 执行
    let result = hand.execute(&request.description).await?;
    
    // 4. 更新使用统计
    self.lifecycle_manager.update_hand_usage(&hand_id).await;
    
    // 5. 评估固化决策
    let decision = self.lifecycle_manager.evaluate_retention(&hand.agent_id).await;
    self.lifecycle_manager.apply_retention_decision(&decision).await;
    
    Ok(result)
}
```

### 5.3 复杂任务执行流程

```rust
async fn execute_complex_task(
    &self,
    request: OrchestrationRequest,
    analysis: IntentAnalysis,
) -> Result<serde_json::Value, OrchestratorError> {
    // 1. 动态生成Swarm定义
    let swarm_def = self.generate_swarm_definition(&analysis).await;
    
    // 2. 为每个步骤创建/获取Agent
    for step in &swarm_def.steps {
        let agent_id = self.lifecycle_manager
            .get_or_create_agent(None, Some(&step.hand), &step.name)
            .await?;
        context.agents.push(agent_id);
    }
    
    // 3. 加载Swarm定义并创建执行
    let def_id = self.swarm_engine.load_definition(&swarm_def.to_toml()).await?;
    let exec_id = self.swarm_engine.create_execution(&def_id, input).await?;
    
    // 4. 执行并监听事件
    let event_handler = self.event_handler.clone();
    let mut event_stream = self.swarm_engine.subscribe_events(&exec_id);
    
    tokio::spawn(async move {
        while let Some(event) = event_stream.next().await {
            event_handler.handle_swarm_event(event).await;
        }
    });
    
    // 5. 执行Swarm
    let result = self.swarm_engine
        .execute(&exec_id, |hand, prompt| {
            self.execute_hand(hand, prompt)
        })
        .await?;
    
    // 6. 批量评估所有使用的Agents
    for agent_id in &context.agents {
        let decision = self.lifecycle_manager.evaluate_retention(agent_id).await;
        self.lifecycle_manager.apply_retention_decision(&decision).await;
    }
    
    Ok(result)
}
```

---

## 6. 复用现有模块的策略

### 6.1 复用模块清单

| 现有模块 | 复用方式 | 新增代码 |
|----------|----------|----------|
| `openfang-kernel/src/workflow.rs` | 直接使用 `WorkflowEngine` | 0行 |
| `openfang-kernel/src/swarm.rs` | 直接使用 `SwarmEngine` | 0行 |
| `openfang-hands/src/registry.rs` | 通过 `HandsRegistry` 访问 | ~50行（包装） |
| `openfang-agents/src/registry.rs` | 通过 `AgentTemplateRegistry` 访问 | ~50行（包装） |
| `openfang-types/src/agent.rs` | 直接使用数据类型 | 0行 |
| `openfang-types/src/message.rs` | 直接使用消息类型 | 0行 |

### 6.2 扩展现有模块

#### 6.2.1 扩展 WorkflowEngine

```rust
// 在 workflow.rs 中添加
impl WorkflowEngine {
    /// 订阅Workflow运行事件（新增方法）
    pub async fn subscribe_events(&self, run_id: WorkflowRunId) -> EventStream {
        // 返回事件流
    }
}
```

#### 6.2.2 扩展 SwarmEngine

```rust
// 在 swarm.rs 中添加
impl SwarmEngine {
    /// 订阅Swarm执行事件（新增方法）
    pub async fn subscribe_events(&self, exec_id: &str) -> EventStream {
        // 返回事件流
    }
    
    /// 动态调整步骤（新增方法）
    pub async fn adjust_step(&self, exec_id: &str, step_id: &str, adjustment: StepAdjustment) {
        // 应用步骤调整
    }
}
```

---

## 7. 新增代码清单

### 7.1 文件结构

```
crates/openfang-kernel/src/
├── orchestrator/
│   ├── mod.rs              # 模块导出
│   ├── analyzer.rs         # 意图分析器 (~300行)
│   ├── router.rs           # 任务路由器 (~200行)
│   ├── lifecycle.rs        # 生命周期管理器 (~400行)
│   ├── executor.rs         # 执行器实现 (~300行)
│   ├── event.rs            # 事件系统 (~250行)
│   ├── adaptive.rs         # 自适应调整 (~250行)
│   └── learning.rs         # 学习优化 (~150行)
├── lib.rs                  # 导出Orchestrator

crates/openfang-api/src/
└── routes/
    └── orchestrator.rs     # API端点 (~200行)
```

### 7.2 预估代码量

| 组件 | 预估行数 | 说明 |
|------|----------|------|
| Intent Analyzer | 300 | 基于规则的分类器 |
| Task Router | 200 | 路由逻辑 |
| Lifecycle Manager | 400 | 固化策略 + LRU缓存 |
| Executors | 300 | 包装现有模块 |
| Event System | 250 | 事件定义 + 处理 |
| Adaptive Adjuster | 250 | 调整策略 |
| Learning System | 150 | 数据记录 |
| API Routes | 200 | REST端点 |
| **总计** | **~2050行** | |

---

## 8. API 设计

### 8.1 REST API

```yaml
# 提交编排任务
POST /api/v1/orchestrate
Request:
  {
    "description": "分析这段代码并生成文档",
    "channel": "slack",           # 可选
    "deadline": "2025-03-31T00:00:00Z",  # 可选
    "force_complexity": "medium"  # 可选：simple/medium/complex
  }

Response:
  {
    "task_id": "task-uuid",
    "status": "analyzing",
    "estimated_duration": 120,
    "analysis": {
      "task_type": "coding",
      "complexity": "medium",
      "confidence": 0.85,
      "suggested_agents": ["code-analyzer", "tech-writer"]
    }
  }

# 查询任务状态
GET /api/v1/orchestrate/{task_id}
Response:
  {
    "task_id": "task-uuid",
    "status": "running",
    "progress": 0.45,
    "current_step": "生成文档",
    "agents": ["agent-1", "agent-2"],
    "started_at": "2025-03-30T10:00:00Z",
    "estimated_completion": "2025-03-30T10:02:00Z"
  }

# 获取任务结果
GET /api/v1/orchestrate/{task_id}/result
Response:
  {
    "task_id": "task-uuid",
    "status": "completed",
    "output": "生成的文档内容...",
    "metrics": {
      "total_duration_ms": 115000,
      "input_tokens": 1500,
      "output_tokens": 2500,
      "agent_count": 2,
      "step_count": 3
    }
  }

# 获取任务事件流 (SSE)
GET /api/v1/orchestrate/{task_id}/events
Content-Type: text/event-stream

event: progress
data: {"progress": 0.3, "message": "正在分析代码..."}

event: milestone
data: {"milestone": "analysis_complete"}

event: completed
data: {"output": "..."}

# 取消任务
DELETE /api/v1/orchestrate/{task_id}
Response: { "status": "cancelled" }

# 获取托管Agents列表
GET /api/v1/orchestrate/agents
Response:
  {
    "agents": [
      {
        "agent_id": "agent-uuid",
        "name": "code-analyzer",
        "status": "active",
        "retention": "permanent",
        "use_count": 150,
        "success_rate": 0.95
      }
    ],
    "stats": {
      "total": 10,
      "permanent": 3,
      "cached": 5,
      "dynamic": 2
    }
  }

# 手动触发固化评估
POST /api/v1/orchestrate/agents/{agent_id}/evaluate
Response:
  {
    "agent_id": "agent-uuid",
    "decision": "permanent",
    "score": 85,
    "metrics": { ... }
  }
```

### 8.2 CLI 命令

```bash
# 提交任务
openfang orchestrate "分析这段代码并生成文档"

# 提交任务（指定复杂度）
openfang orchestrate "设计微服务架构" --complexity complex

# 查看任务状态
openfang orchestrate status <task-id>

# 查看任务结果
openfang orchestrate result <task-id>

# 列出托管Agents
openfang orchestrate agents list

# 查看Agent详情
openfang orchestrate agents show <agent-id>

# 手动清理动态Agents
openfang orchestrate agents cleanup
```

---

## 9. 配置示例

### 9.1 Orchestrator 配置

```toml
# openfang.toml
[orchestrator]
# 意图分析
[orchestrator.analyzer]
# 复杂度判定阈值
simple_confidence_threshold = 0.8
medium_confidence_threshold = 0.6

# 生命周期管理
[orchestrator.lifecycle]
max_resident_agents = 20
lru_cache_size = 10
dynamic_idle_timeout = 300  # 5分钟

# 固化评分阈值
[orchestrator.lifecycle.retention]
permanent_threshold = 80    # 80分以上永久固化
cache_threshold = 60        # 60-80分临时缓存

# 自适应调整
[orchestrator.adaptive]
enabled = true
stagnation_timeout = 60     # 60秒无进度触发调整
max_adjustments = 5         # 单任务最大调整次数

# 学习系统
[orchestrator.learning]
enabled = true
history_retention_days = 30
```

---

## 10. 后续优化方向

### 10.1 MVP 之后 (Phase 2)

1. **智能意图识别**: 升级为LLM-based分类器
2. **Workflow模板库**: 预定义常见任务流程
3. **Swarm可视化**: Web UI展示执行DAG
4. **A/B测试**: 对比不同编排策略效果

### 10.2 长期规划 (Phase 3)

1. **强化学习**: 基于历史数据优化编排决策
2. **预测性固化**: 预判任务需求，提前准备Agents
3. **跨任务优化**: 识别任务间依赖，批量优化
4. **联邦学习**: 多节点共享学习数据

---

## 11. 总结

本设计基于现有 OpenFang 代码库，通过最大化复用现有模块（Workflow、Swarm、Hands、Agents），新增约 **2000 行代码**实现智能编排器核心功能：

- ✅ **意图识别**: 基于规则的任务分类
- ✅ **智能路由**: 根据复杂度自动选择执行路径
- ✅ **生命周期管理**: 智能固化策略（LRU + 评分）
- ✅ **事件驱动**: 实时跟踪和自适应调整
- ✅ **学习优化**: 数据记录和分析

**关键设计决策**:
- 简单任务 → Hands（单步）
- 中等任务 → Workflow（预定义多步）
- 复杂任务 → Swarm（动态DAG）

**核心价值**:
- 用户只需描述任务，系统自动完成编排
- 支持 100+ Agents 的协调管理
- 持续学习和优化编排策略
