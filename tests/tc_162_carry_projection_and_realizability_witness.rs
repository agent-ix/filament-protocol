// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-162: a backend's witness, counterexample or projected artifact carried
//! unchanged beside the source identities the disposition retains (FR-152).

mod support {
    pub mod dynamic;
    pub mod handoff;
    pub mod projection;
    pub mod realizability;
    pub mod standin;
}

use filament_contract_model::CheckedNodeId;
use filament_protocol::dispatch::{
    dispatch, dispatch_claims, ClaimDisposition, ClaimRequest, DispositionRefusal,
    ProtocolClaimKind, ProtocolOutcome,
};
use filament_protocol::disposition::ClaimSettlement;
use fsl_foundation::witness::ValuePathStep;
use fsl_replay::{SettlementBasis, Truth};
use fsl_semantics::check::Capability;
use ix_trace_rs::trace;
use support::dynamic::Dynamic;
use support::realizability::refuted_record;
use support::standin::{always, request, serve, witness, Boundary, Sentinel};

const KIND: ProtocolClaimKind = ProtocolClaimKind::Realizability;

/// A decisive result carrying the record FSL's engine writes for a refuted
/// realizability result, and the witness that record holds.
fn decisive(_: &Dynamic, truth: Truth, basis: SettlementBasis) -> Sentinel {
    let (record, witness) = refuted_record();
    let mut result = Sentinel::new("decisive", truth, basis);
    result.witness = Some(witness);
    result.record = Some(record);
    result
}

fn carried(dynamic: &Dynamic, kind: ProtocolClaimKind, result: Sentinel) {
    let mut boundary = Boundary::new();
    serve(
        &mut boundary,
        "backend",
        &[kind.capability()],
        always(result.clone()),
    );
    let asked = request(dynamic, kind, &["Head"]);
    let outcome = dispatch(&mut boundary, dynamic.linked().declarations(), &asked);
    let ProtocolOutcome::Settled(ClaimSettlement::Result {
        result: carried,
        premises,
    }) = outcome
    else {
        panic!("{outcome:?}");
    };
    assert_eq!(carried, result);
    assert_eq!(carried.witness, result.witness);
    assert!(!premises.request.sources.is_empty());
    assert_eq!(premises.request.sources, asked.sources);
}

#[test]
#[trace("TC-162", "FR-152-AC-1")]
fn a_decisive_witness_is_carried_with_the_retained_source_identities() {
    let dynamic = Dynamic::with("workflow", 3);
    // A realizability record is the refutation FSL's engine writes, so a
    // `decisive-witness` result is carried here as a projection result.
    let mut result = Sentinel::new(
        "decisive",
        Truth::Satisfied,
        SettlementBasis::DecisiveWitness,
    );
    result.witness = Some(refuted_record().1);
    carried(&dynamic, ProtocolClaimKind::LocalProjection, result);
}

#[test]
#[trace("TC-162", "FR-152-AC-2")]
fn a_decisive_counterexample_is_carried_with_the_retained_source_identities() {
    let dynamic = Dynamic::with("workflow", 3);
    carried(
        &dynamic,
        KIND,
        decisive(
            &dynamic,
            Truth::Violated,
            SettlementBasis::DecisiveCounterexample,
        ),
    );
}

#[test]
#[trace("TC-162", "FR-152-AC-3")]
fn a_non_minimal_witness_is_carried_with_every_step_in_order() {
    let dynamic = Dynamic::with("workflow", 3);
    let steps = vec![
        ValuePathStep::Field("a".to_owned()),
        ValuePathStep::Index(2),
        ValuePathStep::Index(0),
        ValuePathStep::Field("a".to_owned()),
        ValuePathStep::Index(2),
    ];
    let mut result = Sentinel::new(
        "long",
        Truth::Violated,
        SettlementBasis::DecisiveCounterexample,
    );
    result.witness = Some(witness(&dynamic, steps.clone()));
    let mut boundary = Boundary::new();
    serve(
        &mut boundary,
        "backend",
        &[Capability::LocalProjection],
        always(result),
    );
    let outcome = dispatch(
        &mut boundary,
        dynamic.linked().declarations(),
        &request(&dynamic, ProtocolClaimKind::LocalProjection, &["Head"]),
    );
    let ProtocolOutcome::Settled(ClaimSettlement::Result { result, .. }) = outcome else {
        panic!("{outcome:?}");
    };
    let record = result.witness.expect("the witness");
    assert_eq!(record.value_path.steps, steps);
}

#[test]
#[trace("TC-162", "FR-152-AC-4")]
fn a_decisive_result_with_no_source_identity_is_refused_alone() {
    let dynamic = Dynamic::with("workflow", 3);
    let mut boundary = Boundary::new();
    serve(
        &mut boundary,
        "backend",
        &[Capability::Realizability],
        always(decisive(
            &dynamic,
            Truth::Violated,
            SettlementBasis::DecisiveCounterexample,
        )),
    );
    let declarations = dynamic.linked().declarations();
    let mut unbound = request(&dynamic, KIND, &["Head"]);
    unbound.sources.clear();
    let bound = request(&dynamic, KIND, &["Head"]);
    let together = dispatch_claims::<_, _, CheckedNodeId, _>(
        &mut boundary,
        declarations,
        &[],
        &[
            ClaimRequest::Protocol(unbound),
            ClaimRequest::Protocol(bound.clone()),
        ],
    );
    let alone: Vec<ClaimDisposition<&str, _, _, _>> = dispatch_claims::<_, _, CheckedNodeId, _>(
        &mut boundary,
        declarations,
        &[],
        &[ClaimRequest::Protocol(bound)],
    );
    assert_eq!(
        together[0],
        ClaimDisposition::Protocol(ProtocolOutcome::Refused(
            DispositionRefusal::NoSourceIdentity
        ))
    );
    assert_eq!(together[1], alone[0]);
}

#[test]
#[trace("TC-162", "FR-152-AC-5")]
fn a_projected_local_artifact_is_carried_unchanged() {
    let dynamic = Dynamic::with("workflow", 3);
    let mut result = Sentinel::new("projected", Truth::Satisfied, SettlementBasis::ClosedScope);
    result.artifact = Some("local-artifact-bytes");
    let mut boundary = Boundary::new();
    serve(
        &mut boundary,
        "backend",
        &[Capability::LocalProjection],
        always(result.clone()),
    );
    let outcome = dispatch(
        &mut boundary,
        dynamic.linked().declarations(),
        &request(&dynamic, ProtocolClaimKind::LocalProjection, &["Head"]),
    );
    let ProtocolOutcome::Settled(ClaimSettlement::Result {
        result: carried, ..
    }) = outcome
    else {
        panic!("{outcome:?}");
    };
    assert_eq!(carried, result);
}

#[test]
#[trace("TC-162", "FR-152-AC-7")]
fn a_decisive_realizability_result_needs_a_positioned_witness_and_a_projection_does_not() {
    let dynamic = Dynamic::with("workflow", 3);
    let declarations = dynamic.linked().declarations();
    let bare = Sentinel::new("bare", Truth::Satisfied, SettlementBasis::DecisiveWitness);
    // Each carries the record FSL wrote, but no positioned witness.
    let mut recorded = bare.clone();
    recorded.record = Some(refuted_record().0);
    let mut unpositioned = recorded.clone();
    let mut record = witness(&dynamic, Vec::new());
    record.trace_position = None;
    unpositioned.witness = Some(record);
    for result in [recorded, unpositioned] {
        let mut boundary = Boundary::new();
        serve(
            &mut boundary,
            "backend",
            &[Capability::Realizability],
            always(result),
        );
        let outcome = dispatch(
            &mut boundary,
            declarations,
            &request(&dynamic, KIND, &["Head"]),
        );
        assert_eq!(
            outcome,
            ProtocolOutcome::Refused(DispositionRefusal::DecisiveWithoutWitness)
        );
    }
    let mut boundary = Boundary::new();
    serve(
        &mut boundary,
        "backend",
        &[Capability::LocalProjection],
        always(bare.clone()),
    );
    let outcome = dispatch(
        &mut boundary,
        declarations,
        &request(&dynamic, ProtocolClaimKind::LocalProjection, &["Head"]),
    );
    let ProtocolOutcome::Settled(ClaimSettlement::Result { result, .. }) = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(result, bare);
}
