// ============================================================================
// tests/harness/mod.rs
// CoreProver v0.3 Test Harness - Root Module
// ============================================================================
//
// This module provides the top-level interface for the CoreProver v0.3
// integration harness. All advanced tests, including:
//
//   * lifecycle replay tests
//   * event-driven simulation
//   * fault injection
//   * etherscan/RPC capture replay
//   * invariant checking
//   * timestamp validation
//   * cross-chain provenance tracking
//
//   ...depend on this root.
//
// The harness is intentionally lightweight and fully ASCII-safe.
// It can be expanded to a full B/C-grade lab harness later.
//
// ============================================================================

pub mod engine_driver;
pub mod trace_source;
pub mod faults;
pub mod invariants;
pub mod replay;

// Re-export for convenience
pub use engine_driver::EngineDriver;
pub use trace_source::{TraceSource, TraceEvent, TraceFrame};
pub use faults::{Fault, FaultType};
pub use invariants::{Invariant, InvariantResult};
pub use replay::{ReplayController, ReplayConfig};

// ============================================================================
// Shared harness types
// ============================================================================

use std::fmt;

// Standard Result alias for harness
pub type HResult<T> = Result<T, HarnessError>;

// ============================================================================
// HarnessError - Unified error type for all harness actions
// ============================================================================

#[derive(Debug)]
pub struct HarnessError {
    pub msg: String,
}

impl HarnessError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { msg: msg.into() }
    }
}

impl fmt::Display for HarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HarnessError: {}", self.msg)
    }
}

impl std::error::Error for HarnessError {}

// ============================================================================
// Event enums used by fault injector + replay system
// ============================================================================

/// High-level event that can be fed to the engine driver.
/// TraceSource produces these.
#[derive(Debug, Clone)]
pub enum HarnessEvent {
    BuyerCommit {
        buyer_id: String,
        seller_id: String,
        amount: u128,
        payment_profile: String,
    },
    SellerAccept {
        order_id: [u8; 32],
        commitment: String,
    },
    SellerFulfill {
        order_id: [u8; 32],
        proof: String,
    },
    SellerClaim {
        order_id: [u8; 32],
        proof: String,
    },
    BuyerWithdraw {
        order_id: [u8; 32],
        reason: Option<String>,
    },
    AdvanceTime {
        seconds: u64,
    },
}

/// Internal meta-event for tracing or debugging.
#[derive(Debug, Clone)]
pub enum HarnessMetaEvent {
    BeginScenario(String),
    EndScenario,
    FaultInjected(FaultType),
    InvariantCheck(String),
    EngineState(String),
}

// ============================================================================
// Utility helpers (IDs, hex encoding, timestamps)
// ============================================================================

/// Generate a random 32-byte order ID.
/// We keep it deterministic in tests by using a simple counter.
static mut ORDER_COUNTER: u64 = 0;

pub fn new_order_id() -> [u8; 32] {
    let n = unsafe {
        ORDER_COUNTER += 1;
        ORDER_COUNTER
    };
    let mut id = [0u8; 32];
    id[24..32].copy_from_slice(&n.to_be_bytes());
    id
}

/// Convert bytes to hex string
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Convert hex string to bytes (loose parser)
pub fn hex_decode(s: &str) -> HResult<Vec<u8>> {
    let s = s.trim_start_matches("0x");
    if s.len() % 2 != 0 {
        return Err(HarnessError::new("hex_decode: odd-length string"));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let mut chars = s.as_bytes().chunks(2);
    while let Some(chunk) = chars.next() {
        let hi = from_hex_char(chunk[0])?;
        let lo = from_hex_char(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn from_hex_char(c: u8) -> HResult<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(10 + (c - b'a')),
        b'A'..=b'F' => Ok(10 + (c - b'A')),
        _ => Err(HarnessError::new("invalid hex char")),
    }
}

// ============================================================================
// Timestamp helpers (for test scenarios only)
// ============================================================================

/// Minimal triple-timestamp generator for harness-only use.
/// Engine uses TimestampProvider; harness provides values for synthetic events.
#[derive(Debug, Clone)]
pub struct TestTimestamp {
    pub mono: u64,
    pub unix: u64,
    pub iso: String,
}

impl TestTimestamp {
    pub fn new(mono: u64, unix: u64, iso: impl Into<String>) -> Self {
        Self {
            mono,
            unix,
            iso: iso.into(),
        }
    }

    pub fn advance(&mut self, secs: u64) {
        self.mono += secs;
        self.unix += secs;
        // We intentionally do NOT recompute ISO timestamps in the harness.
    }
}

// ============================================================================
// Scenario metadata
// ============================================================================

/// Metadata describing a scenario for logging + debugging.
#[derive(Debug, Clone)]
pub struct ScenarioMeta {
    pub name: String,
    pub description: String,
    pub source: String,
    pub event_count: usize,
    pub fault_count: usize,
}

impl ScenarioMeta {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            source: String::new(),
            event_count: 0,
            fault_count: 0,
        }
    }
}

// ============================================================================
// Root-level integration test loader
// ============================================================================

/// Load a scenario JSON file from tests/scenarios/.
pub fn load_scenario_json(name: &str) -> HResult<String> {
    use std::fs;
    let path = format!("tests/scenarios/{}", name);
    fs::read_to_string(&path)
        .map_err(|e| HarnessError::new(format!("cannot load scenario '{}': {}", name, e)))
}