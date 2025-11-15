//! ============================================================================
//! CoreProver Test Harness - Fault Injection System (Chunk 7 of 9)
//!
//! This module mutates a HarnessEventTrace to:
//!   - drop events
//!   - reorder events
//!   - duplicate events
//!   - corrupt TXIDs and chain IDs
//!   - skew timestamps (mono + unix)
//!   - force deadline violations
//!   - inject random noise events
//!
//! Deterministic when a seed is provided.
//!
//! ============================================================================

use serde::{Deserialize, Serialize};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;

use super::trace::{HarnessEvent, HarnessEventKind, HarnessEventTrace};

/// Enumeration of supported fault types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FaultType {
    DropEvents(f32),              // % to drop
    DuplicateEvents(f32),         // % to duplicate
    ReorderRange(usize),          // window size for shuffling
    TimestampSkew { mono: i64, unix: i64 },
    CorruptTxids(f32),            // % to mutate txids
    CorruptChainIds(f32),         // % chance per event
    ForceLateFulfillment,         // modify deadlines
    InjectNoise(f32),             // % chance to insert fake events
}

/// A single applied fault (log entry)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultApplied {
    pub fault: FaultType,
    pub description: String,
}

/// Output trace + record of all faults applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultResult {
    pub mutated: HarnessEventTrace,
    pub log: Vec<FaultApplied>,
}

/// Fault injector with deterministic RNG
pub struct FaultInjector {
    rng: ChaCha20Rng,
}

impl FaultInjector {
    pub fn new(seed: Option<u64>) -> Self {
        let rng = match seed {
            Some(s) => ChaCha20Rng::seed_from_u64(s),
            None => ChaCha20Rng::from_entropy(),
        };
        Self { rng }
    }

    // =========================================================================
    // Public entry point
    // =========================================================================
    pub fn apply_faults(
        &mut self,
        original: &HarnessEventTrace,
        faults: &[FaultType],
    ) -> FaultResult {
        let mut trace = original.clone();
        let mut log = Vec::new();

        for fault in faults {
            match fault {
                FaultType::DropEvents(p) => {
                    self.drop_events(&mut trace, *p);
                    log.push(FaultApplied {
                        fault: fault.clone(),
                        description: format!("Dropped ~{}% of events", p * 100.0),
                    });
                }

                FaultType::DuplicateEvents(p) => {
                    self.duplicate_events(&mut trace, *p);
                    log.push(FaultApplied {
                        fault: fault.clone(),
                        description: format!("Duplicated ~{}% of events", p * 100.0),
                    });
                }

                FaultType::ReorderRange(win) => {
                    self.reorder_range(&mut trace, *win);
                    log.push(FaultApplied {
                        fault: fault.clone(),
                        description: format!("Reordered events within window {}", win),
                    });
                }

                FaultType::TimestampSkew { mono, unix } => {
                    self.timestamp_skew(&mut trace, *mono, *unix);
                    log.push(FaultApplied {
                        fault: fault.clone(),
                        description: format!("Skewed timestamps mono={} unix={}", mono, unix),
                    });
                }

                FaultType::CorruptTxids(p) => {
                    self.corrupt_txids(&mut trace, *p);
                    log.push(FaultApplied {
                        fault: fault.clone(),
                        description: format!("Corrupted ~{}% of txids", p * 100.0),
                    });
                }

                FaultType::CorruptChainIds(p) => {
                    self.corrupt_chain_ids(&mut trace, *p);
                    log.push(FaultApplied {
                        fault: fault.clone(),
                        description: format!("Corrupted ~{}% of chain IDs", p * 100.0),
                    });
                }

                FaultType::ForceLateFulfillment => {
                    self.force_late_fulfillment(&mut trace);
                    log.push(FaultApplied {
                        fault: fault.clone(),
                        description: "Forced deadlines to create late fulfillment".into(),
                    });
                }

                FaultType::InjectNoise(p) => {
                    self.inject_noise(&mut trace, *p);
                    log.push(FaultApplied {
                        fault: fault.clone(),
                        description: format!("Injected ~{}% noise events", p * 100.0),
                    });
                }
            }
        }

        FaultResult { mutated: trace, log }
    }

    // =========================================================================
    // Individual fault implementations
    // =========================================================================

    /// Randomly remove events
    fn drop_events(&mut self, trace: &mut HarnessEventTrace, pct: f32) {
        trace.events.retain(|_| self.rng.gen::<f32>() > pct);
    }

    /// Randomly duplicate events
    fn duplicate_events(&mut self, trace: &mut HarnessEventTrace, pct: f32) {
        let mut out = Vec::new();
        for evt in trace.events.iter().cloned() {
            out.push(evt.clone());
            if self.rng.gen::<f32>() < pct {
                out.push(evt);
            }
        }
        trace.events = out;
    }

    /// Shuffle windows of events to simulate reordering
    fn reorder_range(&mut self, trace: &mut HarnessEventTrace, window_size: usize) {
        if trace.events.len() < 2 {
            return;
        }

        let mut i = 0;
        while i + 1 < trace.events.len() {
            let end = usize::min(i + window_size, trace.events.len());
            let slice = &mut trace.events[i..end];
            slice.shuffle(&mut self.rng);
            i += window_size;
        }
    }

    /// Adds skew to timestamp offsets
    fn timestamp_skew(&mut self, trace: &mut HarnessEventTrace, mono: i64, unix: i64) {
        for evt in trace.events.iter_mut() {
            evt.offset_mono = evt.offset_mono.saturating_add_signed(mono);
            evt.offset_unix = evt.offset_unix.saturating_add_signed(unix);
        }
    }

    /// Mutate txids in events that carry them
    fn corrupt_txids(&mut self, trace: &mut HarnessEventTrace, pct: f32) {
        for evt in trace.events.iter_mut() {
            if self.rng.gen::<f32>() > pct {
                continue;
            }

            match &mut evt.kind {
                HarnessEventKind::BuyerCommit { txid, .. }
                | HarnessEventKind::SellerAccept { txid, .. }
                | HarnessEventKind::SellerFulfill { txid, .. }
                | HarnessEventKind::SellerClaim { txid, .. }
                | HarnessEventKind::SellerRefund { txid, .. }
                | HarnessEventKind::BuyerWithdraw { txid, .. } => {
                    *txid = self.corrupt_string(txid.clone());
                }

                _ => {}
            }
        }
    }

    /// Mutate numeric chain IDs
    fn corrupt_chain_ids(&mut self, trace: &mut HarnessEventTrace, pct: f32) {
        for evt in trace.events.iter_mut() {
            if self.rng.gen::<f32>() > pct {
                continue;
            }

            match &mut evt.kind {
                HarnessEventKind::BuyerCommit { buyer_chain_id, .. } => {
                    *buyer_chain_id = ((*buyer_chain_id as i64) ^ 0x1337) as u64;
                }
                _ => {}
            }
        }
    }

    /// Force deadlines to make fulfillment late
    fn force_late_fulfillment(&mut self, trace: &mut HarnessEventTrace) {
        for evt in trace.events.iter_mut() {
            if let HarnessEventKind::SellerAccept { .. } = evt.kind {
                evt.offset_mono += 86400;
            }
        }
    }

    /// Insert random noise events (fake buyer commits)
    fn inject_noise(&mut self, trace: &mut HarnessEventTrace, pct: f32) {
        let mut out = Vec::new();
        for evt in trace.events.iter().cloned() {
            out.push(evt.clone());

            if self.rng.gen::<f32>() < pct {
                out.push(HarnessEvent {
                    offset_mono: evt.offset_mono + 1,
                    offset_unix: evt.offset_unix + 1,
                    kind: HarnessEventKind::AdvanceTime { secs: 1 },
                });

                out.push(HarnessEvent {
                    offset_mono: evt.offset_mono + 2,
                    offset_unix: evt.offset_unix + 2,
                    kind: HarnessEventKind::BuyerCommit {
                        buyer: "noise".into(),
                        seller: "noise".into(),
                        amount: 1,
                        buyer_chain_id: 999,
                        txid: format!("noise_tx_{}", self.rng.gen::<u32>()),
                    },
                });
            }
        }
        trace.events = out;
    }

    // =========================================================================
    // Helper: mutate a string to make it invalid
    // =========================================================================
    fn corrupt_string(&mut self, s: String) -> String {
        let mut bytes = s.into_bytes();
        if !bytes.is_empty() {
            let idx = self.rng.gen_range(0..bytes.len());
            bytes[idx] ^= 0xFF; // flip bits
        }
        String::from_utf8_lossy(&bytes).to_string()
    }
}