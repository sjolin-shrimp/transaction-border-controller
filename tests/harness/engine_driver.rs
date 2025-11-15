// ============================================================================
// CoreProver v0.3 Test Harness
// File: tests/harness/engine_driver.rs
//
// Purpose:
//   Canonical wrapper around the CoreProver engine that:
//     * Translates harness types to engine types
//     * Converts String errors to typed EngineError
//     * Integrates MockChain for TXID generation
//     * Coordinates time advancement with TimeController
//     * Records all operations to trace system
//     * Provides clean scenario-friendly API
//
// Architecture:
//   This is Layer 2 of the integration (from reconciliation report):
//     - Layer 1: types.rs (bridge types)
//     - Layer 2: engine_driver.rs (operation wrapper) ← THIS FILE
//     - Layer 3: context.rs (test coordination)
//
// Rules:
//   - All engine operations go through this driver
//   - No direct engine calls from scenarios
//   - All errors are typed (EngineError)
//   - All operations traced
//   - Time always synchronized
//
// ============================================================================

use crate::harness::types::*;
use crate::harness::trace::{Tracer, TraceEvent};
use crate::harness::time::TimeController;
use crate::harness::mock_chain::MockChain;

use coreprover_service::engine::CoreProverEngine;
use coreprover_types_v03::{
    PaymentProfile, TimingProfile, EscrowState, CoreProverReceipt,
};

use std::time::Duration;

// ============================================================================
// EngineDriver - Canonical wrapper around CoreProverEngine
// ============================================================================

pub struct EngineDriver {
    /// The canonical CoreProver engine
    engine: CoreProverEngine,
    
    /// Time controller (harness clock)
    time: TimeController,
    
    /// Mock blockchain for TXID generation
    mock_chain: MockChain,
    
    /// Event tracer
    tracer: Tracer,
    
    /// Driver configuration
    config: DriverConfig,
}

// ============================================================================
// DriverConfig - Configuration for driver behavior
// ============================================================================

#[derive(Debug, Clone)]
pub struct DriverConfig {
    /// Chain ID for this test environment
    pub chain_id: u64,
    
    /// Block interval in seconds (e.g., 12s for Ethereum)
    pub block_interval_secs: u64,
    
    /// Genesis timestamp (unix)
    pub genesis_unix: u64,
    
    /// Default payment profile if scenarios don't specify one
    pub default_profile: PaymentProfile,
}

impl Default for DriverConfig {
    fn default() -> Self {
        Self {
            chain_id: 1, // Ethereum mainnet
            block_interval_secs: 12,
            genesis_unix: 1700000000, // Nov 2023
            default_profile: PaymentProfile::default(),
        }
    }
}

// ============================================================================
// EngineDriver Implementation
// ============================================================================

impl EngineDriver {
    /// Create a new engine driver with given configuration
    pub fn new(config: DriverConfig) -> Self {
        let engine = CoreProverEngine::new(
            config.chain_id,
            config.block_interval_secs,
            config.genesis_unix,
        );
        
        let time = TimeController::new(config.genesis_unix, config.block_interval_secs);
        let mock_chain = MockChain::new(config.chain_id);
        let tracer = Tracer::new();
        
        Self {
            engine,
            time,
            mock_chain,
            tracer,
            config,
        }
    }
    
    // ========================================================================
    // BUYER → Commit
    // ========================================================================
    
    /// Buyer commits funds to escrow
    pub fn buyer_commit(&mut self, params: CommitParams) -> Result<HarnessOrderId, EngineError> {
        let commit_txid = self.mock_chain.generate_txid("commit");
        
        let profile = params.profile.unwrap_or_else(|| self.default_profile());
        
        let order_id_bytes = self.engine.buyer_commit(
            params.buyer.clone(),
            params.seller.clone(),
            params.amount,
            profile.clone(),
            self.mock_chain.chain_id(),
            commit_txid.clone().into_string(),
        ).map_err(EngineError::from)?;
        
        let order_id = HarnessOrderId::from_bytes(order_id_bytes);
        
        self.tracer.record(TraceEvent::BuyerCommitted {
            order_id: order_id.clone(),
            buyer: params.buyer.clone(),
            seller: params.seller.clone(),
            amount: params.amount,
            txid: commit_txid.clone(),
            timestamp: self.time.current_triple(),
        });
        
        Ok(order_id)
    }
    
    // ========================================================================
    // SELLER → Accept
    // ========================================================================
    
    pub fn seller_accept(
        &mut self, 
        order_id: HarnessOrderId, 
        txid: Option<TxId>
    ) -> Result<(), EngineError> {
        let order_id_bytes = order_id.to_bytes()
            .map_err(|e| EngineError::InvalidOperation(e))?;
        
        let accept_txid = txid.unwrap_or_else(|| self.mock_chain.generate_txid("accept"));
        
        let state_before = self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        self.engine.seller_accept(&order_id_bytes, accept_txid.clone().into_string())
            .map_err(EngineError::from)?;
        
        let state_after = self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        self.tracer.record(TraceEvent::SellerAccepted {
            order_id: order_id.clone(),
            txid: accept_txid.clone(),
            timestamp: self.time.current_triple(),
        });
        
        self.tracer.record(TraceEvent::StateTransition {
            order_id,
            from: state_before,
            to: state_after,
            timestamp: self.time.current_triple(),
        });
        
        Ok(())
    }
    
    // ========================================================================
    // SELLER → Fulfill
    // ========================================================================
    
    pub fn seller_fulfill(
        &mut self,
        order_id: HarnessOrderId,
        txid: Option<TxId>
    ) -> Result<(), EngineError> {
        let order_id_bytes = order_id.to_bytes()
            .map_err(|e| EngineError::InvalidOperation(e))?;
        
        let fulfill_txid = txid.unwrap_or_else(|| self.mock_chain.generate_txid("fulfill"));
        
        let state_before = self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        self.engine.seller_fulfill(&order_id_bytes, fulfill_txid.clone().into_string())
            .map_err(EngineError::from)?;
        
        let state_after = self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        let is_late = state_after == EscrowState::FulfillmentExpired;
        
        self.tracer.record(TraceEvent::SellerFulfilled {
            order_id: order_id.clone(),
            txid: fulfill_txid.clone(),
            late: is_late,
            timestamp: self.time.current_triple(),
        });
        
        self.tracer.record(TraceEvent::StateTransition {
            order_id: order_id.clone(),
            from: state_before,
            to: state_after,
            timestamp: self.time.current_triple(),
        });
        
        self.tracer.record(TraceEvent::ReceiptCreated {
            order_id,
            timestamp: self.time.current_triple(),
        });
        
        Ok(())
    }
    
    // ========================================================================
    // SELLER → Claim
    // ========================================================================
    
    pub fn seller_claim(
        &mut self,
        order_id: HarnessOrderId,
        txid: Option<TxId>
    ) -> Result<u64, EngineError> {
        let order_id_bytes = order_id.to_bytes()
            .map_err(|e| EngineError::InvalidOperation(e))?;
        
        let claim_txid = txid.unwrap_or_else(|| self.mock_chain.generate_txid("claim"));
        
        let state_before = self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        let amount = self.engine.seller_claim(&order_id_bytes, claim_txid.clone().into_string())
            .map_err(EngineError::from)?;
        
        let state_after = self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        self.tracer.record(TraceEvent::SellerClaimed {
            order_id: order_id.clone(),
            amount,
            txid: claim_txid.clone(),
            timestamp: self.time.current_triple(),
        });
        
        self.tracer.record(TraceEvent::StateTransition {
            order_id: order_id.clone(),
            from: state_before,
            to: state_after,
            timestamp: self.time.current_triple(),
        });
        
        self.tracer.record(TraceEvent::SettlementFinalized {
            order_id,
            amount,
            settlement_type: "claim".into(),
            timestamp: self.time.current_triple(),
        });
        
        Ok(amount)
    }
    
    // ========================================================================
    // SELLER → Refund
    // ========================================================================
    
    pub fn seller_refund(
        &mut self,
        order_id: HarnessOrderId,
        txid: Option<TxId>
    ) -> Result<u64, EngineError> {
        let order_id_bytes = order_id.to_bytes()
            .map_err(|e| EngineError::InvalidOperation(e))?;
        
        let refund_txid = txid.unwrap_or_else(|| self.mock_chain.generate_txid("refund"));
        
        let state_before = self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        let amount = self.engine.seller_refund(&order_id_bytes, refund_txid.clone().into_string())
            .map_err(EngineError::from)?;
        
        let state_after = self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        self.tracer.record(TraceEvent::SellerRefunded {
            order_id: order_id.clone(),
            amount,
            txid: refund_txid.clone(),
            timestamp: self.time.current_triple(),
        });
        
        self.tracer.record(TraceEvent::StateTransition {
            order_id: order_id.clone(),
            from: state_before,
            to: state_after,
            timestamp: self.time.current_triple(),
        });
        
        self.tracer.record(TraceEvent::SettlementFinalized {
            order_id,
            amount,
            settlement_type: "refund".into(),
            timestamp: self.time.current_triple(),
        });
        
        Ok(amount)
    }
    
    // ========================================================================
    // BUYER → Withdraw
    // ========================================================================
    
    pub fn buyer_withdraw(
        &mut self,
        order_id: HarnessOrderId,
        txid: Option<TxId>
    ) -> Result<u64, EngineError> {
        let order_id_bytes = order_id.to_bytes()
            .map_err(|e| EngineError::InvalidOperation(e))?;
        
        let withdraw_txid = txid.map(|t| t.into_string());
        
        let state_before = self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        let amount = self.engine.buyer_withdraw(&order_id_bytes, withdraw_txid.clone())
            .map_err(EngineError::from)?;
        
        let state_after = self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        self.tracer.record(TraceEvent::BuyerWithdrew {
            order_id: order_id.clone(),
            amount,
            txid: withdraw_txid.map(TxId::new),
            timestamp: self.time.current_triple(),
        });
        
        self.tracer.record(TraceEvent::StateTransition {
            order_id,
            from: state_before,
            to: state_after,
            timestamp: self.time.current_triple(),
        });
        
        Ok(amount)
    }
    
    // ========================================================================
    // TIMED RELEASE → Auto-claim
    // ========================================================================
    
    pub fn timed_release(&mut self, order_id: HarnessOrderId) -> Result<u64, EngineError> {
        let order_id_bytes = order_id.to_bytes()
            .map_err(|e| EngineError::InvalidOperation(e))?;
        
        let state_before = self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        let amount = self.engine.timed_release(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        let state_after = self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)?;
        
        self.tracer.record(TraceEvent::TimedRelease {
            order_id: order_id.clone(),
            amount,
            timestamp: self.time.current_triple(),
        });
        
        self.tracer.record(TraceEvent::StateTransition {
            order_id: order_id.clone(),
            from: state_before,
            to: state_after,
            timestamp: self.time.current_triple(),
        });
        
        self.tracer.record(TraceEvent::SettlementFinalized {
            order_id,
            amount,
            settlement_type: "timed_release".into(),
            timestamp: self.time.current_triple(),
        });
        
        Ok(amount)
    }
    
    // ========================================================================
    // Time Control
    // ========================================================================
    
    pub fn advance_time(&mut self, duration: Duration) {
        let secs = duration.as_secs();
        
        self.time.advance(secs);
        
        self.engine.advance_time(secs);
        
        let blocks = secs / self.config.block_interval_secs;
        self.mock_chain.advance_blocks(blocks);
        
        self.tracer.record(TraceEvent::TimeAdvanced {
            secs,
            new_block: self.mock_chain.current_block(),
            timestamp: self.time.current_triple(),
        });
        
        debug_assert_eq!(
            self.time.current_mono(),
            self.engine.current_mono,
            "Time desync between harness and engine"
        );
        debug_assert_eq!(
            self.mock_chain.current_block(),
            self.engine.current_block_height,
            "Block height desync between mock chain and engine"
        );
    }
    
    pub fn advance_blocks(&mut self, blocks: u64) {
        let secs = blocks * self.config.block_interval_secs;
        self.advance_time(Duration::from_secs(secs));
    }
    
    pub fn current_time(&self) -> HarnessTimestamp {
        self.time.current_timestamp()
    }
    
    pub fn current_block(&self) -> u64 {
        self.mock_chain.current_block()
    }
    
    // ========================================================================
    // State Queries
    // ========================================================================
    
    pub fn get_state(&self, order_id: &HarnessOrderId) -> Result<EscrowState, EngineError> {
        let order_id_bytes = order_id.to_bytes()
            .map_err(|e| EngineError::InvalidOperation(e))?;
        
        self.engine.get_state(&order_id_bytes)
            .map_err(EngineError::from)
    }
    
    pub fn get_receipt(&self, order_id: &HarnessOrderId) -> Result<Option<CoreProverReceipt>, EngineError> {
        let order_id_bytes = order_id.to_bytes()
            .map_err(|e| EngineError::InvalidOperation(e))?;
        
        Ok(self.engine.get_receipt(&order_id_bytes)
            .map(|meta| meta.to_public_receipt()))
    }
    
    // ========================================================================
    // Profile Factories
    // ========================================================================
    
    pub fn default_profile(&self) -> PaymentProfile {
        self.config.default_profile.clone()
    }
    
    pub fn profile_for(&self, scenario: &str) -> PaymentProfile {
        match scenario {
            "pizza" | "pizza_standard" | "pizza_delivery" => self.pizza_profile(),
            "digital" | "digital_goods" => self.digital_goods_profile(),
            "swap" | "atomic_swap" => self.swap_profile(),
            _ => self.default_profile(),
        }
    }
    
    fn pizza_profile(&self) -> PaymentProfile {
        PaymentProfile {
            timing: TimingProfile {
                acceptance_window_secs: 300,
                fulfillment_window_secs: 3600,
                claim_window_secs: 86400,
            },
            enables_late_discount: true,
            late_discount_pct: 15,
            discount_expiration_days: 7,
            allows_timed_release: true,
        }
    }
    
    fn digital_goods_profile(&self) -> PaymentProfile {
        PaymentProfile {
            timing: TimingProfile {
                acceptance_window_secs: 60,
                fulfillment_window_secs: 300,
                claim_window_secs: 3600,
            },
            enables_late_discount: false,
            late_discount_pct: 0,
            discount_expiration_days: 0,
            allows_timed_release: true,
        }
    }
    
    fn swap_profile(&self) -> PaymentProfile {
        PaymentProfile {
            timing: TimingProfile {
                acceptance_window_secs: 600,
                fulfillment_window_secs: 1800,
                claim_window_secs: 7200,
            },
            enables_late_discount: false,
            late_discount_pct: 0,
            discount_expiration_days: 0,
            allows_timed_release: false,
        }
    }
    
    // ========================================================================
    // Trace Access
    // ========================================================================
    
    pub fn get_trace(&self) -> &[TraceEvent] {
        self.tracer.events()
    }
    
    pub fn clear_trace(&mut self) {
        self.tracer.clear();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn driver_initialization() {
        let config = DriverConfig::default();
        let driver = EngineDriver::new(config);
        
        assert_eq!(driver.current_block(), 1);
        assert_eq!(driver.time.current_mono(), 0);
    }
    
    #[test]
    fn time_advancement_sync() {
        let config = DriverConfig::default();
        let mut driver = EngineDriver::new(config);
        
        driver.advance_time(Duration::from_secs(600));
        
        assert_eq!(driver.current_block(), 51);
        assert_eq!(driver.time.current_mono(), 600);
    }
    
    #[test]
    fn profile_factory() {
        let config = DriverConfig::default();
        let driver = EngineDriver::new(config);
        
        let pizza = driver.profile_for("pizza");
        assert_eq!(pizza.timing.fulfillment_window_secs, 3600);
        assert!(pizza.enables_late_discount);
        
        let digital = driver.profile_for("digital");
        assert_eq!(digital.timing.fulfillment_window_secs, 300);
        assert!(!digital.enables_late_discount);
    }
}