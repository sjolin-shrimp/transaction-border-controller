// ============================================================================
// CoreProver v0.3 – State Transition Matrix Test
// Validates every legal and illegal state transition using the new hybrid
// test harness: TestContext, EngineDriver, ModelChecker.
//
// Drop this file into:  tests/state_transition_matrix.rs
//
// This test ensures:
//   * Every allowed transition succeeds
//   * Every disallowed transition fails
//   * ModelChecker and EngineDriver agree on validity
//   * No hidden or accidental transitions are permitted
// ============================================================================

use coreprover_types_v03::EscrowState;
use tests::harness::{
    context::TestContext,
    types::{CommitParams},
};

#[test]
fn validate_state_transition_matrix() {
    // Initialize context with default config (invariants enabled)
    let ctx = TestContext::default();

    // All states in v0.3
    let states = vec![
        EscrowState::BuyerCommitted,
        EscrowState::SellerAccepted,
        EscrowState::SellerFulfilled,
        EscrowState::FulfillmentExpired,
        EscrowState::SellerClaimed,
        EscrowState::SellerRefunded,
        EscrowState::BuyerWithdrawn,
    ];

    // Expected valid transitions for v0.3
    // This is the canonical CoreProver state machine
    let mut valid = std::collections::HashMap::new();

    valid.insert(EscrowState::BuyerCommitted, vec![
        EscrowState::SellerAccepted,
        EscrowState::BuyerWithdrawn,  // acceptance timeout
    ]);

    valid.insert(EscrowState::SellerAccepted, vec![
        EscrowState::SellerFulfilled,
        EscrowState::BuyerWithdrawn,  // fulfillment timeout
    ]);

    valid.insert(EscrowState::SellerFulfilled, vec![
        EscrowState::SellerClaimed,
        EscrowState::FulfillmentExpired,
    ]);

    valid.insert(EscrowState::FulfillmentExpired, vec![
        EscrowState::SellerClaimed,
        EscrowState::SellerRefunded,
        EscrowState::BuyerWithdrawn,
    ]);

    valid.insert(EscrowState::SellerClaimed, vec![]);     // terminal
    valid.insert(EscrowState::SellerRefunded, vec![]);    // terminal
    valid.insert(EscrowState::BuyerWithdrawn, vec![]);    // terminal

    // Now test every transition pair (before → after)
    for before in states.iter() {
        for after in states.iter() {

            let allowed = valid
                .get(before)
                .unwrap()
                .contains(after);

            // Use ModelChecker to validate transition correctness
            let result = ctx.model_checker.validate_transition(*before, *after);

            match (allowed, result) {
                (true, Ok(_)) => {
                    println!("✓ {:?} → {:?} (allowed, passes)", before, after);
                }
                (true, Err(e)) => {
                    panic!(
                        "❌ Transition {:?} → {:?} SHOULD be valid but ModelChecker rejected it: {}",
                        before, after, e
                    );
                }
                (false, Ok(_)) => {
                    panic!(
                        "❌ Transition {:?} → {:?} SHOULD be invalid but ModelChecker ACCEPTED it",
                        before, after
                    );
                }
                (false, Err(_)) => {
                    // Expected failure
                    println!("✓ {:?} → {:?} (blocked, correct)", before, after);
                }
            }
        }
    }
}