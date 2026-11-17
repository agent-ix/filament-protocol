// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-205: `global-conformance` claims dispatched to FSL's explicit-state
//! protocol checker over protocols written by FSL's checked-handoff writer
//! (FR-205). The decision, the proof certificate and the counterexample are
//! FSL's; the tests read what FSL settled.

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
use filament_protocol::disposition::{ClaimSettlement, Disposition, Warning};
use fsl_analyze::analyze::ProtocolItem;
use fsl_analyze::model_check::{check_protocol, ModelCheckRefusal, ProtocolCheckRequest};
use fsl_eval::simulation::protocol::ProtocolTransition;
use fsl_foundation::bound::DomainKind;
use fsl_replay::{
    settle_protocol_check, ModelCheckItem, ModelCheckLimits, ModelCheckOutcome,
    ProtocolCertificateRequest, ReplayRefusal, ResultLabel, SettlementBasis, TerminalValue, Truth,
};
use fsl_route::ProviderOrigin;
use fsl_semantics::check::Capability;
use ix_trace_rs::trace;
use support::projection::{claim_of, Asked, CANCELLED, PICK};
use support::realizability::Realizability;

type Boundary<'p> = ProviderBoundary<'p, Asked, ConformanceResult>;

/// A deadlock-freedom item over the fixture's protocol. `replay` stands for the
/// caller's FSL replay of a counterexample; a handoff carries no source to
/// recompile, so these tests supply a replay that refuses.
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

fn boundary(fixture: &Realizability) -> Boundary<'_> {
    let mut boundary = Boundary::new();
    boundary
        .register(
            engine_descriptor(ProviderOrigin::Linked).expect("FSL's own descriptor"),
            FslProtocolCheck::new(move |_: &Binding<()>| item(fixture)),
        )
        .expect("the registration is accepted");
    boundary
}

fn claim(fixture: &Realizability) -> filament_protocol::dispatch::ProtocolRequest<()> {
    claim_of(
        ProtocolClaimKind::GlobalConformance,
        fixture.linked(),
        fixture.clause(),
        |form, name| fixture.named(form, name),
    )
}

fn settle(fixture: &Realizability) -> ProtocolOutcome<ConformanceResult, Binding<()>> {
    dispatch(
        &mut boundary(fixture),
        fixture.linked().declarations(),
        &claim(fixture),
    )
}

fn result(outcome: ProtocolOutcome<ConformanceResult, Binding<()>>) -> ConformanceResult {
    match outcome {
        ProtocolOutcome::Settled(ClaimSettlement::Result { result, premises }) => {
            assert_eq!(premises.request.kind, ProtocolClaimKind::GlobalConformance);
            result
        }
        other => panic!("FSL's checker decides the claim: {other:?}"),
    }
}

#[test]
#[trace("TC-205", "FR-205-AC-1")]
fn a_deadlock_free_protocol_is_proved_by_the_checker_and_carried_satisfied() {
    let fixture = Realizability::worked(PICK, CANCELLED);
    let result = result(settle(&fixture));
    assert_eq!(result.truth(), Truth::Satisfied);
    assert_eq!(result.basis(), SettlementBasis::ClosedScope);
    let record = result.record();
    assert_eq!(record.label, ResultLabel::Proved);
    assert!(matches!(record.value, TerminalValue::Proved { .. }));
    assert!(record.counterexample.is_none());
}

/// The ship branch sends twice on a channel of capacity one that blocks, and
/// nothing receives: the second send waits forever.
fn deadlocked() -> Realizability {
    let extra = support::realizability::channel("Cap", "Seller", "Warehouse", "PickMsg")
        .replace("capacity 4 overflow reject", "capacity 1 overflow block");
    let ship = "send C1 via Cap as (c1: PickMsg) { true };\n\
        send C2 via Cap as (c2: PickMsg) { true };";
    Realizability::worked_with(&extra, ship, CANCELLED)
}

/// FSL's own settlement of the fixture's item, made by its two calls directly.
fn direct(fixture: &Realizability) -> fsl_replay::ModelCheckRecord<ProtocolTransition> {
    let direct = item(fixture);
    let limits = direct.request.limits;
    let outcome = check_protocol(direct.request, &Cancel::new()).expect("the subject checks");
    settle_protocol_check(
        &outcome,
        &limits,
        &direct.certificate,
        &Cancel::new(),
        direct.replay,
    )
}

fn assert_same_record(
    carried: &fsl_replay::ModelCheckRecord<ProtocolTransition>,
    direct: &fsl_replay::ModelCheckRecord<ProtocolTransition>,
) {
    assert_eq!(carried.value, direct.value);
    assert_eq!(carried.label, direct.label);
    assert_eq!(carried.basis, direct.basis);
    assert_eq!(carried.truth, direct.truth);
    assert_eq!(carried.limits, direct.limits);
    assert_eq!(
        carried.counterexample.is_some(),
        direct.counterexample.is_some()
    );
}

#[test]
#[trace("TC-205", "FR-205-AC-2")]
fn the_dispatched_record_is_the_checkers_own_record_unchanged() {
    let fixture = Realizability::worked(PICK, CANCELLED);
    let dispatched = result(settle(&fixture));
    assert_same_record(dispatched.record(), &direct(&fixture));
}

#[test]
#[trace("TC-205", "FR-205-AC-3")]
fn a_deadlock_whose_counterexample_fsl_cannot_replay_is_not_reported_violated() {
    let fixture = deadlocked();
    // The checker itself finds the deadlock.
    let found = check_protocol(item(&fixture).request, &Cancel::new()).expect("the subject checks");
    assert!(
        matches!(found, ModelCheckOutcome::Violated { .. }),
        "the checker finds the deadlock: {found:?}"
    );
    let result = result(settle(&fixture));
    // What FSL settles from a refused replay, carried unchanged.
    let settled = direct(&fixture);
    assert_same_record(result.record(), &settled);
    assert_ne!(settled.truth, Truth::Violated);
    assert_ne!(result.truth(), Truth::Violated);
    assert!(result.record().counterexample.is_none());
    assert_ne!(result.record().label, ResultLabel::Refuted);
}

#[test]
#[trace("TC-205", "FR-205-AC-4")]
fn an_empty_registry_settles_the_claim_unsupported_naming_the_capability() {
    let fixture = Realizability::worked(PICK, CANCELLED);
    let mut empty = Boundary::new();
    let outcome = dispatch(
        &mut empty,
        fixture.linked().declarations(),
        &claim(&fixture),
    );
    assert_eq!(outcome.truth(), Truth::Unavailable);
    let ProtocolOutcome::Settled(ClaimSettlement::Disposition(Disposition::Unsupported(found))) =
        outcome
    else {
        panic!("an unsupported disposition");
    };
    assert_eq!(
        found.warnings,
        vec![Warning::UnservedCapability {
            kind: Capability::TemporalSatisfaction,
            backend: None,
        }]
    );
}

#[test]
#[trace("TC-205", "FR-205-AC-5")]
fn an_unbounded_quantity_root_is_reported_requires_bound_by_the_real_checker_as_the_typed_disposition(
) {
    // The recipe model as FSL ships it: `Total` and `Amount` are unbounded
    // quantities (FSL FR-120), and neither the fixture nor this test bounds
    // them (the in-memory rewrite of the other fixtures is not applied).
    let fixture = Realizability::unbounded();
    // The checker itself refuses the subject: it requires a bound, and names
    // the unbounded roots by key and kind.
    let Err(ModelCheckRefusal::RequiresBound(direct)) =
        check_protocol(item(&fixture).request, &Cancel::new())
    else {
        panic!("the checker refuses an unbounded root as requiring a bound");
    };
    let direct_domains: Vec<_> = direct.domains.iter().collect();
    // FSL names the unbounded roots of `Workflow` and `Depot` (the quantity
    // fields, which FSL classifies as `Integer` domains), by key and kind.
    assert!(!direct_domains.is_empty());
    assert!(direct_domains
        .iter()
        .all(|(_, kind)| *kind == DomainKind::Integer));
    let outcome = settle(&fixture);
    assert_eq!(outcome.truth(), Truth::Unavailable);
    let ProtocolOutcome::Settled(ClaimSettlement::Disposition(Disposition::RequiresBound(found))) =
        outcome
    else {
        panic!("the typed requires-bound disposition (FR-058): {outcome:?}");
    };
    // FSL's outcome, domain kinds and keys unchanged; no bound chosen.
    assert_eq!(found.outcome, direct);
    assert_eq!(found.domains().collect::<Vec<_>>(), direct_domains);
    // The same protocol over the bounded fixture is decided, so the refusal is
    // the model's unbounded root and nothing else.
    let bounded = Realizability::worked(PICK, CANCELLED);
    assert_eq!(result(settle(&bounded)).truth(), Truth::Satisfied);
}
