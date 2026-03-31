//! Agent Lifecycle Manager
//!
//! Manages the lifecycle of agents including registration, usage tracking,
//! retention evaluation, and cleanup of idle agents.

use chrono::Utc;
use openfang_types::agent::AgentId;
use openfang_types::orchestrator::{
    ManagedAgent, ManagedAgentStatus, RetentionDecision, RetentionMetrics, RetentionStatus,
};
use std::collections::HashMap;
use std::sync::RwLock;

/// Agent 生命周期管理池
pub struct ManagedAgentPool {
    agents: RwLock<HashMap<AgentId, ManagedAgent>>,
}

impl ManagedAgentPool {
    /// Create a new empty agent pool
    pub fn new() -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
        }
    }

    /// 注册一个新的受管 Agent
    pub fn register(
        &self,
        agent_id: AgentId,
        name: String,
        template_id: Option<String>,
        hand_id: Option<String>,
    ) {
        let now = Utc::now();
        let agent = ManagedAgent {
            agent_id,
            name,
            template_id,
            hand_id,
            created_at: now,
            last_used_at: now,
            use_count: 0,
            success_count: 0,
            total_tokens: 0,
            avg_duration_ms: 0,
            status: ManagedAgentStatus::Idle,
            retention: RetentionStatus::Dynamic,
        };

        let mut agents = self.agents.write().unwrap();
        agents.insert(agent_id, agent);
    }

    /// 记录 Agent 使用数据
    pub fn record_usage(&self, agent_id: &AgentId, success: bool, tokens: u64, duration_ms: u64) {
        let mut agents = self.agents.write().unwrap();

        if let Some(agent) = agents.get_mut(agent_id) {
            // Update use count
            agent.use_count += 1;

            // Update success count
            if success {
                agent.success_count += 1;
            }

            // Update total tokens
            agent.total_tokens += tokens;

            // Recalculate average duration: (old_avg * (use_count-1) + duration_ms) / use_count
            let old_avg = agent.avg_duration_ms;
            let new_count = agent.use_count;
            agent.avg_duration_ms =
                ((old_avg * (new_count - 1)) + duration_ms) / new_count;

            // Update last used time and status
            agent.last_used_at = Utc::now();
            agent.status = ManagedAgentStatus::Active;
        }
    }

    /// 评估 Agent 固化策略
    pub fn evaluate_retention(&self, agent_id: &AgentId) -> Option<RetentionDecision> {
        let agents = self.agents.read().unwrap();

        let agent = agents.get(agent_id)?;

        // Calculate frequency score (30%)
        let frequency_score = if agent.use_count >= 10 {
            30
        } else if agent.use_count >= 5 {
            20
        } else if agent.use_count >= 2 {
            10
        } else {
            0
        };

        // Calculate success rate score (40%)
        let success_rate = if agent.use_count > 0 {
            agent.success_count as f64 / agent.use_count as f64
        } else {
            0.0
        };

        let success_rate_score = if success_rate >= 0.9 {
            40
        } else if success_rate >= 0.7 {
            30
        } else if success_rate >= 0.5 {
            20
        } else {
            0
        };

        // Calculate efficiency score (20%)
        let avg_tokens = if agent.use_count > 0 {
            agent.total_tokens / agent.use_count
        } else {
            0
        };

        let efficiency_score = if avg_tokens <= 1000 {
            20
        } else if avg_tokens <= 5000 {
            15
        } else if avg_tokens <= 10000 {
            10
        } else {
            5
        };

        // Calculate speed score (10%)
        let speed_score = if agent.avg_duration_ms <= 5000 {
            10
        } else if agent.avg_duration_ms <= 15000 {
            7
        } else if agent.avg_duration_ms <= 30000 {
            4
        } else {
            2
        };

        // Calculate total score
        let total_score = frequency_score + success_rate_score + efficiency_score + speed_score;

        // Determine retention status
        let decision = if total_score >= 70 {
            RetentionStatus::Permanent
        } else if total_score >= 40 {
            RetentionStatus::Cached
        } else {
            RetentionStatus::Dynamic
        };

        Some(RetentionDecision {
            agent_id: *agent_id,
            decision,
            score: total_score,
            metrics: RetentionMetrics {
                frequency_score,
                success_rate_score,
                efficiency_score,
                speed_score,
            },
        })
    }

    /// 清理空闲 Agents，返回需要销毁的 Agent ID 列表
    pub fn cleanup_idle(&self, max_idle_secs: u64) -> Vec<AgentId> {
        let mut agents = self.agents.write().unwrap();
        let now = Utc::now();
        let mut to_destroy = Vec::new();

        for (agent_id, agent) in agents.iter_mut() {
            let idle_duration = (now - agent.last_used_at).num_seconds() as u64;

            // Determine idle threshold based on retention status
            let threshold = match agent.retention {
                RetentionStatus::Permanent => {
                    // Permanent agents are never cleaned up
                    continue;
                }
                RetentionStatus::Cached => max_idle_secs * 3,
                RetentionStatus::Dynamic => max_idle_secs,
            };

            if idle_duration > threshold {
                agent.status = ManagedAgentStatus::Destroying;
                to_destroy.push(*agent_id);
            }
        }

        to_destroy
    }

    /// 根据名称查找已有 Agent（用于复用）
    pub fn find_reusable(&self, name: &str) -> Option<AgentId> {
        let agents = self.agents.read().unwrap();

        let mut permanent_candidate: Option<AgentId> = None;
        let mut cached_candidate: Option<AgentId> = None;

        for (agent_id, agent) in agents.iter() {
            // Check if name matches and status is Active or Idle
            if agent.name == name
                && (agent.status == ManagedAgentStatus::Active || agent.status == ManagedAgentStatus::Idle)
            {
                match agent.retention {
                    RetentionStatus::Permanent => {
                        permanent_candidate = Some(*agent_id);
                        // Permanent agents are preferred, so we can return immediately
                        break;
                    }
                    RetentionStatus::Cached => {
                        if cached_candidate.is_none() {
                            cached_candidate = Some(*agent_id);
                        }
                    }
                    RetentionStatus::Dynamic => {
                        // Dynamic agents are not reusable
                    }
                }
            }
        }

        // Prefer Permanent over Cached
        permanent_candidate.or(cached_candidate)
    }

    /// 获取 Agent 信息
    pub fn get(&self, agent_id: &AgentId) -> Option<ManagedAgent> {
        let agents = self.agents.read().unwrap();
        agents.get(agent_id).cloned()
    }

    /// 列出所有受管 Agents
    pub fn list(&self) -> Vec<ManagedAgent> {
        let agents = self.agents.read().unwrap();
        agents.values().cloned().collect()
    }

    /// 移除 Agent
    pub fn remove(&self, agent_id: &AgentId) -> Option<ManagedAgent> {
        let mut agents = self.agents.write().unwrap();
        agents.remove(agent_id)
    }

    /// 更新 Agent 状态
    pub fn set_status(&self, agent_id: &AgentId, status: ManagedAgentStatus) {
        let mut agents = self.agents.write().unwrap();
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.status = status;
        }
    }

    /// 更新 Agent 固化状态（在评估后使用）
    pub fn set_retention(&self, agent_id: &AgentId, retention: RetentionStatus) {
        let mut agents = self.agents.write().unwrap();
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.retention = retention;
        }
    }
}

impl Default for ManagedAgentPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(
            agent_id,
            "test-agent".to_string(),
            Some("template-1".to_string()),
            Some("hand-1".to_string()),
        );

        let agent = pool.get(&agent_id).unwrap();
        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.template_id, Some("template-1".to_string()));
        assert_eq!(agent.hand_id, Some("hand-1".to_string()));
        assert_eq!(agent.status, ManagedAgentStatus::Idle);
        assert_eq!(agent.retention, RetentionStatus::Dynamic);
    }

    #[test]
    fn test_record_usage() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);

        // Record first usage
        pool.record_usage(&agent_id, true, 500, 1000);
        let agent = pool.get(&agent_id).unwrap();
        assert_eq!(agent.use_count, 1);
        assert_eq!(agent.success_count, 1);
        assert_eq!(agent.total_tokens, 500);
        assert_eq!(agent.avg_duration_ms, 1000);
        assert_eq!(agent.status, ManagedAgentStatus::Active);

        // Record second usage (failed)
        pool.record_usage(&agent_id, false, 300, 2000);
        let agent = pool.get(&agent_id).unwrap();
        assert_eq!(agent.use_count, 2);
        assert_eq!(agent.success_count, 1);
        assert_eq!(agent.total_tokens, 800);
        // avg_duration_ms = (1000 * 1 + 2000) / 2 = 1500
        assert_eq!(agent.avg_duration_ms, 1500);
    }

    #[test]
    fn test_evaluate_retention_permanent() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);

        // Simulate high usage to get Permanent status
        // Need score >= 70: use_count >= 10 (30) + success_rate >= 0.9 (40) + avg_tokens <= 1000 (20) + duration <= 5000 (10) = 100
        for i in 0..10 {
            pool.record_usage(&agent_id, true, 500, 1000);
        }

        let decision = pool.evaluate_retention(&agent_id).unwrap();
        assert_eq!(decision.decision, RetentionStatus::Permanent);
        assert!(decision.score >= 70);
        assert_eq!(decision.metrics.frequency_score, 30);
        assert_eq!(decision.metrics.success_rate_score, 40);
        assert_eq!(decision.metrics.efficiency_score, 20);
        assert_eq!(decision.metrics.speed_score, 10);
    }

    #[test]
    fn test_evaluate_retention_cached() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);

        // Simulate moderate usage to get Cached status (score >= 40, < 70)
        // use_count >= 5 (20) + success_rate >= 0.7 (30) + avg_tokens <= 5000 (15) + duration <= 15000 (7) = 72 (too high)
        // Let's try: use_count >= 2 (10) + success_rate >= 0.5 (20) + avg_tokens <= 10000 (10) + duration <= 30000 (4) = 44
        for i in 0..2 {
            pool.record_usage(&agent_id, i == 0, 5000, 20000); // 1 success out of 2 = 50%
        }

        let decision = pool.evaluate_retention(&agent_id).unwrap();
        assert_eq!(decision.decision, RetentionStatus::Cached);
        assert!(decision.score >= 40);
        assert!(decision.score < 70);
    }

    #[test]
    fn test_evaluate_retention_dynamic() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);

        // Simulate low usage to get Dynamic status (score < 40)
        // use_count = 1 (0) + success_rate = 0 (0) + avg_tokens > 10000 (5) + duration > 30000 (2) = 7
        pool.record_usage(&agent_id, false, 15000, 35000);

        let decision = pool.evaluate_retention(&agent_id).unwrap();
        assert_eq!(decision.decision, RetentionStatus::Dynamic);
        assert!(decision.score < 40);
    }

    #[test]
    fn test_evaluate_nonexistent_agent() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        let decision = pool.evaluate_retention(&agent_id);
        assert!(decision.is_none());
    }

    #[test]
    fn test_cleanup_idle_dynamic() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);

        // Set the agent to Idle
        pool.set_status(&agent_id, ManagedAgentStatus::Idle);

        // Sleep to ensure idle_duration > 0 (num_seconds returns whole seconds)
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Cleanup with max_idle_secs = 0 should mark the agent as Destroying
        // (idle_duration >= 1 > threshold of 0)
        let to_destroy = pool.cleanup_idle(0);
        assert_eq!(to_destroy.len(), 1);
        assert_eq!(to_destroy[0], agent_id);

        let agent = pool.get(&agent_id).unwrap();
        assert_eq!(agent.status, ManagedAgentStatus::Destroying);
    }

    #[test]
    fn test_cleanup_idle_cached() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);
        pool.set_retention(&agent_id, RetentionStatus::Cached);
        pool.set_status(&agent_id, ManagedAgentStatus::Idle);

        // Sleep to ensure idle_duration > 0 (num_seconds returns whole seconds)
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Cached agents have threshold = max_idle_secs * 3
        // With max_idle_secs = 0, threshold = 0, should be destroyed
        let to_destroy = pool.cleanup_idle(0);
        assert_eq!(to_destroy.len(), 1);
    }

    #[test]
    fn test_cleanup_idle_permanent() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);
        pool.set_retention(&agent_id, RetentionStatus::Permanent);
        pool.set_status(&agent_id, ManagedAgentStatus::Idle);

        // Sleep to ensure some time passes
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Permanent agents should never be cleaned up
        let to_destroy = pool.cleanup_idle(0);
        assert!(to_destroy.is_empty());
    }

    #[test]
    fn test_cleanup_idle_active_agents() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);
        // Status is Active, not Idle
        pool.set_status(&agent_id, ManagedAgentStatus::Active);

        // Sleep to ensure idle_duration > 0 (num_seconds returns whole seconds)
        std::thread::sleep(std::time::Duration::from_secs(1));

        // cleanup_idle checks retention and idle time, not status
        // So Active agents with long idle time are still cleaned up
        let to_destroy = pool.cleanup_idle(0);
        assert_eq!(to_destroy.len(), 1);
    }

    #[test]
    fn test_find_reusable_permanent_preferred() {
        let pool = ManagedAgentPool::new();
        let agent_id1 = AgentId::new();
        let agent_id2 = AgentId::new();

        pool.register(agent_id1, "test-agent".to_string(), None, None);
        pool.set_retention(&agent_id1, RetentionStatus::Cached);
        pool.set_status(&agent_id1, ManagedAgentStatus::Idle);

        pool.register(agent_id2, "test-agent".to_string(), None, None);
        pool.set_retention(&agent_id2, RetentionStatus::Permanent);
        pool.set_status(&agent_id2, ManagedAgentStatus::Idle);

        // Should prefer Permanent over Cached
        let found = pool.find_reusable("test-agent");
        assert_eq!(found, Some(agent_id2));
    }

    #[test]
    fn test_find_reusable_cached_fallback() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);
        pool.set_retention(&agent_id, RetentionStatus::Cached);
        pool.set_status(&agent_id, ManagedAgentStatus::Idle);

        let found = pool.find_reusable("test-agent");
        assert_eq!(found, Some(agent_id));
    }

    #[test]
    fn test_find_reusable_dynamic_not_reusable() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);
        // Dynamic by default
        pool.set_status(&agent_id, ManagedAgentStatus::Idle);

        let found = pool.find_reusable("test-agent");
        assert!(found.is_none());
    }

    #[test]
    fn test_find_reusable_wrong_status() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);
        pool.set_retention(&agent_id, RetentionStatus::Permanent);
        pool.set_status(&agent_id, ManagedAgentStatus::Suspended);

        // Suspended agents are not reusable
        let found = pool.find_reusable("test-agent");
        assert!(found.is_none());
    }

    #[test]
    fn test_find_reusable_wrong_name() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "other-agent".to_string(), None, None);
        pool.set_retention(&agent_id, RetentionStatus::Permanent);
        pool.set_status(&agent_id, ManagedAgentStatus::Idle);

        let found = pool.find_reusable("test-agent");
        assert!(found.is_none());
    }

    #[test]
    fn test_list() {
        let pool = ManagedAgentPool::new();
        let agent_id1 = AgentId::new();
        let agent_id2 = AgentId::new();

        pool.register(agent_id1, "agent-1".to_string(), None, None);
        pool.register(agent_id2, "agent-2".to_string(), None, None);

        let list = pool.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_remove() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);

        let removed = pool.remove(&agent_id);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "test-agent");

        let not_found = pool.get(&agent_id);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_set_status() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);
        pool.set_status(&agent_id, ManagedAgentStatus::Suspended);

        let agent = pool.get(&agent_id).unwrap();
        assert_eq!(agent.status, ManagedAgentStatus::Suspended);
    }

    #[test]
    fn test_set_retention() {
        let pool = ManagedAgentPool::new();
        let agent_id = AgentId::new();

        pool.register(agent_id, "test-agent".to_string(), None, None);
        pool.set_retention(&agent_id, RetentionStatus::Permanent);

        let agent = pool.get(&agent_id).unwrap();
        assert_eq!(agent.retention, RetentionStatus::Permanent);
    }

    #[test]
    fn test_default() {
        let pool: ManagedAgentPool = Default::default();
        assert!(pool.list().is_empty());
    }
}
