# TGP State Machine Implementation

**Destination Path:** `crates/tbc-core/src/tgp/state.rs`

**Implementation:** M1 - TGP Message Parsing & Basic Routing

-----

```rust
//! TGP session state machine per TGP-00 §4
//!
//! This module implements the state machine for TGP sessions, ensuring that
//! all state transitions follow the protocol specification and maintaining
//! session metadata for tracking and recovery.
//!
//! # State Machine Overview (TGP-00 §4)
//!
//! ```text
//!     ┌──────┐
//!     │ Idle │
//!     └───┬──┘
//!         │ QUERY sent
//!         ▼
//!  ┌──────────────┐
//!  │ QuerySent    │
//!  └──┬───────────┘
//!     │ OFFER received
//!     ▼
//!  ┌──────────────────┐
//!  │ OfferReceived    │
//!  └──┬───────────────┘
//!     │ ACCEPT sent
//!     ▼
//!  ┌──────────────┐
//!  │ AcceptSent   │
//!  └──┬───────────┘
//!     │ Settlement initiated
//!     ▼
//!  ┌──────────────┐
//!  │ Finalizing   │
//!  └──┬───────────┘
//!     │ SETTLE received
//!     ▼
//!  ┌──────────┐
//!  │ Settled  │
//!  └──────────┘
//!
//!    Any state ──ERROR──> Errored
//! ```
//!
//! # Examples
//!
//! ```rust
//! use tbc_core::tgp::state::{TGPSession, TGPState};
//!
//! // Create a new session
//! let mut session = TGPSession::new("sess-abc123");
//! assert_eq!(session.state, TGPState::Idle);
//!
//! // Progress through valid transitions
//! session.transition(TGPState::QuerySent).unwrap();
//! session.transition(TGPState::OfferReceived).unwrap();
//! session.transition(TGPState::AcceptSent).unwrap();
//! session.transition(TGPState::Finalizing).unwrap();
//! session.transition(TGPState::Settled).unwrap();
//! ```

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during state machine operations
///
/// # Specification Reference
/// - TGP-00 §4 State Machine
#[derive(Debug, Error, Clone, PartialEq)]
pub enum TGPStateError {
    /// Invalid state transition attempted
    ///
    /// Occurs when trying to transition from one state to another
    /// when that transition is not allowed by the protocol.
    ///
    /// **Example:** Trying to go from `Idle` directly to `Settled`
    #[error("Invalid state transition: {0:?} → {1:?}")]
    InvalidTransition(TGPState, TGPState),

    /// Session has timed out
    ///
    /// Occurs when a session exceeds its configured timeout deadline
    /// and can no longer proceed.
    #[error("Session timed out at {0}")]
    SessionTimeout(u64),

    /// Session is in a terminal state and cannot transition
    ///
    /// Occurs when trying to transition from a terminal state
    /// (Settled or Errored) to any other state.
    #[error("Session is in terminal state {0:?} and cannot transition")]
    TerminalState(TGPState),

    /// Session already in the target state
    ///
    /// Not necessarily an error, but indicates a redundant transition attempt.
    #[error("Session already in state {0:?}")]
    AlreadyInState(TGPState),
}

// ============================================================================
// TGPState Enum (§4)
// ============================================================================

/// TGP session state
///
/// Represents the current phase of a TGP session as it progresses through
/// the protocol lifecycle.
///
/// # Specification Reference
/// - TGP-00 §4 State Machine
///
/// # State Descriptions
///
/// - **Idle**: No active session, ready to initiate
/// - **QuerySent**: QUERY message sent, waiting for OFFER
/// - **OfferReceived**: OFFER received, Buyer decides whether to accept
/// - **AcceptSent**: Buyer accepted OFFER, settlement in progress
/// - **Finalizing**: Layer-8 transaction submitted, waiting for confirmation
/// - **Settled**: Settlement confirmed, TDR emitted (terminal state)
/// - **Errored**: Terminal error state, may retry from Idle (terminal state)
///
/// # Examples
///
/// ```rust
/// use tbc_core::tgp::state::TGPState;
///
/// let state = TGPState::Idle;
/// assert!(!state.is_terminal());
///
/// let state = TGPState::Settled;
/// assert!(state.is_terminal());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TGPState {
    /// No active session, ready to initiate
    ///
    /// **Entry:** Session created
    /// **Exit:** QUERY message sent
    Idle,

    /// QUERY message sent, waiting for OFFER
    ///
    /// **Entry:** QUERY sent to Controller
    /// **Exit:** OFFER received OR timeout/error
    ///
    /// **Timeout:** Typically 30 seconds (per TGP-00 §4)
    QuerySent,

    /// OFFER received, Buyer decides whether to accept
    ///
    /// **Entry:** OFFER received from Controller
    /// **Exit:** Buyer accepts OR rejects/timeout
    ///
    /// **Timeout:** Typically 5 minutes (per TGP-00 §4)
    OfferReceived,

    /// Buyer accepted OFFER, settlement in progress
    ///
    /// **Entry:** Buyer accepts and initiates settlement
    /// **Exit:** Layer-8 transaction submitted OR error
    ///
    /// **Note:** This state may be brief if settlement is immediate
    AcceptSent,

    /// Layer-8 transaction submitted, waiting for confirmation
    ///
    /// **Entry:** Settlement transaction submitted to blockchain
    /// **Exit:** SETTLE confirmation received OR timeout
    ///
    /// **Timeout:** Typically 10 minutes (per TGP-00 §4)
    Finalizing,

    /// Settlement confirmed, TDR emitted
    ///
    /// **Terminal State:** Cannot transition to any other state
    ///
    /// **Entry:** SETTLE confirmation received
    /// **Exit:** None (terminal)
    Settled,

    /// Terminal error state
    ///
    /// **Terminal State:** Cannot transition to any other state
    ///
    /// **Entry:** ERROR message OR critical failure from any state
    /// **Exit:** None (terminal, but new session may be initiated)
    Errored,
}

impl TGPState {
    /// Check if this is a terminal state
    ///
    /// Terminal states cannot transition to any other state.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::state::TGPState;
    /// assert!(TGPState::Settled.is_terminal());
    /// assert!(TGPState::Errored.is_terminal());
    /// assert!(!TGPState::Idle.is_terminal());
    /// ```
    pub fn is_terminal(&self) -> bool {
        matches!(self, TGPState::Settled | TGPState::Errored)
    }

    /// Check if this state can transition to the target state
    ///
    /// Validates whether a transition is allowed according to TGP-00 §4.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::state::TGPState;
    /// assert!(TGPState::Idle.can_transition_to(TGPState::QuerySent));
    /// assert!(!TGPState::Idle.can_transition_to(TGPState::Settled));
    /// ```
    pub fn can_transition_to(&self, target: TGPState) -> bool {
        use TGPState::*;

        // Terminal states cannot transition
        if self.is_terminal() {
            return false;
        }

        // Check valid transitions per TGP-00 §4
        match (self, target) {
            // From Idle
            (Idle, QuerySent) => true,

            // From QuerySent
            (QuerySent, OfferReceived) => true,
            (QuerySent, Errored) => true,

            // From OfferReceived
            (OfferReceived, AcceptSent) => true,
            (OfferReceived, Errored) => true,

            // From AcceptSent
            (AcceptSent, Finalizing) => true,
            (AcceptSent, Errored) => true,

            // From Finalizing
            (Finalizing, Settled) => true,
            (Finalizing, Errored) => true,

            // Any state can transition to Errored (already handled by terminal check)
            (_, Errored) => true,

            // All other transitions are invalid
            _ => false,
        }
    }

    /// Get the typical timeout for this state (in seconds)
    ///
    /// Returns the recommended timeout duration per TGP-00 §4.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::state::TGPState;
    /// assert_eq!(TGPState::QuerySent.timeout_seconds(), Some(30));
    /// assert_eq!(TGPState::Finalizing.timeout_seconds(), Some(600));
    /// assert_eq!(TGPState::Settled.timeout_seconds(), None);
    /// ```
    pub fn timeout_seconds(&self) -> Option<u64> {
        match self {
            TGPState::QuerySent => Some(30),      // 30 seconds
            TGPState::OfferReceived => Some(300), // 5 minutes
            TGPState::Finalizing => Some(600),    // 10 minutes
            TGPState::Idle => None,               // No timeout in idle
            TGPState::AcceptSent => Some(60),     // 1 minute (implementation-specific)
            TGPState::Settled => None,            // Terminal state
            TGPState::Errored => None,            // Terminal state
        }
    }

    /// Get a human-readable description of this state
    pub fn description(&self) -> &'static str {
        match self {
            TGPState::Idle => "Ready to initiate session",
            TGPState::QuerySent => "Waiting for OFFER response",
            TGPState::OfferReceived => "Reviewing OFFER, awaiting acceptance",
            TGPState::AcceptSent => "Initiating settlement",
            TGPState::Finalizing => "Waiting for settlement confirmation",
            TGPState::Settled => "Settlement completed successfully",
            TGPState::Errored => "Session terminated with error",
        }
    }
}

impl Default for TGPState {
    fn default() -> Self {
        TGPState::Idle
    }
}

// ============================================================================
// TGPSession Struct
// ============================================================================

/// TGP session with state management
///
/// Maintains the complete state of a TGP session including current state,
/// correlation IDs, timestamps, and timeout tracking.
///
/// # Specification Reference
/// - TGP-00 §4 State Machine
/// - TGP-00 §13 State Summary Objects (SSO)
///
/// # Fields
///
/// | Field | Description |
/// |-------|-------------|
/// | `session_id` | Unique session identifier |
/// | `state` | Current state in the state machine |
/// | `query_id` | ID of originating QUERY message |
/// | `offer_id` | ID of accepted OFFER message |
/// | `created_at` | Unix timestamp of session creation |
/// | `updated_at` | Unix timestamp of last state change |
/// | `timeout_at` | Unix timestamp when session expires |
///
/// # Examples
///
/// ```rust
/// use tbc_core::tgp::state::{TGPSession, TGPState};
///
/// let mut session = TGPSession::new("sess-abc123");
///
/// // Set query ID when QUERY is sent
/// session.query_id = Some("q-abc123".to_string());
/// session.transition(TGPState::QuerySent).unwrap();
///
/// // Set offer ID when OFFER is received
/// session.offer_id = Some("offer-abc123".to_string());
/// session.transition(TGPState::OfferReceived).unwrap();
///
/// // Check if timed out
/// assert!(!session.is_timed_out());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TGPSession {
    /// Unique session identifier
    ///
    /// **Spec:** Used for correlation across TGP messages and CoreProver transactions
    ///
    /// **Format:** Typically prefixed with "sess-"
    pub session_id: String,

    /// Current state in the state machine
    ///
    /// **Spec:** TGP-00 §4 - Tracks session progression
    pub state: TGPState,

    /// ID of the originating QUERY message
    ///
    /// **Spec:** Set when QUERY is sent, used for correlation
    ///
    /// **Present:** From QuerySent onwards
    pub query_id: Option<String>,

    /// ID of the accepted OFFER message
    ///
    /// **Spec:** Set when OFFER is received, used for correlation
    ///
    /// **Present:** From OfferReceived onwards
    pub offer_id: Option<String>,

    /// Unix timestamp of session creation (seconds since epoch)
    ///
    /// **Spec:** Used for audit trail and timeout calculation
    pub created_at: u64,

    /// Unix timestamp of last state change (seconds since epoch)
    ///
    /// **Spec:** Updated on every state transition
    pub updated_at: u64,

    /// Unix timestamp when session expires (seconds since epoch)
    ///
    /// **Spec:** TGP-00 §4 - Calculated based on state timeout policies
    ///
    /// **None:** For states without timeouts (Idle, terminal states)
    pub timeout_at: Option<u64>,
}

impl TGPSession {
    /// Create a new session in Idle state
    ///
    /// # Arguments
    ///
    /// * `session_id` - Unique identifier for this session
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::state::TGPSession;
    /// let session = TGPSession::new("sess-abc123");
    /// assert_eq!(session.session_id, "sess-abc123");
    /// ```
    pub fn new(session_id: impl Into<String>) -> Self {
        let now = current_timestamp();
        Self {
            session_id: session_id.into(),
            state: TGPState::Idle,
            query_id: None,
            offer_id: None,
            created_at: now,
            updated_at: now,
            timeout_at: None,
        }
    }

    /// Transition to a new state with validation
    ///
    /// This method validates the transition, updates timestamps, sets timeouts,
    /// and emits a log event.
    ///
    /// # Arguments
    ///
    /// * `new_state` - The target state to transition to
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The transition is not allowed by the protocol
    /// - The session is in a terminal state
    /// - The session has timed out
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::state::{TGPSession, TGPState};
    /// let mut session = TGPSession::new("sess-123");
    ///
    /// // Valid transition
    /// assert!(session.transition(TGPState::QuerySent).is_ok());
    ///
    /// // Invalid transition
    /// assert!(session.transition(TGPState::Settled).is_err());
    /// ```
    pub fn transition(&mut self, new_state: TGPState) -> Result<(), TGPStateError> {
        // Check if session has timed out
        if self.is_timed_out() {
            let timeout = self.timeout_at.unwrap_or(0);
            return Err(TGPStateError::SessionTimeout(timeout));
        }

        // Check if already in target state
        if self.state == new_state {
            return Err(TGPStateError::AlreadyInState(new_state));
        }

        // Check if current state is terminal
        if self.state.is_terminal() {
            return Err(TGPStateError::TerminalState(self.state));
        }

        // Validate transition
        if !self.state.can_transition_to(new_state) {
            return Err(TGPStateError::InvalidTransition(self.state, new_state));
        }

        // Perform transition
        let old_state = self.state;
        self.state = new_state;
        self.updated_at = current_timestamp();

        // Set timeout for new state
        if let Some(timeout_seconds) = new_state.timeout_seconds() {
            self.timeout_at = Some(self.updated_at + timeout_seconds);
        } else {
            self.timeout_at = None;
        }

        // Emit state change event
        log::info!(
            "TGP session {} transitioned: {:?} → {:?}",
            self.session_id,
            old_state,
            new_state
        );

        Ok(())
    }

    /// Check if the session has timed out
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::state::TGPSession;
    /// let session = TGPSession::new("sess-123");
    /// assert!(!session.is_timed_out());
    /// ```
    pub fn is_timed_out(&self) -> bool {
        if let Some(timeout) = self.timeout_at {
            current_timestamp() > timeout
        } else {
            false
        }
    }

    /// Set a custom timeout deadline
    ///
    /// # Arguments
    ///
    /// * `seconds` - Seconds from now until timeout
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::state::TGPSession;
    /// let mut session = TGPSession::new("sess-123");
    /// session.set_timeout(300); // 5 minutes
    /// assert!(session.timeout_at.is_some());
    /// ```
    pub fn set_timeout(&mut self, seconds: u64) {
        self.timeout_at = Some(current_timestamp() + seconds);
    }

    /// Clear the timeout deadline
    ///
    /// Useful for manual control of timeout behavior.
    pub fn clear_timeout(&mut self) {
        self.timeout_at = None;
    }

    /// Get the remaining time until timeout (in seconds)
    ///
    /// Returns `None` if no timeout is set or if already timed out.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::state::TGPSession;
    /// let mut session = TGPSession::new("sess-123");
    /// session.set_timeout(300);
    /// if let Some(remaining) = session.remaining_timeout() {
    ///     assert!(remaining <= 300);
    /// }
    /// ```
    pub fn remaining_timeout(&self) -> Option<u64> {
        if let Some(timeout) = self.timeout_at {
            let now = current_timestamp();
            if now < timeout {
                Some(timeout - now)
            } else {
                None // Already timed out
            }
        } else {
            None // No timeout set
        }
    }

    /// Get the session age (in seconds)
    ///
    /// Returns the number of seconds since the session was created.
    pub fn age(&self) -> u64 {
        current_timestamp() - self.created_at
    }

    /// Check if session is in a terminal state
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    /// Force transition to Errored state
    ///
    /// This bypasses normal transition validation and always succeeds.
    /// Useful for handling critical errors or external failures.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tbc_core::tgp::state::{TGPSession, TGPState};
    /// let mut session = TGPSession::new("sess-123");
    /// session.transition(TGPState::QuerySent).unwrap();
    /// session.force_error();
    /// assert_eq!(session.state, TGPState::Errored);
    /// ```
    pub fn force_error(&mut self) {
        let old_state = self.state;
        self.state = TGPState::Errored;
        self.updated_at = current_timestamp();
        self.timeout_at = None;

        log::warn!(
            "TGP session {} force-transitioned to Errored from {:?}",
            self.session_id,
            old_state
        );
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get current Unix timestamp in seconds
///
/// Returns seconds since Unix epoch (January 1, 1970).
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time before Unix epoch")
        .as_secs()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions_valid_path() {
        let mut session = TGPSession::new("sess-test");

        // Valid happy path
        assert!(session.transition(TGPState::QuerySent).is_ok());
        assert_eq!(session.state, TGPState::QuerySent);

        assert!(session.transition(TGPState::OfferReceived).is_ok());
        assert_eq!(session.state, TGPState::OfferReceived);

        assert!(session.transition(TGPState::AcceptSent).is_ok());
        assert_eq!(session.state, TGPState::AcceptSent);

        assert!(session.transition(TGPState::Finalizing).is_ok());
        assert_eq!(session.state, TGPState::Finalizing);

        assert!(session.transition(TGPState::Settled).is_ok());
        assert_eq!(session.state, TGPState::Settled);
    }

    #[test]
    fn test_state_transitions_error_path() {
        let mut session = TGPSession::new("sess-test");

        // Transition to QuerySent
        session.transition(TGPState::QuerySent).unwrap();

        // Valid error transition
        assert!(session.transition(TGPState::Errored).is_ok());
        assert_eq!(session.state, TGPState::Errored);
    }

    #[test]
    fn test_state_transitions_early_error() {
        let mut session = TGPSession::new("sess-test");

        // Any state can transition to Errored
        assert!(session.transition(TGPState::Errored).is_ok());
        assert_eq!(session.state, TGPState::Errored);
    }

    #[test]
    fn test_invalid_transition_skip_state() {
        let mut session = TGPSession::new("sess-test");

        // Cannot skip from Idle directly to Settled
        let result = session.transition(TGPState::Settled);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TGPStateError::InvalidTransition(TGPState::Idle, TGPState::Settled)
        ));
    }

    #[test]
    fn test_invalid_transition_from_terminal() {
        let mut session = TGPSession::new("sess-test");

        // Transition to terminal state
        session.transition(TGPState::QuerySent).unwrap();
        session.transition(TGPState::OfferReceived).unwrap();
        session.transition(TGPState::AcceptSent).unwrap();
        session.transition(TGPState::Finalizing).unwrap();
        session.transition(TGPState::Settled).unwrap();

        // Cannot transition from terminal state
        let result = session.transition(TGPState::Idle);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TGPStateError::TerminalState(TGPState::Settled)
        ));
    }

    #[test]
    fn test_already_in_state() {
        let mut session = TGPSession::new("sess-test");

        // Try to transition to same state
        let result = session.transition(TGPState::Idle);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TGPStateError::AlreadyInState(TGPState::Idle)
        ));
    }

    #[test]
    fn test_timeout_setting() {
        let mut session = TGPSession::new("sess-test");

        // Idle has no timeout
        assert!(session.timeout_at.is_none());

        // QuerySent should set timeout
        session.transition(TGPState::QuerySent).unwrap();
        assert!(session.timeout_at.is_some());
        assert_eq!(session.remaining_timeout().is_some(), true);
    }

    #[test]
    fn test_timeout_detection() {
        let mut session = TGPSession::new("sess-test");
        session.transition(TGPState::QuerySent).unwrap();

        // Manually set timeout to past
        session.timeout_at = Some(1); // Unix epoch + 1 second (definitely past)

        // Should be timed out
        assert!(session.is_timed_out());

        // Cannot transition when timed out
        let result = session.transition(TGPState::OfferReceived);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TGPStateError::SessionTimeout(_)
        ));
    }

    #[test]
    fn test_force_error() {
        let mut session = TGPSession::new("sess-test");
        session.transition(TGPState::QuerySent).unwrap();
        session.transition(TGPState::OfferReceived).unwrap();

        // Force error from any state
        session.force_error();
        assert_eq!(session.state, TGPState::Errored);
        assert!(session.timeout_at.is_none());
    }

    #[test]
    fn test_session_age() {
        let session = TGPSession::new("sess-test");
        let age = session.age();
        assert_eq!(age, 0); // Should be 0 or very small
    }

    #[test]
    fn test_state_timeout_values() {
        assert_eq!(TGPState::QuerySent.timeout_seconds(), Some(30));
        assert_eq!(TGPState::OfferReceived.timeout_seconds(), Some(300));
        assert_eq!(TGPState::Finalizing.timeout_seconds(), Some(600));
        assert_eq!(TGPState::Idle.timeout_seconds(), None);
        assert_eq!(TGPState::Settled.timeout_seconds(), None);
    }

    #[test]
    fn test_state_descriptions() {
        assert_eq!(
            TGPState::Idle.description(),
            "Ready to initiate session"
        );
        assert_eq!(
            TGPState::Settled.description(),
            "Settlement completed successfully"
        );
    }

    #[test]
    fn test_is_terminal() {
        assert!(!TGPState::Idle.is_terminal());
        assert!(!TGPState::QuerySent.is_terminal());
        assert!(TGPState::Settled.is_terminal());
        assert!(TGPState::Errored.is_terminal());
    }

    #[test]
    fn test_can_transition_to() {
        // Valid transitions
        assert!(TGPState::Idle.can_transition_to(TGPState::QuerySent));
        assert!(TGPState::QuerySent.can_transition_to(TGPState::OfferReceived));
        assert!(TGPState::QuerySent.can_transition_to(TGPState::Errored));

        // Invalid transitions
        assert!(!TGPState::Idle.can_transition_to(TGPState::Settled));
        assert!(!TGPState::QuerySent.can_transition_to(TGPState::AcceptSent));

        // Terminal states cannot transition
        assert!(!TGPState::Settled.can_transition_to(TGPState::Idle));
        assert!(!TGPState::Errored.can_transition_to(TGPState::QuerySent));
    }

    #[test]
    fn test_session_metadata() {
        let mut session = TGPSession::new("sess-abc123");

        // Set correlation IDs
        session.query_id = Some("q-123".to_string());
        session.offer_id = Some("offer-123".to_string());

        assert_eq!(session.query_id.as_ref().unwrap(), "q-123");
        assert_eq!(session.offer_id.as_ref().unwrap(), "offer-123");
    }

    #[test]
    fn test_custom_timeout() {
        let mut session = TGPSession::new("sess-test");
        session.set_timeout(300); // 5 minutes

        assert!(session.timeout_at.is_some());
        let remaining = session.remaining_timeout().unwrap();
        assert!(remaining <= 300 && remaining > 290); // Should be close to 300

        session.clear_timeout();
        assert!(session.timeout_at.is_none());
    }

    #[test]
    fn test_timestamps_updated() {
        let mut session = TGPSession::new("sess-test");
        let created = session.created_at;
        let updated = session.updated_at;

        // Initial timestamps should match
        assert_eq!(created, updated);

        // Wait a tiny bit (not reliable but best we can do in unit test)
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Transition should update timestamp
        session.transition(TGPState::QuerySent).unwrap();
        assert!(session.updated_at >= updated); // Should be equal or greater
        assert_eq!(session.created_at, created); // Created should not change
    }
}
```