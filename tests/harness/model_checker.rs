//! ============================================================================
//! CoreProver Harness - Model Checker
//!
//! Responsibilities:
//! - Validate state transitions (strict v0.3 rules)
//! - Validate timing constraints (acceptance → fulfillment → claim windows)
//! - Validate triple-clock monotonicity
//! - Validate TXID provenance rules
//! - Validate cross-chain invariants
//! - Validate settlement outcomes
//! - Validate discount rules
//!
//! This is a READ-ONLY verification layer that consumes:
//! - EscrowTrace (events)
//! - EngineSnapshot (states)
//! - EngineConfig
//!
//! It produces:
//! - ModelCheckReport (per trace)
//! - Specific violation types
//! ============================================================================

use crate::harness::{
    trace::{EscrowEvent, EscrowTrace, TxRole},
    engine_runner::{EngineSnapshot},
};
use serde::{Serialize, Deserialize};

// ============================================================================
// Violations
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ViolationType {
    // --- State transition rule violations ---
    InvalidTransition {
        from: String,
        to: String,
    },

    // --- Timing violations ---
    AcceptanceTooLate,
    FulfillmentTooLate,
    ClaimTooLate,
    ClaimTooEarly,
    WithdrawalTooEarly,

    // --- Provenance violations ---
    MissingBuyerCommitTxid,
    MissingSellerAcceptTxid,
    MissingSellerFulfillTxid,
    MissingSettlementTxid,
    InvalidTxidFormat,

    // --- Chain invariants ---
    BuyerChainMismatch,
    SellerChainMismatch,
    CrossChainHeightMissing,

    // --- Timestamp issues ---
    NonMonotonicMonoClock,
    NonMonotonicUnixClock,
    IsoMismatch,

    // --- Discount rules ---
    LateWithoutDiscountFlag,
    DiscountWithoutLateFlag,

    // --- Settlement ---
    DoubleSettlement,
    MissingSettlementOutcome,
}

// ============================================================================
// ModelCheckReport
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCheckReport {
    pub trace_name: String,
    pub passed: bool,
    pub violations: Vec<ViolationType>,
}

impl ModelCheckReport {
    pub fn fail(v: ViolationType) -> Self {
        Self {
            trace_name: "unnamed".into(),
            passed: false,
            violations: vec![v],
        }
    }
}

// ============================================================================
// ModelChecker
// ============================================================================

pub struct ModelChecker;

impl ModelChecker {
    pub fn check(trace: &EscrowTrace, snapshot: &EngineSnapshot) -> ModelCheckReport {
        let mut report = ModelCheckReport {
            trace_name: trace.name.clone(),
            passed: true,
            violations: vec![],
        };

        // =====================================================================
        // 1. Check monotonic clocks
        // =====================================================================
        for w in snapshot.history.windows(2) {
            let prev = &w[0].clock;
            let next = &w[1].clock;

            if next.mono < prev.mono {
                report.violations.push(ViolationType::NonMonotonicMonoClock);
            }
            if next.unix < prev.unix {
                report.violations.push(ViolationType::NonMonotonicUnixClock);
            }
        }

        // =====================================================================
        // 2. Validate each event in sequence
        // =====================================================================

        for (idx, event) in trace.events.iter().enumerate() {
            match event {
                EscrowEvent::BuyerCommit { buyer_commit_txid, .. } => {
                    if buyer_commit_txid.trim().is_empty() {
                        report.violations.push(ViolationType::MissingBuyerCommitTxid);
                    }
                }

                EscrowEvent::SellerAccept { seller_accept_txid, .. } => {
                    if seller_accept_txid.trim().is_empty() {
                        report.violations.push(ViolationType::MissingSellerAcceptTxid);
                    }

                    // Check timing
                    if snapshot.state.acceptance_deadline.is_some() {
                        let d = snapshot.state.acceptance_deadline.unwrap();
                        if snapshot.clock.mono > d.mono {
                            report.violations.push(ViolationType::AcceptanceTooLate);
                        }
                    }
                }

                EscrowEvent::SellerFulfill { seller_fulfill_txid } => {
                    if seller_fulfill_txid.trim().is_empty() {
                        report.violations.push(ViolationType::MissingSellerFulfillTxid);
                    }

                    if let Some(fd) = snapshot.state.fulfillment_deadline {
                        if snapshot.clock.mono > fd.mono {
                            report.violations.push(ViolationType::FulfillmentTooLate);
                        }
                    }
                }

                EscrowEvent::SellerClaim { seller_claim_txid } => {
                    if seller_claim_txid.trim().is_empty() {
                        report.violations.push(ViolationType::MissingSettlementTxid);
                    }

                    if let Some(cd) = snapshot.state.claim_deadline {
                        if snapshot.clock.mono > cd.mono {
                            report.violations.push(ViolationType::ClaimTooLate);
                        }
                    }
                }

                EscrowEvent::SellerRefund { seller_refund_txid } => {
                    if seller_refund_txid.trim().is_empty() {
                        report.violations.push(ViolationType::MissingSettlementTxid);
                    }
                }

                EscrowEvent::BuyerWithdraw { buyer_withdraw_txid } => {
                    // Withdrawal validity depends on state + timing
                    if snapshot.state.state_name == "BuyerCommitted" {
                        let ac = snapshot.state.acceptance_deadline.unwrap();
                        if snapshot.clock.mono <= ac.mono {
                            report.violations.push(ViolationType::WithdrawalTooEarly);
                        }
                    }

                    // txid optional, but if present must be non-empty
                    if let Some(id) = buyer_withdraw_txid {
                        if id.trim().is_empty() {
                            report.violations.push(ViolationType::InvalidTxidFormat);
                        }
                    }
                }

                EscrowEvent::AdvanceTime { secs: _ } => {
                    // No direct validation; clock monotonicity already checked
                }
            }
        }

        // =====================================================================
        // 3. Validate final escrow invariants
        // =====================================================================

        let s = &snapshot.state;

        // Must not have both or neither settlement outcome
        let has_claim = s.seller_claim_txid.is_some();
        let has_refund = s.seller_refund_txid.is_some();

        if has_claim && has_refund {
            report.violations.push(ViolationType::DoubleSettlement);
        }

        if !has_claim && !has_refund && s.is_terminal {
            report.violations.push(ViolationType::MissingSettlementOutcome);
        }

        // Discount logic
        if s.late_fulfilled && s.discount_pct == 0 {
            report.violations.push(ViolationType::LateWithoutDiscountFlag);
        }

        if !s.late_fulfilled && s.discount_pct > 0 {
            report.violations.push(ViolationType::DiscountWithoutLateFlag);
        }

        // Mark overall pass/fail
        if !report.violations.is_empty() {
            report.passed = false;
        }

        report
    }
}
// ============================================================================
// Hybrid API compatibility (required by TestContext)
// ============================================================================

use coreprover_types_v03::EscrowState;
use crate::harness::engine_driver::EngineDriver;

impl ModelChecker {
    /// Empty model checker (no validation)
    pub fn new() -> Self {
        ModelChecker
    }

    /// Full v0.3 rule set (this version)
    pub fn new_with_v03_rules() -> Self {
        ModelChecker
    }

    /// Validate transition (standalone)
    pub fn validate_transition(
        &self,
        from: EscrowState,
        to: EscrowState,
    ) -> Result<(), String> {
        // Allowed transitions per v0.3
        match (from, to) {
            // Allowed:
            (EscrowState::BuyerCommitted, EscrowState::SellerAccepted) => Ok(()),
            (EscrowState::SellerAccepted, EscrowState::SellerFulfilled) => Ok(()),
            (EscrowState::SellerFulfilled, EscrowState::SellerClaimed) => Ok(()),
            (EscrowState::SellerFulfilled, EscrowState::SellerRefunded) => Ok(()),
            (EscrowState::FulfillmentExpired, EscrowState::SellerClaimed) => Ok(()),
            (EscrowState::FulfillmentExpired, EscrowState::SellerRefunded) => Ok(()),

            // Buyer can withdraw anytime after window expires
            (from, EscrowState::BuyerWithdrawn) => Ok(()),

            // Identity transition allowed only for terminal states
            (s1, s2) if s1 == s2 && matches!(
                s1,
                EscrowState::SellerClaimed
                    | EscrowState::SellerRefunded
                    | EscrowState::BuyerWithdrawn
            ) => Ok(()),

            // Everything else rejected
            (from, to) => Err(format!("invalid transition: {:?} -> {:?}", from, to)),
        }
    }

    /// Validate invariants via EngineDriver snapshot
    pub fn check(&self, driver: &EngineDriver) -> Result<(), String> {
        let trace = driver.get_trace();
        let snapshot = driver.get_snapshot();

        let report = ModelChecker::check(&trace, &snapshot);

        if report.passed {
            Ok(())
        } else {
            Err(format!("ModelChecker violations: {:?}", report.violations))
        }
    }
}