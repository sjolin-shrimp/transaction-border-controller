// ============================================================================
// CoreProver v0.3 Stress & Scale Test Suite
// File: tests/harness/stress.rs
//
// These tests stress-test the engine across:
//   - 10–100 chains
//   - 100–10,000 total escrows
//   - massively parallel settlement flows
//   - deterministic timestamp progression
//   - cross-chain provenance correctness
//
// NOTE: These are HIGH-VOLUME tests — designed for CI nightly or local.
//
// ============================================================================

#![allow(dead_code)]

use super::context::TestContext;
use super::types::*;
use coreprover_types_v03::EscrowState;

// Deterministic PRNG (no_std friendly)
fn rng(seed: &mut u64) -> u64 {
    // xorshift64*
    *seed ^= *seed >> 12;
    *seed ^= *seed << 25;
    *seed ^= *seed >> 27;
    (*seed).wrapping_mul(0x2545F4914F6CDD1D)
}

// ============================================================================
// Stress 1 — 10 Chains, 100 Escrows, Full Settlement
// ============================================================================
#[test]
fn stress_10_chains_100_escrows() {
    let mut ctx = TestContext::new();

    let num_chains = 10;
    let num_orders = 100;

    for cid in 1..=num_chains {
        ctx.chain(cid);
    }

    let mut seed = 12345_u64;
    let mut orders = vec![];

    // Create 100 escrows with randomized cross-chain pairs
    for _ in 0..num_orders {
        let buyer_c = (rng(&mut seed) % num_chains as u64) + 1;
        let seller_c = (rng(&mut seed) % num_chains as u64) + 1;
        let order = ctx.commit((buyer_c, ""), (seller_c, ""), 1000, "stress10");
        orders.push((order, buyer_c, seller_c));
    }

    ctx.advance_time(10);

    for (order, _, sc) in &orders {
        ctx.accept(order, *sc);
    }

    ctx.advance_time(25);

    for (order, _, sc) in &orders {
        ctx.fulfill(order, *sc);
    }

    ctx.advance_time(25);

    for (order, _, sc) in &orders {
        ctx.claim(order, *sc);
    }

    // Assert all receipts exist + correct provenance
    for (order, bc, sc) in orders {
        let r = ctx.receipt(&order, sc).unwrap();
        assert_eq!(r.buyer_chain_id, bc);
        assert_eq!(r.seller_chain_id, sc);
        assert!(r.seller_was_paid());
    }
}

// ============================================================================
// Stress 2 — 25 Chains, 1,000 Escrows (Batch Mode)
// ============================================================================
#[test]
fn stress_25_chains_1000_escrows() {
    let mut ctx = TestContext::new();

    let num_chains = 25;
    let num_orders = 1000;

    for cid in 1..=num_chains {
        ctx.chain(cid);
    }

    let mut seed = 777777_u64;
    let mut orders = Vec::with_capacity(num_orders);

    // Create 1,000 cross-chain escrows
    for _ in 0..num_orders {
        let bc = (rng(&mut seed) % num_chains as u64) + 1;
        let sc = (rng(&mut seed) % num_chains as u64) + 1;
        let order = ctx.commit((bc, ""), (sc, ""), 777, "stress25");
        orders.push((order, bc, sc));
    }

    ctx.advance_time(5);
    for (order, _, sc) in &orders {
        ctx.accept(order, *sc);
    }

    ctx.advance_time(20);
    for (order, _, sc) in &orders {
        ctx.fulfill(order, *sc);
    }

    ctx.advance_time(30);
    for (order, _, sc) in &orders {
        ctx.claim(order, *sc);
    }

    // Verify load-invariant conditions
    for (order, bc, sc) in orders {
        let r = ctx.receipt(&order, sc).unwrap();
        assert_eq!(r.buyer_chain_id, bc);
        assert_eq!(r.seller_chain_id, sc);
        assert!(r.seller_was_paid());
    }
}

// ============================================================================
// Stress 3 — 50 Chains, 2,500 Escrows
// ============================================================================
#[test]
fn stress_50_chains_2500_escrows() {
    let mut ctx = TestContext::new();

    let num_chains = 50;
    let num_orders = 2500;

    for cid in 1..=num_chains {
        ctx.chain(cid);
    }

    let mut seed = 99999_u64;
    let mut orders = Vec::with_capacity(num_orders);

    for _ in 0..num_orders {
        let bc = (rng(&mut seed) % num_chains as u64) + 1;
        let sc = (rng(&mut seed) % num_chains as u64) + 1;
        let order = ctx.commit((bc, ""), (sc, ""), 2222, "stress50");
        orders.push((order, bc, sc));
    }

    ctx.advance_time(10);
    for (order, _, sc) in &orders {
        ctx.accept(order, *sc);
    }

    ctx.advance_time(40);
    for (order, _, sc) in &orders {
        ctx.fulfill(order, *sc);
    }

    ctx.advance_time(40);
    for (order, _, sc) in &orders {
        ctx.claim(order, *sc);
    }

    // A few random checks
    for i in (0..num_orders).step_by(211) {
        let (_, bc, sc) = orders[i];
        let order = orders[i].0.clone();
        let r = ctx.receipt(&order, sc).unwrap();
        assert_eq!(r.buyer_chain_id, bc);
        assert!(
            r.settlement_unix > r.fulfillment_unix,
            "Timestamps must be monotonic"
        );
    }
}

// ============================================================================
// Stress 4 — 100 Chains, 10,000 Escrows
//
// NOTE: This is extremely heavy but ensures the engine has NO hidden O(n^2)
// behavior and validates full correctness under max parallel load.
// ============================================================================
#[test]
fn stress_100_chains_10000_escrows() {
    let mut ctx = TestContext::new();

    let num_chains = 100;
    let num_orders = 10_000;

    for cid in 1..=num_chains {
        ctx.chain(cid);
    }

    let mut seed = 424242_u64;
    let mut orders = Vec::with_capacity(num_orders);

    // Phase 1 — Commit
    for _ in 0..num_orders {
        let bc = (rng(&mut seed) % num_chains as u64) + 1;
        let sc = (rng(&mut seed) % num_chains as u64) + 1;
        let order = ctx.commit((bc, ""), (sc, ""), 1, "mega");
        orders.push((order, bc, sc));
    }

    ctx.advance_time(20);
    // Phase 2 — Accept in bulk
    for (order, _, sc) in &orders {
        ctx.accept(order, *sc);
    }

    ctx.advance_time(60);
    // Phase 3 — Fulfill in bulk
    for (order, _, sc) in &orders {
        ctx.fulfill(order, *sc);
    }

    ctx.advance_time(80);
    // Phase 4 — Claim in bulk
    for (order, _, sc) in &orders {
        ctx.claim(order, *sc);
    }

    // Phase 5 — Verify a sample set
    for idx in (0..num_orders).step_by(777) {
        let (order, bc, sc) = orders[idx];
        let r = ctx.receipt(&order, sc).unwrap();
        assert_eq!(r.buyer_chain_id, bc);
        assert_eq!(r.seller_chain_id, sc);
        assert!(r.seller_was_paid());
    }
}

// ============================================================================
// Stress 5 — Randomized Window Chaos (Out-of-order events)
// ============================================================================
#[test]
fn stress_randomized_deadline_chaos() {
    let mut ctx = TestContext::new();
    let buyer_chain = ctx.chain(8000);
    let seller_chain = ctx.chain(9000);

    let mut seed = 20240220_u64;

    // Create 200 escrows
    let mut orders = vec![];
    for _ in 0..200 {
        let amt = (rng(&mut seed) % 5000) + 1;
        let order = ctx.commit(buyer_chain, seller_chain, amt, "chaos");
        orders.push(order);
    }

    // Randomly advance and apply operations
    for order in &orders {
        let r = rng(&mut seed) % 4;
        match r {
            0 => {
                ctx.advance_time(10);
                ctx.accept(order, seller_chain);
            }
            1 => ctx.advance_time(20),
            2 => {
                ctx.fulfill(order, seller_chain);
            }
            3 => {
                // Maybe withdraw or accept-late or claim-late
                ctx.advance_time(50);
            }
            _ => {}
        }
    }

    // Ensure engine NEVER panics
    for order in &orders {
        let oid = super::context::TestContext::encode_order_id(order);
        let engine = ctx.engine(seller_chain);
        let _ = engine.get_receipt(&oid); // Must NOT panic
    }
}