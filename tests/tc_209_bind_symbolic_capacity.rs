// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-209: a `symbolic(<name>)` queue capacity dispatched to FSL's real engines
//! (FR-054, FR-058, FR-150, FR-205; FSpec FR-785). The capacity binding is
//! FSL's own `CapacityBinding` in the request's protocol subject. FSL decides
//! with the bound, or reports the unbound capacities as requires-bound, or
//! refuses the binding as `invalid_runtime_input`; the tests read that the
//! library carries each of those unchanged.

mod support {
    pub mod dynamic;
    pub mod handoff;
    pub mod projection;
    pub mod realizability;
}

use filament_exact::Cancel;
use filament_protocol::conformance::{engine_descriptor, ConformanceResult, FslProtocolCheck};
use filament_protocol::dispatch::{
    dispatch, BackendResult, Binding, ProtocolClaimKind, ProtocolOutcome, ProviderBoundary,
};
use filament_protocol::disposition::{ClaimSettlement, Disposition};
use filament_protocol::engine::{EngineRequest, EngineResult, FslEngine};
use fsl_analyze::analyze::ProtocolItem;
use fsl_analyze::model_check::{check_protocol, ModelCheckRefusal, ProtocolCheckRequest};
use fsl_analyze::protocol_analysis::ProtocolAnalysisArtifact;
use fsl_eval::simulation::protocol::ProtocolTransition;
use fsl_eval::simulation::protocol::{ProtocolRefusal, UnboundCapacity};
use fsl_replay::protocol_analysis::{AnalysisOutcome, UnsupportedSlug};
use fsl_replay::{
    ModelCheckItem, ModelCheckLimits, ModelCheckOutcome, ProtocolCertificateRequest, ReplayRefusal,
    Truth,
};
use fsl_route::ProviderOrigin;
use ix_trace_rs::trace;
use support::projection::{claim_of, Asked, CANCELLED};
use support::realizability::{channel, Realizability};

/// The ship branch sends twice on `Cap`, which blocks when full and which
/// nothing receives from: a deadlock at capacity one, none at capacity two.
fn blocking(capacity: &str) -> Realizability {
    let extra = channel("Cap", "Seller", "Warehouse", "PickMsg").replace(
        "capacity 4 overflow reject",
        &format!("capacity {capacity} overflow block"),
    );
    let ship = "send C1 via Cap as (c1: PickMsg) { true };\n\
        send C2 via Cap as (c2: PickMsg) { true };";
    Realizability::worked_with(&extra, ship, CANCELLED)
}

fn symbolic() -> Realizability {
    blocking("symbolic(Bound)")
}

fn item(fixture: &Realizability) -> ProtocolItem<'_> {
    ProtocolItem {
        request: ProtocolCheckRequest {
            subject: fixture.subject(),
            temporal_clock_selections: &[],
            item: ModelCheckItem::DeadlockFreedom,
            max_depth: None,
            max_store_buffer: None,
            max_messages: None,
            ignore_memory_constraints: false,
            max_live_instances: None,
            limits: ModelCheckLimits::default(),
        },
        certificate: ProtocolCertificateRequest {
            subject: fixture.subject(),
            temporal_clock_selections: &[],
            item: ModelCheckItem::DeadlockFreedom,
            max_store_buffer: None,
            max_messages: None,
            max_live_instances: None,
        },
        replay: Box::new(|_| Err(ReplayRefusal::SourceCount(0))),
    }
}

fn conformance(fixture: &Realizability) -> ProtocolOutcome<ConformanceResult, Binding<()>> {
    let mut boundary: ProviderBoundary<'_, Asked, ConformanceResult> = ProviderBoundary::new();
    boundary
        .register(
            engine_descriptor(ProviderOrigin::Linked).expect("FSL's own descriptor"),
            FslProtocolCheck::new(move |_: &Binding<()>| item(fixture)),
        )
        .expect("the registration is accepted");
    let asked = claim_of(
        ProtocolClaimKind::GlobalConformance,
        fixture.linked(),
        fixture.clause(),
        |form, name| fixture.named(form, name),
    );
    dispatch(&mut boundary, fixture.linked().declarations(), &asked)
}

fn realizability(fixture: &Realizability) -> ProtocolOutcome<EngineResult, Binding<()>> {
    let mut boundary: ProviderBoundary<'_, Asked, EngineResult> = ProviderBoundary::new();
    let engine = FslEngine::new(fixture.package(), |_: &Binding<()>| {
        EngineRequest::Realizability(Box::new(fixture.request()))
    })
    .expect("FSL's linked engines register");
    boundary
        .register(
            filament_protocol::engine::engine_descriptor(ProviderOrigin::Linked)
                .expect("FSL's own descriptor"),
            engine,
        )
        .expect("the registration is accepted");
    let asked = claim_of(
        ProtocolClaimKind::Realizability,
        fixture.linked(),
        fixture.clause(),
        |form, name| fixture.named(form, name),
    );
    dispatch(&mut boundary, fixture.linked().declarations(), &asked)
}

/// What the checker itself does with the fixture's deadlock-freedom item.
fn direct(
    fixture: &Realizability,
) -> Result<ModelCheckOutcome<ProtocolTransition>, ModelCheckRefusal> {
    check_protocol(item(fixture).request, &Cancel::new())
}

fn unbound_bound() -> Vec<UnboundCapacity> {
    vec![UnboundCapacity {
        name: "Bound".to_owned(),
        channels: vec!["Flow::Cap".to_owned()],
    }]
}

#[test]
#[trace("TC-209", "FR-058-AC-4", "FR-205-AC-6")]
fn an_unbound_symbolic_capacity_is_requires_bound_with_fsls_list_carried_unchanged() {
    let fixture = symbolic();
    // The checker itself explores nothing and names the capacity and the
    // channel whose queue uses it.
    let Err(ModelCheckRefusal::UnboundCapacity(direct)) = direct(&fixture) else {
        panic!("the checker reports the unbound capacity");
    };
    assert_eq!(direct, unbound_bound());
    let outcome = conformance(&fixture);
    assert_eq!(outcome.truth(), Truth::Unavailable);
    let ProtocolOutcome::Settled(ClaimSettlement::Disposition(Disposition::UnboundCapacity(found))) =
        outcome
    else {
        panic!("the typed requires-bound disposition (FR-058): {outcome:?}");
    };
    assert_eq!(found.capacities, direct);
}

#[test]
#[trace("TC-209", "FR-205-AC-7")]
fn a_bound_symbolic_capacity_is_decided_by_fsl_with_the_bound_and_its_record_carried_unchanged() {
    // At capacity one the second send waits forever; at two the run finishes.
    // The same protocol with the capacity written as a literal decides alike.
    for (value, literal) in [(1, "1"), (2, "2")] {
        let bound = symbolic().bound(&[("Bound", value)]);
        let written = blocking(literal);
        let (Ok(with_binding), Ok(with_literal)) = (direct(&bound), direct(&written)) else {
            panic!("FSL checks the bound subject");
        };
        assert_eq!(
            matches!(with_binding, ModelCheckOutcome::Violated { .. }),
            value == 1,
            "capacity {value}"
        );
        assert_eq!(
            std::mem::discriminant(&with_binding),
            std::mem::discriminant(&with_literal)
        );
    }
    let ProtocolOutcome::Settled(ClaimSettlement::Result { result, .. }) =
        conformance(&symbolic().bound(&[("Bound", 2)]))
    else {
        panic!("FSL decides a bound subject");
    };
    assert_eq!(result.truth(), Truth::Satisfied);
    // The dispatched record is the checker's own for the bound subject.
    let bound = symbolic().bound(&[("Bound", 2)]);
    let direct_item = item(&bound);
    let limits = direct_item.request.limits;
    let outcome = check_protocol(direct_item.request, &Cancel::new()).expect("checks");
    let settled = fsl_replay::settle_protocol_check(
        &outcome,
        &limits,
        &direct_item.certificate,
        &Cancel::new(),
        direct_item.replay,
    );
    assert_eq!(result.record().value, settled.value);
    assert_eq!(result.record().truth, settled.truth);
    assert_eq!(result.record().basis, settled.basis);
}

#[test]
#[trace("TC-209", "FR-205-AC-8")]
fn a_refused_capacity_binding_is_fsls_invalid_runtime_input_carried_unchanged() {
    for (bindings, cause) in [
        (&[("Nope", 3)][..], "unknown-member"),
        (&[("Bound", 2), ("Bound", 2)][..], "duplicate-member"),
    ] {
        let fixture = symbolic().bound(bindings);
        let Err(ModelCheckRefusal::Protocol(ProtocolRefusal::Capacity(refusal))) = direct(&fixture)
        else {
            panic!("FSL refuses the binding {bindings:?}");
        };
        assert_eq!(refusal.cause(), cause);
        let outcome = conformance(&fixture);
        assert_eq!(outcome.truth(), Truth::Unavailable);
        let ProtocolOutcome::Settled(ClaimSettlement::Disposition(Disposition::Refused(found))) =
            outcome
        else {
            panic!("FSL's refusal, carried: {outcome:?}");
        };
        assert_eq!(found.code.code(), "invalid_runtime_input");
        assert_eq!(found.code.cause(), cause);
    }
}

/// FSL's `bounded-static` realizability profile admits literal capacities
/// only, so FSL settles a symbolic one `unsupported` (`symbolic-queue-capacity`)
/// whatever the binding is; its bounded profile is not available yet. The
/// library carries FSL's record as it is and defines no cause of its own.
fn assert_unsupported_symbolic_queue_capacity(fixture: &Realizability) {
    let ProtocolOutcome::Settled(ClaimSettlement::Result { result, .. }) = realizability(fixture)
    else {
        panic!("FSL settles the item");
    };
    assert_eq!(result.truth(), Truth::Unavailable);
    let Some(ProtocolAnalysisArtifact::Realizability(found)) = result.artifact() else {
        panic!("a realizability result");
    };
    let AnalysisOutcome::Unsupported(cause) = &found.outcome else {
        panic!("FSL's unsupported outcome: {:?}", found.outcome);
    };
    assert_eq!(cause.slug, UnsupportedSlug::SymbolicQueueCapacity);
    assert_eq!(cause.detail.as_deref(), Some("Bound"));
}

#[test]
#[trace("TC-209", "FR-150-AC-21")]
fn realizability_over_an_unbound_symbolic_capacity_is_fsls_unsupported_cause_unchanged() {
    assert_unsupported_symbolic_queue_capacity(&symbolic());
}

#[test]
#[trace("TC-209", "FR-150-AC-21")]
fn realizability_over_a_bound_symbolic_capacity_is_still_fsls_unsupported_cause_unchanged() {
    assert_unsupported_symbolic_queue_capacity(&symbolic().bound(&[("Bound", 2)]));
}

#[test]
#[trace("TC-209", "FR-150-AC-21")]
fn realizability_over_a_refused_capacity_binding_carries_fsls_refusal() {
    let fixture = symbolic().bound(&[("Nope", 3)]);
    let outcome = realizability(&fixture);
    assert_eq!(outcome.truth(), Truth::Unavailable);
    let ProtocolOutcome::Settled(ClaimSettlement::Disposition(Disposition::Refused(found))) =
        outcome
    else {
        panic!("FSL's refusal, carried: {outcome:?}");
    };
    assert_eq!(found.code.code(), "invalid_runtime_input");
    assert_eq!(found.code.cause(), "unknown-member");
}
