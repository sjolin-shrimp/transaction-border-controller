// ============================================================================
// CoreProver v0.3 Test Harness
// File: tests/harness/context.rs
//
// TestContext is the orchestrator that ties together:
//   * TimeController (deterministic clock)
//   * MockChain (deterministic TXID + block height)
//   * EngineFactory (one engine per chain)
//   * Harness timestamp/session/order types
//
// This creates a full multi-chain testing runtime for CoreProver v0.3.
// ============================================================================

#![allow(dead_code)]

use std::collections::HashMap;

use super::engine_factory::EngineFactory;
use super::mock_chain::{MockChain, MockChainId, TxStamp};
use super::time::{TimeController, EngineTimeSnapshot};
use super::types::{
    HarnessTimestamp,
    HarnessSessionId,
    HarnessOrderId,
    HarnessAsset,
    HarnessAmount,
};

// Bring in engine types
use coreprover_engine::CoreProverEngine;
use coreprover_types_v03::{CoreProverReceipt, EscrowState};

// ============================================================================
// TestContext
// ============================================================================

pub struct TestContext {
    /// Global deterministic time controller (shared for all chains)
    time: TimeController,

    /// One engine per chain
    engines: HashMap<MockChainId, CoreProverEngine>,

    /// Mock chains for TXID + block height simulation
    chains: HashMap<MockChainId, MockChain>,

    /// Session + order counters for reproducible test IDs
    session_counter: u64,
    order_counter: u64,
}

impl TestContext {
    /// Create a new empty test context
    pub fn new() -> Self {
        Self {
            time: TimeController::default(),
            engines: HashMap::new(),
            chains: HashMap::new(),
            session_counter: 1,
            order_counter: 1,
        }
    }

    // =========================================================================
    // Chain Registration
    // =========================================================================

    /// Create or fetch a chain inside the harness.
    ///
    /// This automatically:
    ///   - Creates a new MockChain
    ///   - Creates a new CoreProverEngine
    ///   - Synchronizes engine timestamps with TimeController
    pub fn chain(&mut self, chain_id: MockChainId) -> MockChainId {
        if !self.chains.contains_key(&chain_id) {
            // Create a mock chain
            let chain = MockChain::new(chain_id);
            self.chains.insert(chain_id, chain);

            // Create a corresponding engine
            let engine = EngineFactory::build(
                chain_id,
                12, // block every 12 seconds
                self.time.unix_now(),
            );
            self.engines.insert(chain_id, engine);
        }
        chain_id
    }

    // =========================================================================
    // Time Advancement
    // =========================================================================

    /// Advance global time and block heights on all chains.
    ///
    /// This ensures all engines AND all mock chains remain perfectly synchronized
    /// under the deterministic triple-clock model.
    pub fn advance_time(&mut self, secs: u64) {
        // Advance global clock
        self.time.advance(secs);

        // Advance all engines
        for engine in self.engines.values_mut() {
            engine.advance_time(secs);
        }

        // Advance all mock chains
        for chain in self.chains.values_mut() {
            chain.advance_time(secs);
        }
    }

    // =========================================================================
    // Session + Order Generation
    // =========================================================================

    fn next_session_id(&mut self, label: &str) -> HarnessSessionId {
        let s = HarnessSessionId::new(label, self.session_counter);
        self.session_counter += 1;
        s
    }

    fn next_order_id(&mut self, label: &str) -> HarnessOrderId {
        let o = HarnessOrderId::new(label, self.order_counter);
        self.order_counter += 1;
        o
    }

    // =========================================================================
    // Utility: Get mutable engine or chain
    // =========================================================================

    fn engine_mut(&mut self, chain_id: MockChainId) -> &mut CoreProverEngine {
        self.engines
            .get_mut(&chain_id)
            .expect("Engine must exist for chain")
    }

    fn chain_mut(&mut self, chain_id: MockChainId) -> &mut MockChain {
        self.chains
            .get_mut(&chain_id)
            .expect("Chain must exist for chain")
    }

    // =========================================================================
    // BUYER → Commit
    //
    // Automatically:
    //   - Generates a deterministic session + order ID
    //   - Allocates buyer commit TXID via MockChain
    //   - Pushes commit into engine on SELLER'S chain
    //
    // Because the SELLER chooses the settlement chain.
    // =========================================================================

    pub fn commit(
        &mut self,
        buyer_chain: MockChainId,
        seller_chain: MockChainId,
        amount: u64,
        label: &str,
    ) -> HarnessOrderId {
        self.chain(buyer_chain);
        self.chain(seller_chain);

        let session = self.next_session_id(label);
        let order = self.next_order_id(label);

        // Deterministic buyer commit TXID
        let commit_tx = {
            let chain = self.chain_mut(buyer_chain);
            chain.generate_txid()
        };

        // Convert order ID to engine format
        let mut oid_bytes = [0u8; 32];
        {
            let s = order.as_str().as_bytes();
            let copy_len = s.len().min(32);
            oid_bytes[..copy_len].copy_from_slice(&s[..copy_len]);
        }

        // Send commit into seller's engine
        let engine = self.engine_mut(seller_chain);
        let _ = engine.buyer_commit(
            format!("buyer@chain{}", buyer_chain.0),
            format!("seller@chain{}", seller_chain.0),
            amount,
            crate::payment_profiles::TEST_PROFILE.clone(), // placeholder test profile
            buyer_chain.0,
            commit_tx.txid.clone(),
        );

        order
    }

    // =========================================================================
    // SELLER → Accept
    // =========================================================================

    pub fn accept(&mut self, order: &HarnessOrderId, seller_chain: MockChainId) {
        let tx = {
            let chain = self.chain_mut(seller_chain);
            chain.generate_txid()
        };

        let oid = Self::encode_order_id(order);

        let engine = self.engine_mut(seller_chain);
        let _ = engine.seller_accept(&oid, tx.txid);
    }

    // =========================================================================
    // SELLER → Fulfill
    // =========================================================================

    pub fn fulfill(&mut self, order: &HarnessOrderId, seller_chain: MockChainId) {
        let tx = {
            let chain = self.chain_mut(seller_chain);
            chain.generate_txid()
        };

        let oid = Self::encode_order_id(order);
        let engine = self.engine_mut(seller_chain);
        let _ = engine.seller_fulfill(&oid, tx.txid);
    }

    // =========================================================================
    // SELLER → Claim
    // =========================================================================

    pub fn claim(&mut self, order: &HarnessOrderId, seller_chain: MockChainId) {
        let tx = {
            let chain = self.chain_mut(seller_chain);
            chain.generate_txid()
        };

        let oid = Self::encode_order_id(order);
        let engine = self.engine_mut(seller_chain);
        let _ = engine.seller_claim(&oid, tx.txid);
    }

    // =========================================================================
    // BUYER → Withdraw
    // =========================================================================

    pub fn withdraw(&mut self, order: &HarnessOrderId, seller_chain: MockChainId) {
        let tx = {
            let chain = self.chain_mut(seller_chain);
            chain.generate_txid()
        };

        let oid = Self::encode_order_id(order);
        let engine = self.engine_mut(seller_chain);
        let _ = engine.buyer_withdraw(&oid, Some(tx.txid));
    }

    // =========================================================================
    // Receipt Retrieval
    // =========================================================================

    pub fn receipt(
        &mut self,
        order: &HarnessOrderId,
        seller_chain: MockChainId,
    ) -> Option<CoreProverReceipt> {
        let oid = Self::encode_order_id(order);
        let engine = self.engines.get(&seller_chain)?;
        engine.get_receipt(&oid).cloned()
    }

    // =========================================================================
    // INTERNAL: order-id → [u8; 32]
    // =========================================================================

    fn encode_order_id(order: &HarnessOrderId) -> [u8; 32] {
        let mut id = [0u8; 32];
        let s = order.as_str().as_bytes();
        let n = s.len().min(32);
        id[..n].copy_from_slice(&s[..n]);
        id
    }
}

// ============================================================================
// Tests (smoke test for TestContext)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_flow() {
        let mut ctx = TestContext::new();

        let buyer_chain = ctx.chain(1);
        let seller_chain = ctx.chain(369);

        let order = ctx.commit(buyer_chain, seller_chain, 1000, "pizza");

        ctx.advance_time(5);
        ctx.accept(&order, seller_chain);

        ctx.advance_time(10);
        ctx.fulfill(&order, seller_chain);

        ctx.advance_time(20);
        ctx.claim(&order, seller_chain);

        let receipt = ctx.receipt(&order, seller_chain)
            .expect("receipt must exist");

        assert_eq!(receipt.order_amount, 1000);
        assert!(receipt.seller_was_paid());
    }
}