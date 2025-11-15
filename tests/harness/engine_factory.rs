// ============================================================================
// CoreProver v0.3 Test Harness — Engine Factory
// File: tests/harness/engine_factory.rs
//
// Creates fully configured CoreProverEngine instances.
//
// The engine **MUST NOT** be constructed directly in tests because:
//   - Time must be deterministic.
//   - Chain configuration must be explicit.
//   - Tests must share the same initialization semantics.
//   - Profiles should attach cleanly via the harness.
//   - v0.3 requires (chain_id, block_interval_secs, genesis_unix).
//
// This factory enforces all of that.
//
// ============================================================================

#![allow(dead_code)]
#![allow(unused_imports)]

use crate::harness::time::TimeController;
use coreprover_service::engine::CoreProverEngine;

use std::time::{SystemTime, UNIX_EPOCH};

/// Fixed block interval for all harness simulations (seconds).
/// In production this varies by chain, but for tests uniformity wins.
///
/// NOTE: Engines should NOT rely on this except through the harness.
pub const DEFAULT_BLOCK_INTERVAL: u64 = 12;

/// Deterministic "genesis" for all tests unless caller requests override.
/// This timestamp is stable across all machines and executions.
pub const DEFAULT_GENESIS_UNIX: u64 = 1_700_000_000;

/// Defines which blockchain environment the engine is simulating.
/// It also maps to chain IDs in the real world.
///
/// v0.3 engine requires a chain_id in all receipts and state views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestChain {
    /// PulseChain (Mainnet)
    PulseChain,

    /// Base / OP stack
    Base,

    /// Ethereum mainnet
    Ethereum,

    /// Local ephemeral chain for highly stable testing.
    LocalMock,
}

impl TestChain {
    /// Map to a canonical chain_id used by the engine.
    pub fn chain_id(self) -> u64 {
        match self {
            TestChain::PulseChain => 369,
            TestChain::Base => 8453,
            TestChain::Ethereum => 1,
            TestChain::LocalMock => 999_999,
        }
    }

    /// Describe the chain for debugging.
    pub fn name(self) -> &'static str {
        match self {
            TestChain::PulseChain => "pulsechain",
            TestChain::Base => "base",
            TestChain::Ethereum => "ethereum",
            TestChain::LocalMock => "local-mock",
        }
    }
}

// ============================================================================
// EngineFactory
// ============================================================================

/// High-level builder for spawning CoreProverEngine instances.
///
/// Typical usage:
///
/// ```rust
/// use harness::*;
///
/// let mut engine = EngineFactory::pulsechain().spawn();
/// let mut t = TimeController::new();
/// t.advance(&mut engine, 300);
/// ```
///
/// The factory enforces proper configuration:
///     - chain_id
///     - block_interval_secs
///     - genesis timestamp
///
/// v0.3 requires all three.
#[derive(Debug, Clone)]
pub struct EngineFactory {
    chain: TestChain,
    block_interval: u64,
    genesis_unix: u64,
}

impl EngineFactory {
    // --------------------------------------------------------------------
    // Constructors for each chain preset
    // --------------------------------------------------------------------

    /// PulseChain preset (our primary target chain)
    pub fn pulsechain() -> Self {
        Self {
            chain: TestChain::PulseChain,
            block_interval: DEFAULT_BLOCK_INTERVAL,
            genesis_unix: DEFAULT_GENESIS_UNIX,
        }
    }

    /// Base chain preset
    pub fn base() -> Self {
        Self {
            chain: TestChain::Base,
            block_interval: DEFAULT_BLOCK_INTERVAL,
            genesis_unix: DEFAULT_GENESIS_UNIX,
        }
    }

    /// Ethereum chain preset
    pub fn ethereum() -> Self {
        Self {
            chain: TestChain::Ethereum,
            block_interval: DEFAULT_BLOCK_INTERVAL,
            genesis_unix: DEFAULT_GENESIS_UNIX,
        }
    }

    /// Stable deterministic mock chain
    pub fn local_mock() -> Self {
        Self {
            chain: TestChain::LocalMock,
            block_interval: DEFAULT_BLOCK_INTERVAL,
            genesis_unix: DEFAULT_GENESIS_UNIX,
        }
    }

    // --------------------------------------------------------------------
    // Custom setters
    // --------------------------------------------------------------------

    pub fn with_block_interval(mut self, secs: u64) -> Self {
        self.block_interval = secs;
        self
    }

    pub fn with_genesis(mut self, unix_ts: u64) -> Self {
        self.genesis_unix = unix_ts;
        self
    }

    pub fn with_chain(mut self, chain: TestChain) -> Self {
        self.chain = chain;
        self
    }

    // --------------------------------------------------------------------
    // Engine spawner (the purpose of this module)
    // --------------------------------------------------------------------

    /// Instantiate a fully configured CoreProverEngine.
    ///
    /// Matches engine.rs constructor:
    ///
    ///     pub fn new(chain_id: u64, block_interval_secs: u64, genesis_unix: u64)
    ///
    pub fn spawn(&self) -> CoreProverEngine {
        let chain_id = self.chain.chain_id();

        let mut engine = CoreProverEngine::new(
            chain_id,
            self.block_interval,
            self.genesis_unix,
        );

        // Optional hook where we can preload default payment profiles,
        // or configure a test-mode registry.
        //
        // This becomes important when we add:
        //     TBC-driven profile deployment
        //     on-chain profile resolution semantics
        //
        // For v0.3 we leave it as a stub.
        self.initialize_profiles(&mut engine);

        engine
    }

    /// Stub for injecting payment profiles into engine.
    ///
    /// In the real system this might:
    ///     - Deploy a CoreProver settlement contract
    ///     - Register profile IDs
    ///     - Load deadlines, fee settings, legal commit requirements
    ///
    /// In v0.3 tests this will remain empty unless scenario modules request it.
    fn initialize_profiles(&self, _engine: &mut CoreProverEngine) {
        // no-op
    }
}

// ============================================================================
// Engine + Time Initialization Helpers
// ============================================================================

/// Construct `(engine, time_controller)` in one shot.
pub fn spawn_engine_with_time(chain: TestChain) -> (CoreProverEngine, TimeController) {
    let factory = EngineFactory::with_chain(EngineFactory::local_mock(), chain);
    let engine = factory.spawn();
    let t = TimeController::new();
    (engine, t)
}

/// Construct a default PulseChain engine + time controller.
pub fn pulsechain_engine() -> (CoreProverEngine, TimeController) {
    spawn_engine_with_time(TestChain::PulseChain)
}

/// Construct a default Base engine + time controller.
pub fn base_engine() -> (CoreProverEngine, TimeController) {
    spawn_engine_with_time(TestChain::Base)
}

// ============================================================================
// Tests (ensuring factory is operational)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_constructs_engine() {
        let factory = EngineFactory::pulsechain();
        let engine = factory.spawn();

        assert_eq!(engine.chain_id(), TestChain::PulseChain.chain_id());
    }

    #[test]
    fn factory_sets_block_interval() {
        let factory = EngineFactory::pulsechain().with_block_interval(5);
        let engine = factory.spawn();

        assert_eq!(engine.block_interval_secs(), 5);
    }

    #[test]
    fn factory_with_custom_chain() {
        let factory = EngineFactory::local_mock().with_chain(TestChain::Ethereum);
        let engine = factory.spawn();

        assert_eq!(engine.chain_id(), 1);
    }

    #[test]
    fn spawn_engine_and_time() {
        let (engine, time) = pulsechain_engine();
        assert_eq!(engine.chain_id(), TestChain::PulseChain.chain_id());
        assert_eq!(time.now_unix(), DEFAULT_GENESIS_UNIX);
    }
}
