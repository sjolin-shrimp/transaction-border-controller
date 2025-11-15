//! ============================================================================
//! CoreProver Test Harness - Event Trace System (Chunk 3 of 9)
//!
//! This module defines the canonical test-trace model. Every engine action
//! performed by EngineDriver is recorded here. These logs power:
//!    - scenario debugging
//!    - invariant checking
//!    - deterministic replay
//!    - fault-injection simulations
//!
//! The trace is *append-only* and strictly ordered.
//!
//! ============================================================================

use serde::{Deserialize, Serialize};

/// Result of a single engine transition attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransitionResult {
    Ok,
    Err(String),
}

/// All possible transition types recorded by the harness.
///
/// These events intentionally mirror EngineDriver's public API.
/// They represent *requested* transitions — not the actual engine state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HarnessEventKind {
    // Time
    AdvanceTime {
        seconds: u64,
        before: u64,
        after: u64,
    },

    // Buyer
    BuyerCommit {
        at_mono: u64,
        buyer: String,
        seller: String,
        amount: u64,
        txid: String,
    },

    // Seller acceptance
    SellerAccept {
        at_mono: u64,
        order_id: [u8; 32],
        txid: String,
    },

    // Seller fulfillment
    SellerFulfill {
        at_mono: u64,
        order_id: [u8; 32],
        txid: String,
    },

    // Final settlement
    SellerClaim {
        at_mono: u64,
        order_id: [u8; 32],
        txid: String,
        amount: u64,
    },

    SellerRefund {
        at_mono: u64,
        order_id: [u8; 32],
        txid: String,
        amount: u64,
    },

    // Buyer exits
    BuyerWithdraw {
        at_mono: u64,
        order_id: [u8; 32],
        txid: Option<String>,
        amount: u64,
    },

    // Timed release
    TimedRelease {
        at_mono: u64,
        order_id: [u8; 32],
        amount: u64,
    },

    // Passive state update
    UpdateState {
        at_mono: u64,
        order_id: [u8; 32],
    },
}

/// A single test-harness event (transition attempt + result).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarnessEvent {
    pub kind: HarnessEventKind,
    pub result: TransitionResult,
}

impl HarnessEvent {
    pub fn new(kind: HarnessEventKind, result: TransitionResult) -> Self {
        Self { kind, result }
    }

    /// Pretty-print in human-readable form
    pub fn pretty(&self) -> String {
        match &self.kind {
            HarnessEventKind::AdvanceTime { seconds, before, after } => {
                format!("⏱ advance_time {}s ({} → {}) [{:?}]", seconds, before, after, self.result)
            }

            HarnessEventKind::BuyerCommit { at_mono, buyer, seller, amount, txid } => {
                format!(
                    "👤 buyer_commit @{} buyer={} seller={} amount={} txid={} [{:?}]",
                    at_mono, buyer, seller, amount, txid, self.result
                )
            }

            HarnessEventKind::SellerAccept { at_mono, order_id, txid } => {
                format!(
                    "🏁 seller_accept @{} order_id={:?} txid={} [{:?}]",
                    at_mono, order_id, txid, self.result
                )
            }

            HarnessEventKind::SellerFulfill { at_mono, order_id, txid } => {
                format!(
                    "📦 seller_fulfill @{} order_id={:?} txid={} [{:?}]",
                    at_mono, order_id, txid, self.result
                )
            }

            HarnessEventKind::SellerClaim { at_mono, order_id, txid, amount } => {
                format!(
                    "💰 seller_claim @{} order_id={:?} txid={} amount={} [{:?}]",
                    at_mono, order_id, txid, amount, self.result
                )
            }

            HarnessEventKind::SellerRefund { at_mono, order_id, txid, amount } => {
                format!(
                    "↩️ seller_refund @{} order_id={:?} txid={} amount={} [{:?}]",
                    at_mono, order_id, txid, amount, self.result
                )
            }

            HarnessEventKind::BuyerWithdraw { at_mono, order_id, txid, amount } => {
                format!(
                    "🚪 buyer_withdraw @{} order_id={:?} txid={:?} amount={} [{:?}]",
                    at_mono, order_id, txid, amount, self.result
                )
            }

            HarnessEventKind::TimedRelease { at_mono, order_id, amount } => {
                format!(
                    "⏲️ timed_release @{} order_id={:?} amount={} [{:?}]",
                    at_mono, order_id, amount, self.result
                )
            }

            HarnessEventKind::UpdateState { at_mono, order_id } => {
                format!(
                    "🔄 update_state @{} order_id={:?} [{:?}]",
                    at_mono, order_id, self.result
                )
            }
        }
    }
}

/// A full test execution trace.
///
/// This object must be:
/// - append-only
/// - reproducible
/// - serializable for replay and regression tests
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HarnessEventTrace {
    pub events: Vec<HarnessEvent>,
}

impl HarnessEventTrace {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn push(&mut self, event: HarnessEvent) {
        self.events.push(event);
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Pretty-print all events
    pub fn pretty(&self) -> String {
        self.events
            .iter()
            .enumerate()
            .map(|(i, e)| format!("{:03}: {}", i, e.pretty()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Deterministic hash for regression-test matching
    pub fn hash(&self) -> blake3::Hash {
        let encoded = bincode::serialize(self).expect("trace bincode");
        blake3::hash(&encoded)
    }
}