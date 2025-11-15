//! ============================================================================
//! CoreProver Harness - Glue & Integration Layer (Chunk 9 of 9)
//!
//! This file ties everything together:
//!
//!  - Loads EscrowTrace test scripts
//!  - Runs them through the EngineRunner
//!  - Captures EngineSnapshot sequences
//!  - Performs Model Checking
//!  - Applies optional Fault Injection Profiles
//!  - Produces a HarnessReport summarizing all runs
//!  - Exposes a clean public API for cargo tests
//!
//! All logic here is deterministic and side-effect–free.
//!
//! ============================================================================

use crate::harness::{
    engine_runner::{EngineRunner, EngineSnapshot},
    fault::{FaultProfile, FaultResult},
    model_checker::{ModelChecker, ModelCheckReport},
    trace::{EscrowTrace},
};
use serde::{Serialize, Deserialize};

// ============================================================================
// Result structs
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessCaseResult {
    pub trace_name: String,
    pub engine_snapshots: Vec<EngineSnapshot>,
    pub model_check: ModelCheckReport,
    pub fault_results: Vec<FaultResult>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessReport {
    pub results: Vec<HarnessCaseResult>,
}

impl HarnessReport {
    pub fn passed(&self) -> bool {
        self.results.iter().all(|r| r.passed)
    }
}

// ============================================================================
// Harness Runner
// ============================================================================

pub struct Harness;

impl Harness {
    /// Run a single escrow test trace through:
    /// 1. Engine simulation
    /// 2. Model checking
    /// 3. Optional fault injection
    pub fn run_one(
        trace: EscrowTrace,
        faults: Option<&Vec<FaultProfile>>,
    ) -> HarnessCaseResult {
        let trace_name = trace.name.clone();

        // --- Step 1: simulate engine ---
        let snapshots = EngineRunner::run(&trace);

        // --- Step 2: run model-checker on FINAL snapshot ---
        let final_snapshot = snapshots.last()
            .expect("EngineRunner must always produce >=1 snapshot");

        let model_report = ModelChecker::check(&trace, final_snapshot);

        // --- Step 3: run fault injection (if provided) ---
        let fault_results = match faults {
            Some(list) => {
                list.iter()
                    .map(|profile| profile.inject_and_run(&trace))
                    .collect::<Vec<_>>()
            }
            None => vec![],
        };

        // --- Compute pass condition ---
        let passed =
            model_report.passed &&
            fault_results.iter().all(|fr| fr.passed);

        HarnessCaseResult {
            trace_name,
            engine_snapshots: snapshots,
            model_check: model_report,
            fault_results,
            passed,
        }
    }

    /// Run multiple traces and aggregate results
    pub fn run_all(
        traces: Vec<EscrowTrace>,
        faults: Option<&Vec<FaultProfile>>,
    ) -> HarnessReport {
        let results = traces
            .into_iter()
            .map(|t| Self::run_one(t, faults))
            .collect::<Vec<_>>();

        HarnessReport { results }
    }
}

// ============================================================================
// Public entry points for cargo tests
// ============================================================================

/// Execute a single scenario (used by unit tests)
pub fn run_scenario(trace: EscrowTrace) -> HarnessCaseResult {
    Harness::run_one(trace, None)
}

/// Execute multiple traces (used by integration tests)
pub fn run_scenarios(traces: Vec<EscrowTrace>) -> HarnessReport {
    Harness::run_all(traces, None)
}

/// Execute multiple traces + faults (used by adversarial tests)
pub fn run_scenarios_with_faults(
    traces: Vec<EscrowTrace>,
    faults: Vec<FaultProfile>,
) -> HarnessReport {
    Harness::run_all(traces, Some(&faults))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::trace::*;

    #[test]
    fn harness_runs_basic_scenario() {
        // A tiny sanity test ensuring the harness loads and executes
        let trace = EscrowTrace::new("basic-test")
            .buyer_commit("0xAAA", "buyer", "seller", 100)
            .advance(10)
            .seller_accept("0xBBB")
            .advance(10)
            .seller_fulfill("0xCCC")
            .advance(10)
            .seller_claim("0xDDD");

        let result = run_scenario(trace);

        assert!(result.passed, "basic harness smoke-test failed");
        assert!(result.model_check.passed, "model checker failed on basic test");
    }
}