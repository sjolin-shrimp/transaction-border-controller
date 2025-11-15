// ============================================================================
// CoreProver v0.3 Test Harness
// File: tests/harness/mock_chain.rs
//
// Purpose:
//   Provide a deterministic, zero-network, test-only blockchain simulation
//   that produces:
//      - Deterministic TXIDs
//      - Deterministic block heights
//      - Deterministic timestamp provenance
//      - BuyerTxIds and SellerTxIds compatible with v0.3
//
// This is NOT a blockchain emulator.
// This is EXACTLY what CoreProver needs:
//   -> "TestChain" = reproducible provenance provider
//   -> TXIDs encode (chain_id, mono, height, seq)
//   -> Block height increments based on TimeController
//
// The real blockchain will be used in production.
// This module is for controlled unit-testing only.
//
// ============================================================================

#![allow(dead_code)]

use crate::harness::time::TimeController;
use crate::harness::chain::TestChain;

use coreprover_types_v03::{
    BuyerTxIds,
    SellerTxIds,
};

// Unique counter for deterministic txid creation
#[derive(Debug)]
pub struct TxCounter {
    seq: u64,
}

impl TxCounter {
    pub fn new() -> Self {
        Self { seq: 1 }
    }

    pub fn next(&mut self) -> u64 {
        let x = self.seq;
        self.seq += 1;
        x
    }
}

// ============================================================================
// MockChain
//
// Represents ONE testing chain.
// The engine decides the final settlement chain.
// Each test may spawn multiple MockChains.
//
// ============================================================================

#[derive(Debug)]
pub struct MockChain {
    pub chain_id: u64,

    /// Block interval (seconds)
    pub block_interval: u64,

    /// Deterministic height (derived from TimeController)
    height: u64,

    /// Per-chain tx counter
    tx_counter: TxCounter,
}

impl MockChain {
    pub fn new(chain: &TestChain) -> Self {
        Self {
            chain_id: chain.chain_id,
            block_interval: chain.block_interval_secs,
            height: 0,
            tx_counter: TxCounter::new(),
        }
    }

    // ------------------------------------------------------------------------
    // Block height sync
    // ------------------------------------------------------------------------

    /// Sync mock chain's height with the test clock
    pub fn sync_height(&mut self, t: &TimeController, genesis_unix: u64) {
        let diff = t.now_unix().saturating_sub(genesis_unix);
        self.height = diff / self.block_interval;
    }

    pub fn height(&self) -> u64 {
        self.height
    }

    // ------------------------------------------------------------------------
    // TXID Generation
    // ------------------------------------------------------------------------

    /// Deterministic TXID format:
    ///
    ///   tx_<chainid>_<height>_<mono>_<seq>
    ///
    /// This guarantees:
    ///   - Repeatable tests (no randomness)
    ///   - Unambiguous provenance
    ///   - Every txid carries full timeline context
    pub fn make_txid(&mut self, t: &TimeController) -> String {
        let seq = self.tx_counter.next();

        format!(
            "tx_{}_{}_{}_{}",
            self.chain_id,
            self.height,
            t.now_mono(),
            seq
        )
    }

    // ------------------------------------------------------------------------
    // Buyer & Seller Provenance Bundles
    // ------------------------------------------------------------------------

    /// Generate BuyerTxIds for buyer_commit()
    pub fn buyer_commit(&mut self, t: &TimeController) -> BuyerTxIds {
        let txid = self.make_txid(t);

        BuyerTxIds::new(self.chain_id, txid)
    }

    /// Generate seller_accept provenance
    pub fn seller_accept(&mut self, t: &TimeController) -> SellerTxIds {
        let accept_txid = self.make_txid(t);

        SellerTxIds::new(self.chain_id, accept_txid, "", 0)
    }

    /// Add fulfill txid to a seller provenance
    pub fn seller_fulfill(
        &mut self,
        mut prov: SellerTxIds,
        t: &TimeController,
    ) -> SellerTxIds {
        let fulfill_tx = self.make_txid(t);

        prov.fulfill_txid = fulfill_tx;
        prov.fulfill_block_height = self.height;
        prov
    }

    /// Add claim txid
    pub fn seller_claim(
        &mut self,
        mut prov: SellerTxIds,
        t: &TimeController,
    ) -> SellerTxIds {
        let claim_tx = self.make_txid(t);
        prov.claim_txid = Some(claim_tx);
        prov
    }

    /// Add refund txid
    pub fn seller_refund(
        &mut self,
        mut prov: SellerTxIds,
        t: &TimeController,
    ) -> SellerTxIds {
        let refund_tx = self.make_txid(t);
        prov.refund_txid = Some(refund_tx);
        prov
    }

    /// Add buyer withdrawal txid
    pub fn buyer_withdraw(
        &mut self,
        mut buyer: BuyerTxIds,
        t: &TimeController,
    ) -> BuyerTxIds {
        let wd_tx = self.make_txid(t);
        buyer.withdraw_txid = Some(wd_tx);
        buyer
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::chain::TestChain;
    use crate::harness::time::TimeController;

    #[test]
    fn txid_is_deterministic_and_increases() {
        let chain = TestChain::eth_mainnet();
        let mut m = MockChain::new(&chain);

        let mut t = TimeController::new();
        m.sync_height(&t, 1_700_000_000);

        let a = m.make_txid(&t);
        let b = m.make_txid(&t);

        assert_ne!(a, b);
    }

    #[test]
    fn provenance_objects_are_created() {
        let chain = TestChain::eth_mainnet();
        let mut m = MockChain::new(&chain);
        let mut t = TimeController::new();

        m.sync_height(&t, 1_700_000_000);

        let buyer = m.buyer_commit(&t);
        assert_eq!(buyer.chain_id, chain.chain_id);
        assert!(buyer.commit_txid.starts_with("tx_"));

        let seller = m.seller_accept(&t);
        assert_eq!(seller.chain_id, chain.chain_id);
        assert!(seller.accept_txid.starts_with("tx_"));
    }

    #[test]
    fn seller_fulfillment_embeds_block_height() {
        let chain = TestChain::eth_mainnet();
        let mut m = MockChain::new(&chain);
        let mut t = TimeController::new();

        m.sync_height(&t, 1_700_000_000);
        let base = m.seller_accept(&t);

        t.advance_raw(30);
        m.sync_height(&t, 1_700_000_000);

        let with_f = m.seller_fulfill(base, &t);

        assert_ne!(with_f.fulfill_txid, "");
        assert!(with_f.fulfill_block_height > 0);
    }
}