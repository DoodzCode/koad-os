//! Citadel Agent Support System (CASS)
//!
//! CASS provides cognitive continuity and support services for KoadOS agents,
//! including memory management, context hydration, and inter-agent signaling.

pub mod services;
pub mod storage;
pub mod token_budget;
#[cfg(test)]
pub mod mock_storage {
    pub use crate::storage::mock::MockStorage;
}
