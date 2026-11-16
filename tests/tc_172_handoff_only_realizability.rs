// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-172: `realizability` claims over a subject built from a checked-handoff
//! directory alone (FR-150, FSL FR-571).

mod support {
    pub mod dynamic;
    pub mod handoff;
    pub mod projection;
    pub mod realizability;
}

use filament_protocol::dispatch::BackendResult;
use fsl_analyze::protocol_analysis::ProtocolAnalysisArtifact;
use fsl_replay::protocol_analysis::{AnalysisOutcome, RealizabilityCounterexample};
use fsl_replay::{SettlementBasis, Truth};
use ix_trace_rs::trace;
use support::projection::{CANCELLED, PICK};
use support::realizability::{channel, settle, Realizability};

#[test]
#[trace("TC-172", "FR-150-AC-18")]
fn a_handoff_only_subject_proves_a_protocol_every_role_can_tell_apart() {
    // The fixture holds the compiled handoff and the snapshot built from its
    // domain documents; the subject is FSL's `protocol_subject` over them.
    let result = settle(&Realizability::worked(PICK, CANCELLED));
    assert_eq!(result.truth(), Truth::Satisfied);
    assert_eq!(result.basis(), SettlementBasis::ClosedScope);
    assert!(result.witness().is_none());
}

#[test]
#[trace("TC-172", "FR-150-AC-18")]
fn a_handoff_only_subject_refutes_an_incompatible_merge_with_the_witness_unchanged() {
    let extra = channel("Ready", "Warehouse", "Seller", "PickMsg");
    let ship = "send R1 via Ready as (r1: PickMsg) { true };\n\
        receive R1r via Ready of R1 as (s1: PickMsg) { true };";
    let cancel = "send R2 via Ready as (r2: PickMsg) { true };\n\
        receive R2r via Ready of R2 as (s2: PickMsg) { true };";
    let result = settle(&Realizability::worked_with(&extra, ship, cancel));
    assert_eq!(result.truth(), Truth::Violated);
    assert_eq!(result.basis(), SettlementBasis::DecisiveCounterexample);
    let Some(ProtocolAnalysisArtifact::Realizability(found)) = result.artifact() else {
        panic!("a realizability result");
    };
    let AnalysisOutcome::Refuted(RealizabilityCounterexample::IndistinguishableHistories {
        separating_witness,
        ..
    }) = &found.outcome
    else {
        panic!("indistinguishable histories");
    };
    assert_eq!(result.witness(), Some(separating_witness));
}
