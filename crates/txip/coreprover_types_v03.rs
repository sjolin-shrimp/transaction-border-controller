// crates/tbc-gateway/src/txip/coreprover_types_v03.rs
// FINAL - CoreProver v0.3 Canonical Receipt and Escrow Types
//
// This module defines the ONLY valid CoreProver v0.3 receipt schema
// and escrow state tracking with full TXID provenance.

use serde::{Deserialize, Serialize};

use super::blockchain_types_v03::{BuyerTxIds, ChainId, SellerTxIds, TxIdProvenance};
use super::timestamp_types_v03::TripleTimestamp;

/// Canonical CoreProver v0.3 Receipt
/// This is the ONLY valid receipt format
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoreProverReceipt {
    /// Session identifier linking TxIP/TGP messages
    pub session_id: String,
    
    /// Order amount in smallest unit (wei, satoshi, etc)
    pub order_amount: u128,
    
    /// When order was fulfilled (triple timestamp)
    pub fulfillment_mono: u64,
    pub fulfillment_unix: u64,
    pub fulfillment_iso: String,
    
    /// When payment was settled (triple timestamp)
    pub settlement_mono: u64,
    pub settlement_unix: u64,
    pub settlement_iso: String,
    
    /// Discount percentage if late fulfillment (0-100)
    pub discount_pct: u8,
    
    /// When discount expires (unix timestamp)
    pub discount_expiration_unix: u64,
    
    /// Buyer blockchain provenance
    pub buyer_chain_id: ChainId,
    pub buyer_commit_txid: String,
    
    /// Seller blockchain provenance
    pub seller_chain_id: ChainId,
    pub seller_accept_txid: String,
    pub seller_fulfill_txid: String,
    
    /// Settlement outcome (exactly one must be present)
    pub seller_claim_txid: Option<String>,
    pub seller_refund_txid: Option<String>,
    
    /// Optional buyer withdrawal
    pub buyer_withdraw_txid: Option<String>,
    
    /// Block height where seller fulfillment occurred
    pub seller_block_height: u64,
}

impl CoreProverReceipt {
    /// Create a new receipt with all required fields
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: String,
        order_amount: u128,
        fulfillment: TripleTimestamp,
        settlement: TripleTimestamp,
        discount_pct: u8,
        discount_expiration_unix: u64,
        buyer_chain_id: ChainId,
        buyer_commit_txid: String,
        seller_chain_id: ChainId,
        seller_accept_txid: String,
        seller_fulfill_txid: String,
        seller_block_height: u64,
    ) -> Self {
        Self {
            session_id,
            order_amount,
            fulfillment_mono: fulfillment.mono,
            fulfillment_unix: fulfillment.unix,
            fulfillment_iso: fulfillment.iso,
            settlement_mono: settlement.mono,
            settlement_unix: settlement.unix,
            settlement_iso: settlement.iso,
            discount_pct,
            discount_expiration_unix,
            buyer_chain_id,
            buyer_commit_txid,
            seller_chain_id,
            seller_accept_txid,
            seller_fulfill_txid,
            seller_claim_txid: None,
            seller_refund_txid: None,
            buyer_withdraw_txid: None,
            seller_block_height,
        }
    }

    /// Add seller claim transaction
    pub fn with_seller_claim(mut self, claim_txid: String) -> Self {
        self.seller_claim_txid = Some(claim_txid);
        self
    }

    /// Add seller refund transaction
    pub fn with_seller_refund(mut self, refund_txid: String) -> Self {
        self.seller_refund_txid = Some(refund_txid);
        self
    }

    /// Add buyer withdraw transaction
    pub fn with_buyer_withdraw(mut self, withdraw_txid: String) -> Self {
        self.buyer_withdraw_txid = Some(withdraw_txid);
        self
    }

    /// Validate receipt has all required fields
    pub fn validate(&self) -> Result<(), String> {
        if self.session_id.is_empty() {
            return Err("session_id is required".to_string());
        }

        if self.order_amount == 0 {
            return Err("order_amount must be > 0".to_string());
        }

        if self.buyer_commit_txid.is_empty() {
            return Err("buyer_commit_txid is required".to_string());
        }

        if self.seller_accept_txid.is_empty() {
            return Err("seller_accept_txid is required".to_string());
        }

        if self.seller_fulfill_txid.is_empty() {
            return Err("seller_fulfill_txid is required".to_string());
        }

        // Must have exactly one settlement outcome
        match (&self.seller_claim_txid, &self.seller_refund_txid) {
            (Some(_), None) | (None, Some(_)) => Ok(()),
            (Some(_), Some(_)) => {
                Err("Cannot have both seller_claim_txid and seller_refund_txid".to_string())
            }
            (None, None) => {
                Err("Must have either seller_claim_txid or seller_refund_txid".to_string())
            }
        }
    }

    /// Check if this receipt includes a discount
    pub fn has_discount(&self) -> bool {
        self.discount_pct > 0
    }

    /// Check if discount is still valid
    pub fn is_discount_valid(&self, current_unix: u64) -> bool {
        self.has_discount() && current_unix < self.discount_expiration_unix
    }

    /// Check if seller was paid (vs refunded)
    pub fn seller_was_paid(&self) -> bool {
        self.seller_claim_txid.is_some()
    }

    /// Check if this was a cross-chain transaction
    pub fn is_cross_chain(&self) -> bool {
        self.buyer_chain_id != self.seller_chain_id
    }

    /// Get fulfillment timestamp as TripleTimestamp
    pub fn fulfillment_timestamp(&self) -> TripleTimestamp {
        TripleTimestamp::new(
            self.fulfillment_mono,
            self.fulfillment_unix,
            self.fulfillment_iso.clone(),
        )
    }

    /// Get settlement timestamp as TripleTimestamp
    pub fn settlement_timestamp(&self) -> TripleTimestamp {
        TripleTimestamp::new(
            self.settlement_mono,
            self.settlement_unix,
            self.settlement_iso.clone(),
        )
    }
}

/// Escrow state for CoreProver v0.3
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EscrowState {
    /// No escrow created yet
    None,
    
    /// Buyer has committed payment
    BuyerCommitted,
    
    /// Seller has accepted (signature or counter-escrow)
    SellerAccepted,
    
    /// Seller has fulfilled the order
    SellerFulfilled,
    
    /// Fulfillment window expired without fulfillment
    FulfillmentExpired,
    
    /// Seller has claimed payment
    SellerClaimed,
    
    /// Buyer has withdrawn (timeout or cancellation)
    BuyerClaimed,
    
    /// Acceptance window expired
    Expired,
}

/// Escrow record with full state and TXID tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscrowRecord {
    /// Order/escrow identifier
    pub order_id: String,
    
    /// Session identifier
    pub session_id: String,
    
    /// Current state
    pub state: EscrowState,
    
    /// Order amount
    pub amount: u128,
    
    /// Asset token address or symbol
    pub asset: String,
    
    /// When escrow was created
    pub created_at: TripleTimestamp,
    
    /// Acceptance deadline
    pub acceptance_deadline: TripleTimestamp,
    
    /// Fulfillment deadline (after acceptance)
    pub fulfillment_deadline: Option<TripleTimestamp>,
    
    /// Claim deadline (after fulfillment)
    pub claim_deadline: Option<TripleTimestamp>,
    
    /// Complete TXID provenance (populated as actions occur)
    pub buyer_txids: Option<BuyerTxIds>,
    pub seller_txids: Option<SellerTxIds>,
    
    /// Late fulfillment flag
    pub late_fulfilled: bool,
    
    /// Discount percentage if late
    pub discount_pct: u8,
}

impl EscrowRecord {
    /// Create a new escrow record in BuyerCommitted state
    pub fn new(
        order_id: String,
        session_id: String,
        amount: u128,
        asset: String,
        created_at: TripleTimestamp,
        acceptance_deadline: TripleTimestamp,
        buyer_chain_id: ChainId,
        buyer_commit_txid: String,
    ) -> Self {
        Self {
            order_id,
            session_id,
            state: EscrowState::BuyerCommitted,
            amount,
            asset,
            created_at,
            acceptance_deadline,
            fulfillment_deadline: None,
            claim_deadline: None,
            buyer_txids: Some(BuyerTxIds::new(buyer_chain_id, buyer_commit_txid)),
            seller_txids: None,
            late_fulfilled: false,
            discount_pct: 0,
        }
    }

    /// Transition to SellerAccepted state
    pub fn seller_accept(
        &mut self,
        seller_chain_id: ChainId,
        seller_accept_txid: String,
        fulfillment_deadline: TripleTimestamp,
        claim_window_seconds: u64,
    ) -> Result<(), String> {
        if self.state != EscrowState::BuyerCommitted {
            return Err(format!("Cannot accept from state: {:?}", self.state));
        }

        self.state = EscrowState::SellerAccepted;
        self.fulfillment_deadline = Some(fulfillment_deadline.clone());
        
        // Calculate claim deadline
        let claim_deadline_mono = fulfillment_deadline.mono + claim_window_seconds;
        let claim_deadline_unix = fulfillment_deadline.unix + claim_window_seconds;
        let claim_deadline = TripleTimestamp::new(
            claim_deadline_mono,
            claim_deadline_unix,
            format!("{}+{}s", fulfillment_deadline.iso, claim_window_seconds),
        );
        self.claim_deadline = Some(claim_deadline);

        // Initialize seller TXIDs (fulfill will be added later)
        self.seller_txids = Some(SellerTxIds::new(
            seller_chain_id,
            seller_accept_txid,
            String::new(), // Will be set on fulfill
            0,             // Will be set on fulfill
        ));

        Ok(())
    }

    /// Transition to SellerFulfilled state
    pub fn seller_fulfill(
        &mut self,
        seller_fulfill_txid: String,
        seller_block_height: u64,
        fulfillment_time: TripleTimestamp,
    ) -> Result<(), String> {
        if self.state != EscrowState::SellerAccepted
            && self.state != EscrowState::FulfillmentExpired
        {
            return Err(format!("Cannot fulfill from state: {:?}", self.state));
        }

        // Check if late
        if let Some(ref deadline) = self.fulfillment_deadline {
            if fulfillment_time.mono > deadline.mono {
                self.late_fulfilled = true;
                self.discount_pct = 10; // Standard 10% late discount
            }
        }

        self.state = EscrowState::SellerFulfilled;

        // Update seller TXIDs with fulfill info
        if let Some(ref mut seller_txids) = self.seller_txids {
            seller_txids.fulfill_txid = seller_fulfill_txid;
            seller_txids.fulfill_block_height = seller_block_height;
        }

        Ok(())
    }

    /// Transition to SellerClaimed state
    pub fn seller_claim(
        &mut self,
        seller_claim_txid: String,
    ) -> Result<(), String> {
        if self.state != EscrowState::SellerFulfilled {
            return Err(format!("Cannot claim from state: {:?}", self.state));
        }

        self.state = EscrowState::SellerClaimed;

        // Add claim TXID
        if let Some(ref mut seller_txids) = self.seller_txids {
            seller_txids.claim_txid = Some(seller_claim_txid);
        }

        Ok(())
    }

    /// Generate receipt from completed escrow
    pub fn to_receipt(
        &self,
        fulfillment_time: TripleTimestamp,
        settlement_time: TripleTimestamp,
        discount_expiration_unix: u64,
    ) -> Result<CoreProverReceipt, String> {
        if self.state != EscrowState::SellerClaimed {
            return Err("Can only generate receipt from SellerClaimed state".to_string());
        }

        let buyer = self.buyer_txids.as_ref()
            .ok_or("Missing buyer TXIDs")?;
        let seller = self.seller_txids.as_ref()
            .ok_or("Missing seller TXIDs")?;

        let mut receipt = CoreProverReceipt::new(
            self.session_id.clone(),
            self.amount,
            fulfillment_time,
            settlement_time,
            self.discount_pct,
            discount_expiration_unix,
            buyer.chain_id,
            buyer.commit_txid.clone(),
            seller.chain_id,
            seller.accept_txid.clone(),
            seller.fulfill_txid.clone(),
            seller.fulfill_block_height,
        );

        if let Some(ref claim_txid) = seller.claim_txid {
            receipt = receipt.with_seller_claim(claim_txid.clone());
        }

        if let Some(ref withdraw_txid) = buyer.withdraw_txid {
            receipt = receipt.with_buyer_withdraw(withdraw_txid.clone());
        }

        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_timestamp() -> TripleTimestamp {
        TripleTimestamp::new(
            1000,
            1731600000,
            "2024-11-14T12:00:00Z".to_string(),
        )
    }

    #[test]
    fn test_receipt_validation() {
        let ts = create_test_timestamp();
        let receipt = CoreProverReceipt::new(
            "sess-123".to_string(),
            30000000,
            ts.clone(),
            ts.clone(),
            0,
            0,
            1,
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            369,
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            12345,
        )
        .with_seller_claim("0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string());

        assert!(receipt.validate().is_ok());
        assert!(receipt.seller_was_paid());
        assert!(receipt.is_cross_chain());
        assert!(!receipt.has_discount());
    }

    #[test]
    fn test_escrow_state_transitions() {
        let ts = create_test_timestamp();
        let mut escrow = EscrowRecord::new(
            "order-123".to_string(),
            "sess-456".to_string(),
            30000000,
            "USDC".to_string(),
            ts.clone(),
            ts.clone(),
            1,
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
        );

        assert_eq!(escrow.state, EscrowState::BuyerCommitted);

        // Seller accepts
        let result = escrow.seller_accept(
            369,
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            ts.clone(),
            3600,
        );
        assert!(result.is_ok());
        assert_eq!(escrow.state, EscrowState::SellerAccepted);

        // Seller fulfills
        let result = escrow.seller_fulfill(
            "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            12345,
            ts.clone(),
        );
        assert!(result.is_ok());
        assert_eq!(escrow.state, EscrowState::SellerFulfilled);
        assert!(!escrow.late_fulfilled);

        // Seller claims
        let result = escrow.seller_claim(
            "0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
        );
        assert!(result.is_ok());
        assert_eq!(escrow.state, EscrowState::SellerClaimed);
    }
}
