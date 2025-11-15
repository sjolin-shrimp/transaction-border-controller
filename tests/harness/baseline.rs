//! ============================================================================
//! CoreProver Test Harness - Baseline Scenario Generator (Chunk 5 of 9)
//!
//! This module creates *deterministic* baseline traces from declarative
//! descriptions.
//
//! Example use:
//!
//! let profile = BaselineProfile::simple_buyer_to_claim();
//! let baseline = BaselineBuilder::new().build(&profile);
//!
//! This baseline can then be executed directly, or fed into fault injection.
//!
//! ============================================================================

use serde::{Deserialize, Serialize};

use super::trace::{HarnessEvent, HarnessEventKind, HarnessEventTrace};

/// A declarative description of a scenario.
///
/// The builder turns this into a fully timestamped event sequence.
///
/// Think of this as the "storyboard" for a test case.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BaselineProfile {
    /// Human-readable name
    pub name: String,

    /// Sequence of intended high-level actions
    ///
    /// The builder converts these into timestamped HarnessEvents.
    pub steps: Vec<BaselineStep>,

    /// Optional: starting UNIX for deterministic timestamping
    pub genesis_unix: u64,

    /// Optional: starting monotonic
    pub genesis_mono: u64,

    /// Optional: default inter-event spacing (in seconds)
    ///
    /// If None → no auto spacing (tests explicitly define ADVANCE_TIME)
    pub default_spacing_secs: Option<u64>,
}

/// High-level intended test action
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BaselineStep {
    AdvanceTime { secs: u64 },

    BuyerCommit {
        buyer: String,
        seller: String,
        amount: u64,
        txid: String,
    },

    SellerAccept {
        txid: String,
    },

    SellerFulfill {
        txid: String,
    },

    SellerClaim {
        txid: String,
    },

    SellerRefund {
        txid: String,
    },

    BuyerWithdraw {
        txid: Option<String>,
    },

    TimedRelease {},

    /// state sync request (useful in long scenarios)
    UpdateState {},
}

/// A scenario fully expanded into timestamped events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BaselineScenario {
    pub name: String,
    pub trace: HarnessEventTrace,
}

/// Turns BaselineProfile into a BaselineScenario.
///
/// This is intentionally simple and deterministic.
/// All “intelligence” belongs in the CoreProver engine, not here.
pub struct BaselineBuilder {
    current_mono: u64,
    current_unix: u64,

    default_spacing_secs: Option<u64>,
}

impl BaselineBuilder {
    pub fn new() -> Self {
        Self {
            current_mono: 0,
            current_unix: 0,
            default_spacing_secs: None,
        }
    }

    pub fn build(mut self, profile: &BaselineProfile) -> BaselineScenario {
        self.current_mono = profile.genesis_mono;
        self.current_unix = profile.genesis_unix;
        self.default_spacing_secs = profile.default_spacing_secs;

        let mut events = Vec::new();

        // Build events from steps
        for step in &profile.steps {
            // auto-spacing: only if user didn’t explicitly add ADVANCE_TIME
            if let Some(space) = self.default_spacing_secs {
                if !matches!(step, BaselineStep::AdvanceTime { .. }) {
                    if space > 0 {
                        self.advance(events.as_mut(), space);
                    }
                }
            }

            match step {
                BaselineStep::AdvanceTime { secs } => {
                    self.advance(events.as_mut(), *secs);
                }

                BaselineStep::BuyerCommit {
                    buyer,
                    seller,
                    amount,
                    txid,
                } => {
                    events.push(HarnessEvent {
                        kind: HarnessEventKind::BuyerCommit {
                            buyer: buyer.clone(),
                            seller: seller.clone(),
                            amount: *amount,
                            txid: txid.clone(),
                            at_mono: self.current_mono,
                            at_unix: self.current_unix,
                        },
                    });
                }

                BaselineStep::SellerAccept { txid } => {
                    events.push(HarnessEvent {
                        kind: HarnessEventKind::SellerAccept {
                            txid: txid.clone(),
                            at_mono: self.current_mono,
                            at_unix: self.current_unix,
                        },
                    });
                }

                BaselineStep::SellerFulfill { txid } => {
                    events.push(HarnessEvent {
                        kind: HarnessEventKind::SellerFulfill {
                            txid: txid.clone(),
                            at_mono: self.current_mono,
                            at_unix: self.current_unix,
                        },
                    });
                }

                BaselineStep::SellerClaim { txid } => {
                    events.push(HarnessEvent {
                        kind: HarnessEventKind::SellerClaim {
                            txid: txid.clone(),
                            at_mono: self.current_mono,
                            at_unix: self.current_unix,
                        },
                    });
                }

                BaselineStep::SellerRefund { txid } => {
                    events.push(HarnessEvent {
                        kind: HarnessEventKind::SellerRefund {
                            txid: txid.clone(),
                            at_mono: self.current_mono,
                            at_unix: self.current_unix,
                        },
                    });
                }

                BaselineStep::BuyerWithdraw { txid } => {
                    events.push(HarnessEvent {
                        kind: HarnessEventKind::BuyerWithdraw {
                            txid: txid.clone(),
                            at_mono: self.current_mono,
                            at_unix: self.current_unix,
                        },
                    });
                }

                BaselineStep::TimedRelease {} => {
                    events.push(HarnessEvent {
                        kind: HarnessEventKind::TimedRelease {
                            at_mono: self.current_mono,
                            at_unix: self.current_unix,
                        },
                    });
                }

                BaselineStep::UpdateState {} => {
                    events.push(HarnessEvent {
                        kind: HarnessEventKind::UpdateState {
                            at_mono: self.current_mono,
                            at_unix: self.current_unix,
                        },
                    });
                }
            }
        }

        BaselineScenario {
            name: profile.name.clone(),
            trace: HarnessEventTrace { events },
        }
    }

    // ---------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------
    fn advance(&mut self, out: &mut Vec<HarnessEvent>, secs: u64) {
        self.current_mono += secs;
        self.current_unix += secs;

        out.push(HarnessEvent {
            kind: HarnessEventKind::AdvanceTime { secs },
        });
    }
}

// ==========================================================================
// Convenience helpers — produce common canonical profiles
// ==========================================================================

impl BaselineProfile {
    /// Straightforward buyer → accept → fulfill → claim sequence
    pub fn simple_claim() -> Self {
        Self {
            name: "simple_claim".into(),
            genesis_unix: 1_731_600_000,
            genesis_mono: 0,
            default_spacing_secs: Some(10),
            steps: vec![
                BaselineStep::BuyerCommit {
                    buyer: "buyer1".into(),
                    seller: "seller1".into(),
                    amount: 30000000,
                    txid: "tx_b_commit".into(),
                },
                BaselineStep::SellerAccept {
                    txid: "tx_s_accept".into(),
                },
                BaselineStep::SellerFulfill {
                    txid: "tx_s_fulfill".into(),
                },
                BaselineStep::SellerClaim {
                    txid: "tx_s_claim".into(),
                },
            ],
        }
    }

    /// Fulfill after deadline → SellerClaim still happens
    pub fn late_fulfill_then_claim() -> Self {
        Self {
            name: "late_fulfill_then_claim".into(),
            genesis_unix: 1_731_600_000,
            genesis_mono: 0,
            default_spacing_secs: None,
            steps: vec![
                BaselineStep::BuyerCommit {
                    buyer: "buyer1".into(),
                    seller: "seller1".into(),
                    amount: 3000,
                    txid: "tx_b_commit".into(),
                },
                BaselineStep::AdvanceTime { secs: 200 }, // past acceptance
                BaselineStep::SellerAccept {
                    txid: "tx_s_accept".into(),
                },
                BaselineStep::AdvanceTime { secs: 500 }, // past fulfillment window
                BaselineStep::SellerFulfill {
                    txid: "tx_s_fulfill".into(),
                },
                BaselineStep::SellerClaim {
                    txid: "tx_s_claim".into(),
                },
            ],
        }
    }

    /// Acceptance never arrives → withdraw
    pub fn accept_expired_then_withdraw() -> Self {
        Self {
            name: "accept_expired_then_withdraw".into(),
            genesis_unix: 1_731_600_000,
            genesis_mono: 0,
            default_spacing_secs: None,
            steps: vec![
                BaselineStep::BuyerCommit {
                    buyer: "buyer1".into(),
                    seller: "seller1".into(),
                    amount: 1000,
                    txid: "tx_b_commit".into(),
                },
                BaselineStep::AdvanceTime { secs: 300 }, // past acceptance
                BaselineStep::BuyerWithdraw { txid: Some("tx_b_withdraw".into()) },
            ],
        }
    }

    /// Fulfilled but seller forgets to claim → timed release
    pub fn fulfill_then_timed_release() -> Self {
        Self {
            name: "fulfill_then_timed_release".into(),
            genesis_unix: 1_731_600_000,
            genesis_mono: 0,
            default_spacing_secs: None,
            steps: vec![
                BaselineStep::BuyerCommit {
                    buyer: "buyer1".into(),
                    seller: "seller1".into(),
                    amount: 5000,
                    txid: "tx_b_commit".into(),
                },
                BaselineStep::SellerAccept { txid: "tx_s_accept".into() },
                BaselineStep::SellerFulfill { txid: "tx_s_fulfill".into() },
                BaselineStep::AdvanceTime { secs: 99999 },
                BaselineStep::TimedRelease {},
            ],
        }
    }
}