//! Intent analyzer — classifies task descriptions and suggests execution strategies.
//!
//! Uses a rule-based keyword matching approach (MVP) to determine:
//! - Task type (Coding, Research, Writing, etc.)
//! - Complexity level (Simple, Medium, Complex)
//! - Suggested agents and hands
//! - Estimated duration

use openfang_types::orchestrator::{ComplexityLevel, IntentAnalysis, TaskType};
use std::collections::HashSet;

/// Keywords associated with each task type.
const CODING_KEYWORDS: &[&str] = &[
    "code", "implement", "fix", "bug", "function", "class", "refactor",
    "compile", "build", "test", "debug", "program", "develop", "api",
    "endpoint", "database", "sql", "migration", "代码", "实现", "修复",
    "函数", "类", "重构", "编译", "构建", "测试", "调试", "开发",
];

const RESEARCH_KEYWORDS: &[&str] = &[
    "research", "analyze", "investigate", "study", "compare", "evaluate",
    "review", "survey", "explore", "find", "search", "discover",
    "研究", "分析", "调查", "比较", "评估", "探索", "查找", "发现",
];

const WRITING_KEYWORDS: &[&str] = &[
    "write", "document", "draft", "compose", "blog", "article", "report",
    "summary", "readme", "guide", "tutorial", "写作", "文档", "草稿",
    "博客", "文章", "报告", "摘要", "指南", "教程",
];

const ANALYSIS_KEYWORDS: &[&str] = &[
    "data", "statistics", "metrics", "chart", "graph", "trend", "pattern",
    "insight", "dashboard", "analytics", "数据", "统计", "指标", "图表",
    "趋势", "模式", "洞察", "仪表盘", "分析",
];

const DEVOPS_KEYWORDS: &[&str] = &[
    "deploy", "ci/cd", "docker", "kubernetes", "pipeline", "infrastructure",
    "monitor", "scale", "server", "cloud", "aws", "terraform", "部署",
    "容器", "管道", "基础设施", "监控", "扩展", "服务器", "云",
];

const COMMUNICATION_KEYWORDS: &[&str] = &[
    "email", "message", "notify", "alert", "slack", "discord", "telegram",
    "chat", "respond", "reply", "邮件", "消息", "通知", "提醒", "聊天",
    "回复",
];

/// Connective words that indicate multiple tasks.
const CONNECTIVE_WORDS: &[&str] = &[
    "and", "then", "also", "after", "before", "while", "when",
    "并且", "然后", "同时", "接着", "之后", "之前", "此外",
];

/// Keywords that indicate high complexity.
const COMPLEXITY_KEYWORDS: &[&str] = &[
    "architecture", "system design", "microservice", "distributed",
    "架构", "系统设计", "微服务", "分布式",
];

/// Analyzer for task intent classification.
#[derive(Debug, Default)]
pub struct IntentAnalyzer {
    // Reserved for future ML model integration
}

impl IntentAnalyzer {
    /// Creates a new intent analyzer instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyzes a task description and returns intent analysis results.
    pub fn analyze(&self, description: &str) -> IntentAnalysis {
        let desc_lower = description.to_lowercase();
        let desc_len = description.len();

        // Detect matched task types and collect keywords
        let (matched_types, keywords) = self.detect_task_types(&desc_lower);

        // Determine primary task type
        let task_type = self.determine_primary_type(&matched_types);

        // Calculate complexity
        let complexity = self.determine_complexity(&desc_lower, desc_len, &matched_types);

        // Calculate confidence
        let confidence = self.calculate_confidence(&matched_types, &keywords, desc_len);

        // Get suggested agents and hands
        let suggested_agents = self.get_suggested_agents(task_type, &matched_types);
        let suggested_hands = self.get_suggested_hands(task_type);

        // Estimate duration
        let estimated_duration = self.estimate_duration(task_type, complexity);

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

    /// Detects which task types match the description and extracts keywords.
    fn detect_task_types(&self, desc_lower: &str) -> (Vec<TaskType>, Vec<String>) {
        let mut matched_types = Vec::new();
        let mut keywords = Vec::new();

        if self.matches_keywords(desc_lower, CODING_KEYWORDS, &mut keywords) {
            matched_types.push(TaskType::Coding);
        }
        if self.matches_keywords(desc_lower, RESEARCH_KEYWORDS, &mut keywords) {
            matched_types.push(TaskType::Research);
        }
        if self.matches_keywords(desc_lower, WRITING_KEYWORDS, &mut keywords) {
            matched_types.push(TaskType::Writing);
        }
        if self.matches_keywords(desc_lower, ANALYSIS_KEYWORDS, &mut keywords) {
            matched_types.push(TaskType::Analysis);
        }
        if self.matches_keywords(desc_lower, DEVOPS_KEYWORDS, &mut keywords) {
            matched_types.push(TaskType::DevOps);
        }
        if self.matches_keywords(desc_lower, COMMUNICATION_KEYWORDS, &mut keywords) {
            matched_types.push(TaskType::Communication);
        }

        (matched_types, keywords)
    }

    /// Checks if description contains any keywords from the list.
    fn matches_keywords(
        &self,
        desc_lower: &str,
        keyword_list: &[&str],
        matched_keywords: &mut Vec<String>,
    ) -> bool {
        let mut found = false;
        for &keyword in keyword_list {
            if desc_lower.contains(keyword) {
                found = true;
                matched_keywords.push(keyword.to_string());
            }
        }
        found
    }

    /// Determines the primary task type from matched types.
    fn determine_primary_type(&self, matched_types: &[TaskType]) -> TaskType {
        match matched_types.len() {
            0 => TaskType::Other,
            1 => matched_types[0],
            _ => TaskType::Mixed,
        }
    }

    /// Determines complexity level based on multiple factors.
    fn determine_complexity(
        &self,
        desc_lower: &str,
        desc_len: usize,
        matched_types: &[TaskType],
    ) -> ComplexityLevel {
        // Check for complexity-indicating keywords
        for &keyword in COMPLEXITY_KEYWORDS {
            if desc_lower.contains(keyword) {
                return ComplexityLevel::Complex;
            }
        }

        // Complex: 3+ task types or very long description
        if matched_types.len() >= 3 || desc_len > 200 {
            return ComplexityLevel::Complex;
        }

        // Simple: short description, single type, no connectives
        if desc_len < 50 && matched_types.len() == 1 && !self.has_connectives(desc_lower) {
            return ComplexityLevel::Simple;
        }

        // Default to medium
        ComplexityLevel::Medium
    }

    /// Checks if description contains connective words.
    fn has_connectives(&self, desc_lower: &str) -> bool {
        for &word in CONNECTIVE_WORDS {
            if desc_lower.contains(word) {
                return true;
            }
        }
        false
    }

    /// Calculates confidence score for the classification.
    fn calculate_confidence(
        &self,
        matched_types: &[TaskType],
        keywords: &[String],
        desc_len: usize,
    ) -> f64 {
        let mut confidence: f64 = 0.5;

        // Single type match increases confidence
        if matched_types.len() == 1 {
            confidence += 0.3;
        }

        // Multiple keyword hits increase confidence
        let unique_keywords: HashSet<&str> = keywords.iter().map(|s| s.as_str()).collect();
        if unique_keywords.len() >= 3 {
            confidence += 0.1;
        }

        // Longer description provides more context
        if desc_len > 20 {
            confidence += 0.1;
        }

        // Cap at 1.0
        confidence.min(1.0)
    }

    /// Gets suggested agents based on task type.
    fn get_suggested_agents(
        &self,
        primary_type: TaskType,
        matched_types: &[TaskType],
    ) -> Vec<String> {
        // Mixed types need orchestrator/planner
        if matched_types.len() > 1 {
            return vec!["orchestrator".to_string(), "planner".to_string()];
        }

        match primary_type {
            TaskType::Coding => vec![
                "coder".to_string(),
                "debugger".to_string(),
                "test-engineer".to_string(),
            ],
            TaskType::Research => vec!["researcher".to_string(), "analyst".to_string()],
            TaskType::Writing => vec!["writer".to_string(), "doc-writer".to_string()],
            TaskType::Analysis => vec!["analyst".to_string(), "data-scientist".to_string()],
            TaskType::DevOps => vec!["ops".to_string(), "devops-lead".to_string()],
            TaskType::Communication => {
                vec!["email-assistant".to_string(), "social-media".to_string()]
            }
            TaskType::Mixed => vec!["orchestrator".to_string(), "planner".to_string()],
            TaskType::Other => vec!["assistant".to_string()],
        }
    }

    /// Gets suggested hands based on task type.
    fn get_suggested_hands(&self, primary_type: TaskType) -> Vec<String> {
        match primary_type {
            TaskType::Coding => vec!["browser".to_string()],
            TaskType::Research => vec!["researcher".to_string(), "browser".to_string()],
            TaskType::DevOps => vec!["collector".to_string()],
            _ => vec![],
        }
    }

    /// Estimates duration in seconds based on task type and complexity.
    fn estimate_duration(&self, task_type: TaskType, complexity: ComplexityLevel) -> u64 {
        let base_duration = match task_type {
            TaskType::Coding => 60,
            TaskType::Research => 120,
            TaskType::Writing => 90,
            TaskType::Analysis => 60,
            TaskType::DevOps => 60,
            TaskType::Communication => 60,
            TaskType::Mixed => 90,
            TaskType::Other => 60,
        };

        match complexity {
            ComplexityLevel::Simple => base_duration,
            ComplexityLevel::Medium => base_duration * 3,
            ComplexityLevel::Complex => base_duration * 8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_coding_task() {
        let analyzer = IntentAnalyzer::new();
        let result = analyzer.analyze("fix bug in login function");

        assert_eq!(result.task_type, TaskType::Coding);
        assert_eq!(result.complexity, ComplexityLevel::Simple);
        assert!(result.confidence > 0.5);
        assert!(result.suggested_agents.contains(&"coder".to_string()));
        assert!(result.suggested_hands.contains(&"browser".to_string()));
    }

    #[test]
    fn test_medium_research_task() {
        let analyzer = IntentAnalyzer::new();
        // Use description with only research keywords (avoid "implementing" which matches coding)
        let result = analyzer.analyze(
            "research the best practices and study industry standards \
             for enterprise software solutions in our organization",
        );

        assert_eq!(result.task_type, TaskType::Research);
        assert_eq!(result.complexity, ComplexityLevel::Medium);
        assert!(result.confidence > 0.5);
        assert!(result.suggested_agents.contains(&"researcher".to_string()));
    }

    #[test]
    fn test_complex_multi_type_task() {
        let analyzer = IntentAnalyzer::new();
        let result = analyzer.analyze(
            "Design the system architecture for a distributed microservice platform, \
             implement the core services, write documentation, and set up the CI/CD pipeline",
        );

        assert_eq!(result.task_type, TaskType::Mixed);
        assert_eq!(result.complexity, ComplexityLevel::Complex);
        assert!(result.suggested_agents.contains(&"orchestrator".to_string()));
    }

    #[test]
    fn test_complexity_simple_short_single_type() {
        let analyzer = IntentAnalyzer::new();
        let result = analyzer.analyze("write a blog post");

        assert_eq!(result.complexity, ComplexityLevel::Simple);
    }

    #[test]
    fn test_complexity_medium_with_connective() {
        let analyzer = IntentAnalyzer::new();
        let result = analyzer.analyze("write a blog post and publish it to the website");

        assert_eq!(result.complexity, ComplexityLevel::Medium);
    }

    #[test]
    fn test_complexity_complex_keywords() {
        let analyzer = IntentAnalyzer::new();
        let result = analyzer.analyze("design the system architecture for our platform");

        assert_eq!(result.complexity, ComplexityLevel::Complex);
    }

    #[test]
    fn test_complexity_complex_long_description() {
        let analyzer = IntentAnalyzer::new();
        let long_desc = "a".repeat(250);
        let result = analyzer.analyze(&long_desc);

        assert_eq!(result.complexity, ComplexityLevel::Complex);
    }

    #[test]
    fn test_devops_task() {
        let analyzer = IntentAnalyzer::new();
        let result = analyzer.analyze("deploy to kubernetes cluster");

        assert_eq!(result.task_type, TaskType::DevOps);
        assert!(result.suggested_agents.contains(&"ops".to_string()));
        assert!(result.suggested_hands.contains(&"collector".to_string()));
    }

    #[test]
    fn test_communication_task() {
        let analyzer = IntentAnalyzer::new();
        let result = analyzer.analyze("send email notification to the team");

        assert_eq!(result.task_type, TaskType::Communication);
        assert!(result.suggested_agents.contains(&"email-assistant".to_string()));
    }

    #[test]
    fn test_writing_task() {
        let analyzer = IntentAnalyzer::new();
        // Use description that only matches writing keywords (not "api" which is coding)
        let result = analyzer.analyze("draft a blog article");

        assert_eq!(result.task_type, TaskType::Writing);
        assert!(result.suggested_agents.contains(&"writer".to_string()));
    }

    #[test]
    fn test_analysis_task() {
        let analyzer = IntentAnalyzer::new();
        // Use description that only matches analysis keywords (not "analyze" which is research)
        let result = analyzer.analyze("view dashboard metrics");

        assert_eq!(result.task_type, TaskType::Analysis);
        assert!(result.suggested_agents.contains(&"analyst".to_string()));
    }

    #[test]
    fn test_unknown_task_type() {
        let analyzer = IntentAnalyzer::new();
        let result = analyzer.analyze("hello world");

        assert_eq!(result.task_type, TaskType::Other);
        assert!(result.suggested_agents.contains(&"assistant".to_string()));
    }

    #[test]
    fn test_confidence_calculation() {
        let analyzer = IntentAnalyzer::new();

        // Single type, short description - lower confidence
        let result1 = analyzer.analyze("code");
        assert!(result1.confidence >= 0.5);

        // Single type, longer description - higher confidence
        let result2 = analyzer.analyze("implement a new function to handle user authentication");
        assert!(result2.confidence > result1.confidence);
    }

    #[test]
    fn test_duration_estimation() {
        let analyzer = IntentAnalyzer::new();

        // Simple coding task: 60s
        let result1 = analyzer.analyze("fix bug");
        assert_eq!(result1.estimated_duration, 60);

        // Medium coding task: 180s (60 * 3)
        let result2 = analyzer.analyze("fix bug and add new feature for user management");
        assert_eq!(result2.estimated_duration, 180);

        // Complex task: 480s (60 * 8)
        let result3 = analyzer.analyze(
            "architecture design for distributed microservice system with multiple components",
        );
        assert_eq!(result3.estimated_duration, 480);
    }

    #[test]
    fn test_chinese_keywords() {
        let analyzer = IntentAnalyzer::new();
        let result = analyzer.analyze("修复登录功能的bug");

        assert_eq!(result.task_type, TaskType::Coding);
    }

    #[test]
    fn test_mixed_task_detection() {
        let analyzer = IntentAnalyzer::new();
        let result = analyzer.analyze("write code and document the API");

        assert_eq!(result.task_type, TaskType::Mixed);
        assert!(result.suggested_agents.contains(&"orchestrator".to_string()));
    }
}
