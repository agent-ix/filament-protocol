// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-178: a `finite-replay` claim is decided by FSL's real `protocol-analysis`
//! engine over a supplied finite trace, and FSL's terminal record is carried
//! unchanged or its refusal carried with its code (FR-303, FR-207-AC-3).

mod support {
    pub mod dynamic;
    pub mod handoff;
    pub mod projection;
    pub mod realizability;
    pub mod standin;
}

use filament_protocol::dispatch::{
    dispatch, BackendResult, Binding, DispositionRefusal, ProtocolClaimKind, ProtocolOutcome,
    ProviderBoundary,
};
use filament_protocol::disposition::{ClaimSettlement, Disposition, Warning};
use filament_protocol::engine::{engine_descriptor, EngineRequest, EngineResult, FslEngine};
use filament_semantic_value::catalog::CatalogCode;
use fsl_analyze::analyze::{analyze, Engines};
use fsl_analyze::protocol_analysis::manifest::BACKEND_ID;
use fsl_foundation::{BackendId, RequestIndex};
use fsl_replay::{
    Certification, ProofBasis, SettlementBasis, TerminalRecord, TerminalValue, Truth,
};
use fsl_route::ProviderOrigin;
use fsl_semantics::check::Capability;
use ix_trace_rs::trace;
use serde_json::Value;
use support::dynamic::Dynamic;
use support::projection::{claim_of, Asked, CANCELLED, PICK};
use support::realizability::Realizability;
use support::standin::{always, request, serve, Boundary, Sentinel};

type Outcome = ProtocolOutcome<EngineResult, Binding<()>>;

fn replay_claim(fixture: &Realizability) -> filament_protocol::dispatch::ProtocolRequest<()> {
    claim_of(
        ProtocolClaimKind::FiniteReplay,
        fixture.linked(),
        fixture.clause(),
        |form, name| fixture.named(form, name),
    )
}

/// Dispatch the `finite-replay` claim of the trace `steps` to FSL's engine.
fn replay(fixture: &Realizability, steps: &[Value]) -> Outcome {
    let (snapshots, trace) = fixture.with_trace("t", steps);
    let mut boundary: ProviderBoundary<'_, Asked, EngineResult> = ProviderBoundary::new();
    let engine = FslEngine::new(fixture.package(), |_: &Binding<()>| {
        EngineRequest::FiniteReplay(Box::new(fixture.replay_request(&snapshots, trace.clone())))
    })
    .expect("FSL's linked engines register");
    boundary
        .register(
            engine_descriptor(ProviderOrigin::Linked).expect("FSL's own descriptor"),
            engine,
        )
        .expect("the registration is accepted");
    dispatch(
        &mut boundary,
        fixture.linked().declarations(),
        &replay_claim(fixture),
    )
}

fn refused_with(outcome: Outcome) -> CatalogCode {
    let ProtocolOutcome::Settled(ClaimSettlement::Disposition(Disposition::Refused(refused))) =
        outcome
    else {
        panic!("a refusal: {outcome:?}");
    };
    refused.code
}

#[test]
#[trace("TC-178", "FR-303-AC-1")]
#[trace("TC-178", "FR-207-AC-3")]
fn a_trace_the_protocol_replays_is_proved_and_fsls_record_carried_unchanged() {
    let fixture = Realizability::worked(PICK, CANCELLED);
    let steps = fixture.trace_steps(3);
    let ProtocolOutcome::Settled(ClaimSettlement::Result { result, premises }) =
        replay(&fixture, &steps)
    else {
        panic!("the engine decides the claim");
    };
    assert_eq!(result.truth(), Truth::Satisfied);
    assert_eq!(result.basis(), SettlementBasis::ClosedScope);
    assert_eq!(
        premises.selected.as_ref().map(|id| id.as_str()),
        Some(BACKEND_ID)
    );
    let record = result.replay_record().expect("FSL's terminal record");
    assert_eq!(
        record.value(),
        &TerminalValue::Proved {
            basis: ProofBasis::BoundedComplete { depth: 3 },
            certification: Certification::Certified,
        }
    );
    // The record is the one FSL's `analyze` returns for the same request.
    let (snapshots, trace) = fixture.with_trace("t", &steps);
    let direct = analyze(
        fixture.checked(),
        &fixture.replay_request(&snapshots, trace).request,
        &Engines::fsl().expect("FSL's engines"),
        &fsl_analyze::analyze::AnalyzeLimits::unbounded(),
        &filament_exact::Cancel::new(),
    )
    .expect("the request is admitted")
    .into_value();
    assert_eq!(Some(record), direct.records.first());
    assert_eq!(record.request_index(), RequestIndex::new(0));
}

#[test]
#[trace("TC-178", "FR-303-AC-2")]
fn a_trace_with_a_step_not_enabled_at_the_replayed_state_is_refused_with_fsls_code() {
    let fixture = Realizability::worked(PICK, CANCELLED);
    // The second step of the run, replayed from the initial state: the first
    // step it needs has not been taken.
    let steps = fixture.trace_steps(2);
    let outcome = replay(&fixture, &steps[1..2]);
    assert_eq!(outcome.truth(), Truth::Unavailable);
    assert_eq!(
        refused_with(outcome),
        CatalogCode::new("invalid_runtime_input", "invalid-value")
    );
}

#[test]
#[trace("TC-178", "FR-303-AC-3")]
fn a_send_step_without_its_send_record_is_refused_before_any_step_replays() {
    let fixture = Realizability::worked(PICK, CANCELLED);
    let mut steps = fixture.trace_steps(2);
    steps[0]
        .as_object_mut()
        .expect("a step")
        .remove("send_record")
        .expect("the first step is a send");
    assert_eq!(
        refused_with(replay(&fixture, &steps)),
        CatalogCode::new("invalid_runtime_input", "missing-member")
    );
}

#[test]
#[trace("TC-178", "FR-303-AC-4")]
fn a_request_of_another_kind_than_the_claim_is_refused_inapplicable_kind() {
    let fixture = Realizability::worked(PICK, CANCELLED);
    let inapplicable = CatalogCode::new("invalid_capability", "inapplicable-kind");
    let (snapshots, trace) = fixture.with_trace("t", &fixture.trace_steps(1));
    // A finite-replay claim, a realizability request.
    let mut boundary: ProviderBoundary<'_, Asked, EngineResult> = ProviderBoundary::new();
    let engine = FslEngine::new(fixture.package(), |_: &Binding<()>| {
        EngineRequest::Realizability(Box::new(fixture.request()))
    })
    .expect("FSL's linked engines register");
    boundary
        .register(
            engine_descriptor(ProviderOrigin::Linked).expect("FSL's own descriptor"),
            engine,
        )
        .expect("the registration is accepted");
    let outcome = dispatch(
        &mut boundary,
        fixture.linked().declarations(),
        &replay_claim(&fixture),
    );
    assert_eq!(refused_with(outcome), inapplicable);
    // A realizability claim, a finite-replay request.
    let mut boundary: ProviderBoundary<'_, Asked, EngineResult> = ProviderBoundary::new();
    let engine = FslEngine::new(fixture.package(), |_: &Binding<()>| {
        EngineRequest::FiniteReplay(Box::new(fixture.replay_request(&snapshots, trace.clone())))
    })
    .expect("FSL's linked engines register");
    boundary
        .register(
            engine_descriptor(ProviderOrigin::Linked).expect("FSL's own descriptor"),
            engine,
        )
        .expect("the registration is accepted");
    let claim = claim_of(
        ProtocolClaimKind::Realizability,
        fixture.linked(),
        fixture.clause(),
        |form, name| fixture.named(form, name),
    );
    let outcome = dispatch(&mut boundary, fixture.linked().declarations(), &claim);
    assert_eq!(refused_with(outcome), inapplicable);
}

#[test]
#[trace("TC-178", "FR-303-AC-5")]
fn an_empty_registry_settles_a_finite_replay_claim_unsupported() {
    let fixture = Realizability::worked(PICK, CANCELLED);
    let mut boundary: ProviderBoundary<'_, Asked, EngineResult> = ProviderBoundary::new();
    let outcome = dispatch(
        &mut boundary,
        fixture.linked().declarations(),
        &replay_claim(&fixture),
    );
    assert_eq!(outcome.truth(), Truth::Unavailable);
    let ProtocolOutcome::Settled(ClaimSettlement::Disposition(Disposition::Unsupported(
        unsupported,
    ))) = outcome
    else {
        panic!("unsupported: {outcome:?}");
    };
    assert_eq!(
        unsupported.warnings,
        vec![Warning::UnservedCapability {
            kind: Capability::FiniteReplay,
            backend: None,
        }]
    );
}

/// A stand-in result for a `finite-replay` claim.
fn replayed(truth: Truth, basis: SettlementBasis, record: Option<TerminalRecord>) -> Sentinel {
    let mut result = Sentinel::new("replay", truth, basis);
    result.terminal = record;
    result
}

fn proved_record() -> TerminalRecord {
    TerminalRecord::proved_by_replay(RequestIndex::new(0), BackendId::new(BACKEND_ID), 3)
}

fn standin_outcome(result: Sentinel) -> ProtocolOutcome<Sentinel, support::standin::Bind> {
    let dynamic = Dynamic::with("workflow", 3);
    let mut boundary = Boundary::new();
    serve(
        &mut boundary,
        "backend",
        &[Capability::FiniteReplay],
        always(result),
    );
    dispatch(
        &mut boundary,
        dynamic.linked().declarations(),
        &request(&dynamic, ProtocolClaimKind::FiniteReplay, &["Head"]),
    )
}

#[test]
#[trace("TC-178", "FR-303-AC-6")]
fn a_replay_result_without_a_coherent_terminal_record_is_unsupported_or_refused() {
    // The coherent result is carried.
    let carried = standin_outcome(replayed(
        Truth::Satisfied,
        SettlementBasis::ClosedScope,
        Some(proved_record()),
    ));
    assert!(matches!(
        carried,
        ProtocolOutcome::Settled(ClaimSettlement::Result { .. })
    ));
    // No record: the claim settles unsupported with the warning.
    let ProtocolOutcome::Settled(ClaimSettlement::Disposition(Disposition::Unsupported(
        unsupported,
    ))) = standin_outcome(replayed(
        Truth::Satisfied,
        SettlementBasis::ClosedScope,
        None,
    ))
    else {
        panic!("unsupported");
    };
    assert!(matches!(
        unsupported.warnings.as_slice(),
        [Warning::NoRunRecord {
            kind: Capability::FiniteReplay,
            ..
        }]
    ));
    // A record that is not the proved one, a truth or a basis the record does
    // not state, and a witness a replay does not have.
    let declined = TerminalRecord::declined_with(
        RequestIndex::new(0),
        BackendId::new(BACKEND_ID),
        fsl_replay::ProofRefusalCause::InvalidInput,
        fsl_replay::DeclineCode::Fsl(fsl_foundation::Code::InvalidRuntimeInput),
        CatalogCode::new("invalid_runtime_input", "invalid-value"),
    );
    for (what, result, refusal) in [
        (
            "a declined record",
            replayed(
                Truth::Satisfied,
                SettlementBasis::ClosedScope,
                Some(declined),
            ),
            DispositionRefusal::ReplayWithoutItsRecord,
        ),
        (
            "another truth",
            replayed(
                Truth::Violated,
                SettlementBasis::ClosedScope,
                Some(proved_record()),
            ),
            DispositionRefusal::ReplayWithoutItsRecord,
        ),
        (
            "another basis",
            replayed(
                Truth::Satisfied,
                SettlementBasis::Unsettled,
                Some(proved_record()),
            ),
            DispositionRefusal::ReplayWithoutItsRecord,
        ),
        (
            "a witness",
            {
                let dynamic = Dynamic::with("workflow", 3);
                let mut result = replayed(
                    Truth::Satisfied,
                    SettlementBasis::ClosedScope,
                    Some(proved_record()),
                );
                result.witness = Some(support::standin::witness(&dynamic, Vec::new()));
                result
            },
            DispositionRefusal::WitnessWithoutDecisiveBasis,
        ),
    ] {
        assert_eq!(
            standin_outcome(result),
            ProtocolOutcome::Refused(refusal),
            "{what}"
        );
    }
}
