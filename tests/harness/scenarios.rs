// ============================================================================
// CoreProver v0.3 Test Harness
// File: tests/harness/scenarios.rs
//
// This suite defines ALL canonical scenarios for CoreProver v0.3.
//
// These tests use:
//  - TestContext (Chunk 6)
//  - MockChain for deterministic TXIDs & block heights
//  - TimeController for deterministic triple timestamps
//
// The goal: A complete, self-contained, multi-chain, deterministic test suite.
// ============================================================================

#![allow(dead_code)]

use super::context::TestContext;
use super::types::*;
use coreprover_types_v03::EscrowState;

// ============================================================================
// Scenario 1 — Happy Path (On-Time Fulfillment + Claim)
// ============================================================================
#[test]
fn scenario_happy_path() {
    let mut ctx = TestContext::new();

    let buyer_chain = ctx.chain(1);
    let seller_chain = ctx.chain(100);

    let order = ctx.commit(buyer_chain, seller_chain, 1500, "happy");

    // Seller accepts within window
    ctx.advance_time(10);
    ctx.accept(&order, seller_chain);

    // Seller fulfills on time
    ctx.advance_time(20);
    ctx.fulfill(&order, seller_chain);

    // Seller claims within claim window
    ctx.advance_time(30);
    ctx.claim(&order, seller_chain);

    // Verify receipt
    let receipt = ctx.receipt(&order, seller_chain).unwrap();
    assert_eq!(receipt.order_amount, 1500);
    assert!(receipt.seller_was_paid());
    assert_eq!(receipt.discount_pct, 0);
    assert!(!receipt.has_discount());
}

// ============================================================================
// Scenario 2 — Late Fulfillment → Discount → Claim
//
// Flow:
//   commit → accept → (wait past fulfillment deadline) → fulfill late → claim
// ============================================================================
#[test]
fn scenario_late_fulfill_with_discount() {
    let mut ctx = TestContext::new();

    let buyer_chain = ctx.chain(2);
    let seller_chain = ctx.chain(200);

    let order = ctx.commit(buyer_chain, seller_chain, 5000, "late");

    ctx.advance_time(5);
    ctx.accept(&order, seller_chain);

    // Jump PAST fulfillment deadline (default ~30s)
    ctx.advance_time(90);
    ctx.fulfill(&order, seller_chain);

    // Claim happens after fulfillment (late)
    ctx.advance_time(10);
    ctx.claim(&order, seller_chain);

    let receipt = ctx.receipt(&order, seller_chain).unwrap();
    assert!(receipt.has_discount(), "Late fulfill must apply discount");
    assert!(receipt.seller_was_paid());
}

// ============================================================================
// Scenario 3 — Acceptance Window Expires → Buyer Withdraw
// ============================================================================
#[test]
fn scenario_acceptance_timeout_and_buyer_withdraw() {
    let mut ctx = TestContext::new();

    let buyer_chain = ctx.chain(3);
    let seller_chain = ctx.chain(300);

    let order = ctx.commit(buyer_chain, seller_chain, 8888, "accept-expire");

    // Do NOT accept — jump past acceptance deadline (~60s)
    ctx.advance_time(200);

    ctx.withdraw(&order, seller_chain);

    let receipt = ctx.receipt(&order, seller_chain);
    assert!(
        receipt.is_none(),
        "Withdrawn-before-accept should NOT produce a receipt"
    );
}

// ============================================================================
// Scenario 4 — Fulfillment Window Expires → Buyer Withdraw
// ============================================================================
#[test]
fn scenario_fulfillment_expired_then_buyer_withdraw() {
    let mut ctx = TestContext::new();

    let buyer_chain = ctx.chain(4);
    let seller_chain = ctx.chain(400);

    let order = ctx.commit(buyer_chain, seller_chain, 1234, "fulfill-expire");

    ctx.advance_time(10);
    ctx.accept(&order, seller_chain);

    // Do NOT fulfill — pass fulfillment window (~30s)
    ctx.advance_time(200);

    ctx.withdraw(&order, seller_chain);

    let receipt = ctx.receipt(&order, seller_chain);
    assert!(receipt.is_none(), "Expired → withdraw produces no receipt");
}

// ============================================================================
// Scenario 5 — Seller Refund Path
//
// Flow:
//   commit → accept → fulfill → refund
// ============================================================================
#[test]
fn scenario_seller_refund() {
    let mut ctx = TestContext::new();

    let buyer_chain = ctx.chain(5);
    let seller_chain = ctx.chain(500);

    let order = ctx.commit(buyer_chain, seller_chain, 7777, "refund");

    ctx.advance_time(10);
    ctx.accept(&order, seller_chain);

    ctx.advance_time(10);
    ctx.fulfill(&order, seller_chain);

    // Now seller issues refund instead of claim
    ctx.advance_time(5);

    {
        let tx = {
            let chain = ctx.chain_mut(seller_chain);
            chain.generate_txid()
        };
        let oid = super::context::TestContext::encode_order_id(&order);
        let engine = ctx.engine_mut(seller_chain);
        let _ = engine.seller_refund(&oid, tx.txid.clone());
    }

    let receipt = ctx.receipt(&order, seller_chain).unwrap();
    assert!(receipt.seller_refund_txid.is_some(), "Refund must be present");
    assert!(!receipt.seller_was_paid());
}

// ============================================================================
// Scenario 6 — Timed Release Auto-Claim
// ============================================================================
#[test]
fn scenario_timed_release_autoclaim() {
    let mut ctx = TestContext::new();

    let buyer_chain = ctx.chain(6);
    let seller_chain = ctx.chain(600);

    let order = ctx.commit(buyer_chain, seller_chain, 9999, "auto");

    ctx.advance_time(5);
    ctx.accept(&order, seller_chain);

    ctx.advance_time(5);
    ctx.fulfill(&order, seller_chain);

    // Wait past claim window (~60s)
    ctx.advance_time(200);

    {
        let oid = super::context::TestContext::encode_order_id(&order);
        let engine = ctx.engine_mut(seller_chain);
        let _ = engine.timed_release(&oid);
    }

    let receipt = ctx.receipt(&order, seller_chain).unwrap();
    assert!(receipt.seller_was_paid(), "Auto-claim should pay seller");
    assert!(receipt.seller_claim_txid.is_some(), "Auto-claim should emit txid");
}

// ============================================================================
// Scenario 7 — Multi-Chain Parallel Settlement
//
// Here we create THREE simultaneous escrows across THREE separate chains.
// Then we fulfill & claim all in parallel.
// ============================================================================
#[test]
fn scenario_multichain_parallel_settlement() {
    let mut ctx = TestContext::new();

    let buyer_chain = ctx.chain(7);
    let seller_chains = vec![701, 702, 703];

    let mut orders = vec![];

    for i in 0..3 {
        ctx.chain(seller_chains[i]);
        let order = ctx.commit(buyer_chain, seller_chains[i], 1000 + i as u64, "multi");
        orders.push((order, seller_chains[i]));
    }

    ctx.advance_time(10);
    for (order, c) in &orders {
        ctx.accept(order, *c);
    }

    ctx.advance_time(20);
    for (order, c) in &orders {
        ctx.fulfill(order, *c);
    }

    ctx.advance_time(20);
    for (order, c) in &orders {
        ctx.claim(order, *c);
    }

    // Verify all receipts exist independently
    for (order, c) in orders {
        let r = ctx.receipt(&order, c).unwrap();
        assert!(r.seller_was_paid());
    }
}

// ============================================================================
// Scenario 8 — Cross-Chain Provenance Check
// ============================================================================
#[test]
fn scenario_crosschain_provenance() {
    let mut ctx = TestContext::new();

    let buyer_chain = ctx.chain(8);
    let seller_chain = ctx.chain(808);

    let order = ctx.commit(buyer_chain, seller_chain, 3333, "xchain");

    ctx.advance_time(15);
    ctx.accept(&order, seller_chain);

    ctx.advance_time(15);
    ctx.fulfill(&order, seller_chain);

    ctx.advance_time(15);
    ctx.claim(&order, seller_chain);

    let receipt = ctx.receipt(&order, seller_chain).unwrap();

    assert!(receipt.is_cross_chain());
    assert_eq!(receipt.buyer_chain_id, buyer_chain.0);
    assert_eq!(receipt.seller_chain_id, seller_chain.0);
}

// ============================================================================
// Scenario 9 — Deterministic Timestamps
// ============================================================================
#[test]
fn scenario_timestamp_determinism() {
    let mut ctx = TestContext::new();

    let buyer_chain = ctx.chain(9);
    let seller_chain = ctx.chain(909);

    let order = ctx.commit(buyer_chain, seller_chain, 200, "ts");

    ctx.advance_time(22);
    ctx.accept(&order, seller_chain);

    ctx.advance_time(33);
    ctx.fulfill(&order, seller_chain);

    ctx.advance_time(44);
    ctx.claim(&order, seller_chain);

    let r = ctx.receipt(&order, seller_chain).unwrap();

    // Check monotonicity
    assert!(r.fulfillment_unix > 0);
    assert!(r.settlement_unix > r.fulfillment_unix);
}

// ============================================================================
// Scenario 10 — Re-Entrancy Regression (Simulation-Level)
//
// We ensure calling "claim" twice or out-of-order has no effect.
// ============================================================================
#[test]
fn scenario_reentrancy_regression() {
    let mut ctx = TestContext::new();

    let bc = ctx.chain(10);
    let sc = ctx.chain(1010);

    let order = ctx.commit(bc, sc, 1010, "reent");

    ctx.advance_time(5);
    ctx.accept(&order, sc);

    ctx.advance_time(5);
    ctx.fulfill(&order, sc);

    ctx.advance_time(5);
    ctx.claim(&order, sc);

    // Second claim should do nothing harmful
    ctx.advance_time(5);
    ctx.claim(&order, sc);

    let r = ctx.receipt(&order, sc).unwrap();
    assert!(r.seller_was_paid());
}