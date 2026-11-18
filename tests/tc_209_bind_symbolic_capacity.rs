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
use filament_protocol::disposition::{ClaimSettlement, Disposition, UnsupportedCause};
use filament_protocol::engine::{EngineRequest, EngineResult, FslEngine};
use fsl_analyze::analyze::ProtocolItem;
use fsl_analyze::model_check::{check_protocol, ModelCheckRefusal, ProtocolCheckRequest};
use fsl_analyze::protocol_analysis::ProtocolAnalysisArtifact;
use fsl_eval::simulation::protocol::ProtocolTransition;
use fsl_eval::simulation::protocol::{ProtocolRefusal, UnboundCapacity};
use fsl_replay::protocol_analysis::{AnalysisOutcome, InconclusiveCause, UnsupportedSlug};
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
    realizability_under(fixture, "bounded-static")
}

/// The realizability claim under the strategy profile FSL names `profile`.
fn realizability_under(
    fixture: &Realizability,
    profile: &str,
) -> ProtocolOutcome<EngineResult, Binding<()>> {
    let mut boundary: ProviderBoundary<'_, Asked, EngineResult> = ProviderBoundary::new();
    let engine = FslEngine::new(fixture.package(), |_: &Binding<()>| {
        let mut request = fixture.request();
        request.strategy_profile = Some(profile.to_owned());
        EngineRequest::Realizability(Box::new(request))
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
#[trace("TC-209", "FR-058-AC-4")]
fn beside_an_unbounded_root_fsls_requires_bound_names_the_capacity_with_the_domains() {
    // The recipe model as FSL ships it (unbounded `Total` and `Amount`) with a
    // symbolic channel added and no binding for it.
    let extra = channel("Cap", "Seller", "Warehouse", "PickMsg").replace(
        "capacity 4 overflow reject",
        "capacity symbolic(Bound) overflow block",
    );
    let fixture = Realizability::unbounded_with(&extra);
    let Err(ModelCheckRefusal::RequiresBound(direct)) = direct(&fixture) else {
        panic!("the checker reports a requires-bound outcome");
    };
    assert!(!direct.capacities.is_empty());
    assert!(direct.domains.iter().next().is_some());
    let outcome = conformance(&fixture);
    assert_eq!(outcome.truth(), Truth::Unavailable);
    let ProtocolOutcome::Settled(ClaimSettlement::Disposition(Disposition::RequiresBound(found))) =
        outcome
    else {
        panic!("the typed requires-bound disposition (FR-058): {outcome:?}");
    };
    assert_eq!(found.outcome, direct);
    assert_eq!(found.outcome.capacities, unbound_bound());
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
    assert_eq!(*result.record(), settled);
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

/// The outcome FSL's engine settles for the realizability claim of `$fixture`,
/// carried by the dispatch.
macro_rules! realizability_item {
    ($fixture:expr, $profile:expr) => {{
        let ProtocolOutcome::Settled(ClaimSettlement::Result { result, .. }) =
            realizability_under($fixture, $profile)
        else {
            panic!("FSL settles the item");
        };
        let Some(ProtocolAnalysisArtifact::Realizability(found)) = result.artifact() else {
            panic!("a realizability result");
        };
        (result.truth(), found.outcome.clone())
    }};
}

/// FSL's `bounded-static` profile admits literal capacities only, so FSL
/// settles a symbolic one `unsupported` whatever the binding is; the library
/// carries that record and defines no cause of its own.
#[test]
#[trace("TC-209", "FR-150-AC-21")]
fn the_bounded_static_profile_is_fsls_unsupported_for_a_symbolic_capacity_bound_or_not() {
    for fixture in [symbolic(), symbolic().bound(&[("Bound", 2)])] {
        let (truth, outcome) = realizability_item!(&fixture, "bounded-static");
        assert_eq!(truth, Truth::Unavailable);
        let AnalysisOutcome::Unsupported(cause) = outcome else {
            panic!("FSL's unsupported outcome: {outcome:?}");
        };
        assert_eq!(cause.slug, UnsupportedSlug::SymbolicQueueCapacity);
        assert_eq!(cause.detail.as_deref(), Some("Bound"));
    }
}

#[test]
#[trace("TC-209", "FR-150-AC-21")]
fn realizability_over_an_unbound_symbolic_capacity_is_fsls_premise_not_stated_unchanged() {
    let (truth, outcome) = realizability_item!(&symbolic(), "bounded");
    assert_eq!(truth, Truth::Pending);
    let AnalysisOutcome::Inconclusive(cause) = outcome else {
        panic!("FSL's inconclusive outcome: {outcome:?}");
    };
    assert_eq!(
        cause,
        InconclusiveCause::PremiseNotStated {
            member: "capacities"
        }
    );
}

#[test]
#[trace("TC-209", "FR-150-AC-21")]
fn realizability_over_a_bound_symbolic_capacity_is_decided_by_fsl() {
    let (_, outcome) = realizability_item!(&symbolic().bound(&[("Bound", 2)]), "bounded");
    assert!(
        !matches!(
            outcome,
            AnalysisOutcome::Inconclusive(_) | AnalysisOutcome::Unsupported(_)
        ),
        "FSL decides with the bound: {outcome:?}"
    );
    // As FSL decides the same protocol with a literal capacity.
    let (_, literal) = realizability_item!(&blocking("2"), "bounded");
    assert_eq!(
        std::mem::discriminant(&outcome),
        std::mem::discriminant(&literal)
    );
}

/// Dispatch the `finite-replay` claim of a supplied trace to FSL's engine.
fn finite_replay(
    fixture: &Realizability,
    steps: &[serde_json::Value],
) -> ProtocolOutcome<EngineResult, Binding<()>> {
    let (snapshots, trace) = fixture.with_trace("t", steps);
    let mut boundary: ProviderBoundary<'_, Asked, EngineResult> = ProviderBoundary::new();
    let engine = FslEngine::new(fixture.package(), |_: &Binding<()>| {
        EngineRequest::FiniteReplay(Box::new(fixture.replay_request(&snapshots, trace.clone())))
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
        ProtocolClaimKind::FiniteReplay,
        fixture.linked(),
        fixture.clause(),
        |form, name| fixture.named(form, name),
    );
    dispatch(&mut boundary, fixture.linked().declarations(), &asked)
}

#[test]
#[trace("TC-209", "FR-058-AC-6")]
fn finite_replay_over_a_symbolic_capacity_is_fsls_unbounded_extent_carried_unchanged() {
    // FSL's finite replay request takes no capacity binding, so a symbolic
    // capacity is an unbounded root of the replay whatever the fixture binds.
    let fixture = symbolic().bound(&[("Bound", 2)]);
    let steps = fixture.trace_steps(3);
    let outcome = finite_replay(&fixture, &steps);
    assert_eq!(outcome.truth(), Truth::Unavailable, "{outcome:?}");
    let ProtocolOutcome::Settled(ClaimSettlement::Disposition(Disposition::Unsupported(found))) =
        outcome
    else {
        panic!("FSL's outcome, carried as a typed disposition: {outcome:?}");
    };
    // FSL's analyze maps its engine's requires-bound outcome to
    // `unsupported_projection`/`unbounded-extent`; the library carries that
    // code and cause as FSL gives them.
    let UnsupportedCause::RequestedCapability(code) = &found.cause else {
        panic!("a requested-capability cause: {:?}", found.cause);
    };
    assert_eq!(code.code(), "unsupported_projection");
    assert_eq!(code.cause(), "unbounded-extent");
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
