//! Orchestrator module — task routing and execution strategy management.

pub mod engine;
pub mod intent;
pub mod lifecycle;
pub mod router;

pub use engine::{KernelOperations, OrchestratorEngine};
pub use intent::IntentAnalyzer;
pub use lifecycle::ManagedAgentPool;
pub use router::TaskRouter;
