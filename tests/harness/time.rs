// ============================================================================
// CoreProver v0.3 Test Harness — Deterministic Triple-Clock Controller
// File: tests/harness/time.rs
//
// This module provides the **TimeController**, the deterministic
// simulation clock used for ALL CoreProver test harnesses.
//
// Why this exists:
//     - System time must never influence tests.
//     - CoreProver v0.3 relies heavily on triple timestamps:
//           (monotonic, unix, iso8601)
//     - Engine state transitions depend on elapsed time AND block heights.
//     - Everything must be deterministic, replayable, and isolated.
//
// This module is the single source of time truth inside the harness.
//
// ============================================================================

#![allow(dead_code)]

use chrono::{DateTime, NaiveDateTime, Utc};

use coreprover_service::engine::CoreProverEngine;

// ============================================================================
// TripleTimestamp helper — test-only constructor
// ============================================================================

use crate::harness::types::TripleTimestamp;

// ============================================================================
// TimeController
// ============================================================================

#[derive(Debug, Clone)]
pub struct TimeController {
    /// Monotonic clock in seconds since test-start.
    /// Test-only: this is NOT system time.
    mono: u64,

    /// UNIX timestamp (seconds).
    unix: u64,

    /// ISO8601 formatted timestamp.
    iso: String,
}

impl TimeController {
    /// Construct a fresh deterministic clock.
    ///
    /// All tests start at:
    ///     unix  = DEFAULT_GENESIS_UNIX   (in engine_factory)
    ///     mono  = 0
    ///     iso   = ISO(timestamp(default_genesis))
    pub fn new() -> Self {
        // EngineFactory defines DEFAULT_GENESIS_UNIX,
        // but TimeController should NOT depend on EngineFactory directly.
        //
        // For safety, we detect the genesis timestamp from the engine
        // at runtime when `apply_time_to_engine` is called.
        //
        // Here we use a placeholder and correct it at first engine sync.
        let placeholder_unix = 1_700_000_000;

        let iso = unix_to_iso(placeholder_unix);

        Self {
            mono: 0,
            unix: placeholder_unix,
            iso,
        }
    }

    // ------------------------------------------------------------------------
    // Basic getters
    // ------------------------------------------------------------------------

    pub fn now_mono(&self) -> u64 {
        self.mono
    }

    pub fn now_unix(&self) -> u64 {
        self.unix
    }

    pub fn now_iso(&self) -> String {
        self.iso.clone()
    }

    // ------------------------------------------------------------------------
    // Clock advancement
    // ------------------------------------------------------------------------

    /// Advance both monotonic and unix clocks.
    ///
    /// This is the *ONLY* way time should advance in tests.
    pub fn advance_raw(&mut self, secs: u64) {
        self.mono = self.mono.saturating_add(secs);
        self.unix = self.unix.saturating_add(secs);
        self.iso = unix_to_iso(self.unix);
    }

    /// Advance time AND apply updates to the engine.
    ///
    /// This ensures:
    ///   - block heights increment properly
    ///   - state-machine deadlines are respected
    ///   - fulfillment/claim windows tick forward
    ///
    /// The engine MUST NOT call system time internally.
    pub fn advance(&mut self, engine: &mut CoreProverEngine, secs: u64) {
        self.advance_raw(secs);
        self.apply_time_to_engine(engine);
    }

    // ------------------------------------------------------------------------
    // Engine synchronization
    // ------------------------------------------------------------------------

    /// Apply current time to the engine:
    ///     - Set engine.current_block_height
    ///     - Trigger engine updates via update_state() if needed
    ///
    /// Engine fields expected:
    ///     - engine.current_block_height
    ///     - engine.block_interval_secs
    ///     - engine.genesis_unix
    ///
    /// Any test that calls `advance()` will guarantee that
    /// engine time is consistent with test time.
    pub fn apply_time_to_engine(&mut self, engine: &mut CoreProverEngine) {
        // If the engine hasn't synced, fix our unix timestamp to match.
        if self.unix != engine.genesis_unix() && self.mono == 0 {
            self.unix = engine.genesis_unix();
            self.iso = unix_to_iso(self.unix);
        }

        // Compute expected block height
        let diff = self.unix.saturating_sub(engine.genesis_unix());
        let height = diff / engine.block_interval_secs();

        engine._set_block_height_for_tests(height);

        // Tell engine to refresh internal deadlines
        engine._tick_for_tests(self.unix, self.mono);
    }
}

// ============================================================================
// Helper conversions
// ============================================================================

/// Convert a unix timestamp → ISO8601 string.
///
/// This must remain deterministic, no local timezones.
pub fn unix_to_iso(unix: u64) -> String {
    let nd = NaiveDateTime::from_timestamp_opt(unix as i64, 0)
        .unwrap_or(NaiveDateTime::from_timestamp_opt(0, 0).unwrap());
    DateTime::<Utc>::from_utc(nd, Utc).to_rfc3339()
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::engine_factory::EngineFactory;
    use crate::harness::chain::TestChain;

    #[test]
    fn time_advances_monotonic_and_unix() {
        let mut t = TimeController::new();
        assert_eq!(t.now_mono(), 0);

        t.advance_raw(10);
        assert_eq!(t.now_mono(), 10);
        assert_eq!(t.now_unix(), 1_700_000_010);
    }

    #[test]
    fn iso_format_updates_when_time_advances() {
        let mut t = TimeController::new();
        let iso1 = t.now_iso();

        t.advance_raw(60);
        let iso2 = t.now_iso();

        assert_ne!(iso1, iso2);
    }

    #[test]
    fn engine_syncs_block_height_correctly() {
        let factory = EngineFactory::local_mock();
        let mut engine = factory.spawn();
        let mut t = TimeController::new();

        // First sync initializes unix to engine genesis
        t.apply_time_to_engine(&mut engine);

        let start_height = engine.current_block_height();

        t.advance(&mut engine, 120); // 120 seconds → 10 blocks @ 12 sec
        let height_after = engine.current_block_height();

        assert_eq!(height_after, start_height + 10);
    }

    #[test]
    fn engine_syncs_monotonic_timestamp() {
        let factory = EngineFactory::local_mock();
        let mut engine = factory.spawn();
        let mut t = TimeController::new();

        t.apply_time_to_engine(&mut engine);

        t.advance(&mut engine, 5);
        assert_eq!(t.now_mono(), 5);

        // No direct comparison to engine; engine only stores block height,
        // but internal logic uses the monotonic stamp via _tick_for_tests().
    }
}