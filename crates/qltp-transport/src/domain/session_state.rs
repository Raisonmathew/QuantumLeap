//! Session State - Value Object
//!
//! Represents the lifecycle state of a transport session

use serde::{Deserialize, Serialize};

/// Session lifecycle states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionState {
    /// Session is being initialized
    Initializing,
    /// Session is active and ready for transfers
    Active,
    /// Session is temporarily paused
    Paused,
    /// Session completed successfully
    Completed,
    /// Session failed with an error
    Failed,
}

impl SessionState {
    /// Check if transition to another state is valid
    pub fn can_transition_to(&self, target: SessionState) -> bool {
        use SessionState::*;
        
        match (self, target) {
            // From Initializing
            (Initializing, Active) => true,
            (Initializing, Failed) => true,
            
            // From Active
            (Active, Paused) => true,
            (Active, Completed) => true,
            (Active, Failed) => true,
            
            // From Paused
            (Paused, Active) => true,
            (Paused, Failed) => true,
            (Paused, Completed) => true,
            
            // Terminal states cannot transition
            (Completed, _) => false,
            (Failed, _) => false,
            
            // Same state is always valid
            (a, b) if *a == b => true,
            
            // All other transitions are invalid
            _ => false,
        }
    }

    /// Check if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }

    /// Check if session can send/receive data
    pub fn can_transfer(&self) -> bool {
        matches!(self, Self::Active)
    }
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initializing => write!(f, "Initializing"),
            Self::Active => write!(f, "Active"),
            Self::Paused => write!(f, "Paused"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_state_transitions() {
        assert!(SessionState::Initializing.can_transition_to(SessionState::Active));
        assert!(SessionState::Active.can_transition_to(SessionState::Paused));
        assert!(SessionState::Paused.can_transition_to(SessionState::Active));
        assert!(SessionState::Active.can_transition_to(SessionState::Completed));
    }

    #[test]
    fn test_invalid_state_transitions() {
        assert!(!SessionState::Completed.can_transition_to(SessionState::Active));
        assert!(!SessionState::Failed.can_transition_to(SessionState::Active));
        assert!(!SessionState::Initializing.can_transition_to(SessionState::Completed));
    }

    #[test]
    fn test_terminal_states() {
        assert!(SessionState::Completed.is_terminal());
        assert!(SessionState::Failed.is_terminal());
        assert!(!SessionState::Active.is_terminal());
        assert!(!SessionState::Initializing.is_terminal());
    }

    #[test]
    fn test_can_transfer() {
        assert!(SessionState::Active.can_transfer());
        assert!(!SessionState::Initializing.can_transfer());
        assert!(!SessionState::Paused.can_transfer());
        assert!(!SessionState::Completed.can_transfer());
    }
}

// Made with Bob
