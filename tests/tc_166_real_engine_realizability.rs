// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-166: `realizability` claims dispatched to FSL's real `protocol-analysis`
//! engine, with no stand-in (FR-150, TL-387 AC1).

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
#[trace("TC-166", "FR-150-AC-16")]
fn a_realizable_protocol_is_proved_by_the_real_engine_and_carried_satisfied() {
    let result = settle(&Realizability::worked(PICK, CANCELLED));
    assert_eq!(result.truth(), Truth::Satisfied);
    assert_eq!(result.basis(), SettlementBasis::ClosedScope);
    assert!(result.witness().is_none());
    let Some(ProtocolAnalysisArtifact::Realizability(found)) = result.artifact() else {
        panic!("a realizability result");
    };
    assert!(matches!(found.outcome, AnalysisOutcome::Proved(_)));
}

#[test]
#[trace("TC-166", "FR-150-AC-17")]
fn an_unrealizable_protocol_carries_the_engines_record_and_witness_unchanged() {
    let result = settle(&Realizability::worked(PICK, ""));
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

#[test]
#[trace("TC-166", "FR-150-AC-17")]
fn an_incompatible_merge_is_refuted_by_the_real_engine_and_its_witness_carried_unchanged() {
    // The uninformed `Warehouse` must send a different message in each
    // branch of `Seller`'s choice: the branches cannot merge for it.
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
        action_a,
        action_b,
        separating_witness,
        ..
    }) = &found.outcome
    else {
        panic!("indistinguishable histories");
    };
    assert_ne!(action_a, action_b, "the branches need different actions");
    assert_eq!(result.witness(), Some(separating_witness));
}
