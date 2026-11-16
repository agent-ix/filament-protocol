// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-147, TC-148, TC-202: `refinement` claims decided by this library over
//! the transition systems of real protocols, each built from FSL's protocol
//! step view of a checked-handoff directory (FR-301, FR-202, FR-302). No
//! system is stated by the caller.

mod support {
    pub mod assume_guarantee;
    pub mod dynamic;
    pub mod handoff;
    pub mod projection;
    pub mod realizability;
}

use filament_contract_model::CheckedNodeId;
use filament_protocol::closure::AssessmentExecution;
use filament_protocol::dispatch::{
    dispatch_claims, ClaimDisposition, DispositionRefusal, Fragment,
};
use filament_protocol::refinement::{
    decide, step_system, Action, AssessedSubject, Counterexample, Fault, Label, Party, Refinement,
    RefinementOutcome, RefinementRequest, Side, StateId, Transition, Undecidable, Verdict,
};
use fsl_eval::simulation::protocol::StepKind;
use fsl_replay::UnsteppedConstruct;
use fsl_replay::{SettlementBasis, Truth};
use ix_trace_rs::trace;
use support::assume_guarantee::{Boundary, Claims, Settled};
use support::projection::{assessed_root, CANCELLED, PICK, PICK2};
use support::realizability::Realizability;

const CONFORMANT: &str = "conformant";
const CHANNELS: [&str; 3] = ["Order", "Pick", "Cancelled"];

/// `Order`, then `Pick` or `Cancelled`.
fn wide() -> Realizability {
    Realizability::worked(PICK, CANCELLED)
}

/// `Order`, then `Cancelled` or nothing.
fn narrow() -> Realizability {
    Realizability::worked("", CANCELLED)
}

/// `Order`, then `Cancelled` or `Pick`, over channels that may lose a message.
fn lossy() -> Realizability {
    Realizability::delivering("at-most-once", "", "", PICK, CANCELLED)
}

/// After `Order`, a loop of `Pick` sends and receives that never ends. With
/// `Pick` hidden it is a divergence.
fn spinning() -> Realizability {
    let spin = "repeat Spin by Seller visible () while { true } sequence SpinBody { \
                send PickIt via Pick as (p: PickMsg) { true };\n\
                receive PickedUp via Pick of PickIt as (q: PickMsg) { true }; }";
    Realizability::worked(spin, CANCELLED)
}

fn fragment(of: &Realizability, names: &[&str]) -> Fragment {
    Fragment {
        channels: names.iter().map(|n| of.named("channel", n)).collect(),
        roles: Vec::new(),
    }
}

fn side(of: &Realizability, fragment: &Fragment, max_states: usize) -> Side {
    Side {
        node: of.clause().clone(),
        system: step_system(
            of.package(),
            of.clause(),
            &of.subject(),
            fragment,
            max_states,
        ),
    }
}

fn asked_over(
    implementation: &Realizability,
    specification: &Realizability,
    names: &[&str],
) -> RefinementRequest {
    let fragment = fragment(implementation, names);
    let draft = assessed_root();
    RefinementRequest {
        implementation: side(implementation, &fragment, 4096),
        specification: side(specification, &fragment, 4096),
        fragment,
        assumptions: Vec::new(),
        sources: vec![implementation.clause().clone()],
        max_states: 100_000,
        assessed_root: AssessedSubject {
            observation: draft.observation,
            object: draft.object,
        },
    }
}

fn asked(implementation: &Realizability, specification: &Realizability) -> RefinementRequest {
    asked_over(implementation, specification, &CHANNELS)
}

fn components<'a>(
    implementation: &'a Realizability,
    specification: &'a Realizability,
) -> [&'a filament_protocol::declarations::Declarations; 2] {
    [
        implementation.linked().declarations(),
        specification.linked().declarations(),
    ]
}

fn outcome(
    implementation: &Realizability,
    specification: &Realizability,
    request: &RefinementRequest,
) -> RefinementOutcome {
    decide(&components(implementation, specification), request)
}

fn decided(
    implementation: &Realizability,
    specification: &Realizability,
    request: &RefinementRequest,
) -> Refinement {
    match outcome(implementation, specification, request) {
        RefinementOutcome::Decided(found) => found,
        other => panic!("not decided: {other:?}"),
    }
}

/// The dispositions the dispatcher settles for `requests`.
fn settle(
    implementation: &Realizability,
    specification: &Realizability,
    requests: Vec<Claims>,
) -> Vec<Settled> {
    dispatch_claims(
        &mut Boundary::new(),
        implementation.linked().declarations(),
        &components(implementation, specification),
        &requests,
    )
}

fn settled_outcome(settled: Settled) -> RefinementOutcome {
    match settled {
        ClaimDisposition::Refinement(outcome) => outcome,
        other => panic!("not a refinement disposition: {other:?}"),
    }
}

/// A node of the package that neither protocol declares.
fn foreign(of: &Realizability) -> CheckedNodeId {
    let declarations = of.linked().declarations();
    of.package()
        .resolved_graph()
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .find(|node| !declarations.declares(node))
        .expect("a node the protocol does not declare")
}

fn faults(refinement: &Refinement) -> Vec<&Fault> {
    refinement
        .counterexamples
        .iter()
        .map(|c| &c.fault)
        .collect()
}

fn action(of: &Realizability, channel: &str, kind: StepKind) -> Action {
    Action {
        channel: of.named("channel", channel),
        kind,
    }
}

// ---------------------------------------------------------------- TC-147

#[test]
#[trace("TC-147", "FR-301-AC-1")]
fn a_refinement_neither_carries_nor_is_changed_by_a_conformance_result() {
    let (wide, narrow) = (wide(), narrow());
    let same = asked(&wide, &wide);
    let alone = outcome(&wide, &wide, &same);
    let beside = settle(
        &wide,
        &wide,
        vec![
            Claims::GlobalConformance(CONFORMANT),
            Claims::Refinement(same),
        ],
    );
    assert_eq!(beside[0], ClaimDisposition::GlobalConformance(CONFORMANT));
    assert_eq!(settled_outcome(beside[1].clone()), alone);
    // A refuted refinement leaves the conformant result as it was.
    let bad = asked(&wide, &narrow);
    let beside = settle(
        &wide,
        &narrow,
        vec![
            Claims::GlobalConformance(CONFORMANT),
            Claims::Refinement(bad),
        ],
    );
    assert_eq!(beside[0], ClaimDisposition::GlobalConformance(CONFORMANT));
}

#[test]
#[trace("TC-147", "FR-301-AC-2")]
fn a_replicated_role_in_the_fragment_is_unsupported_and_never_refines() {
    let pool = "role Warehouse each M::Depot from depots max 2 lifetime workflow;\n";
    let replicated = Realizability::delivering("exactly-once-premise", pool, "", PICK, CANCELLED);
    let plain = wide();
    let mut request = asked(&replicated, &plain);
    let warehouse = replicated.named("role", "Warehouse");
    assert_eq!(
        request.implementation.system.unsupported(),
        [UnsteppedConstruct::ReplicatedRole(warehouse.clone())]
    );
    assert!(request.implementation.system.transitions().is_empty());
    request.fragment.roles = vec![warehouse.clone()];
    let found = decided(&replicated, &plain, &request);
    assert_eq!(
        found.undecidable,
        vec![
            Undecidable::DynamicRole(warehouse.clone()),
            Undecidable::Construct(
                Party::Implementation,
                UnsteppedConstruct::ReplicatedRole(warehouse)
            ),
        ]
    );
    assert!(found.counterexamples.is_empty(), "nothing is invented");
    assert_eq!(found.verdict(), Verdict::Unsupported);
    assert_eq!(found.execution(), AssessmentExecution::Unsupported);
    assert_eq!(found.truth(), Truth::Unavailable);
    assert_eq!(found.basis(), SettlementBasis::Unavailable);
    // Beside a conformance result, which stays as it was.
    let beside = settle(
        &replicated,
        &plain,
        vec![
            Claims::GlobalConformance(CONFORMANT),
            Claims::Refinement(request),
        ],
    );
    assert_eq!(beside[0], ClaimDisposition::GlobalConformance(CONFORMANT));
}

#[test]
#[trace("TC-147", "FR-301-AC-2")]
fn a_channel_of_symbolic_capacity_in_the_fragment_is_unsupported() {
    let symbolic = "channel Sym from Buyer to Seller carries Ack ordering unordered \
                    delivery exactly-once-premise capacity symbolic(Bound) overflow reject;\n";
    let unbounded =
        Realizability::delivering("exactly-once-premise", "", symbolic, PICK, CANCELLED);
    let plain = wide();
    let request = asked_over(&unbounded, &plain, &["Order", "Pick", "Cancelled", "Sym"]);
    let found = decided(&unbounded, &plain, &request);
    assert!(found.undecidable.contains(&Undecidable::UnboundedQueue(
        unbounded.named("channel", "Sym")
    )));
    assert_eq!(found.verdict(), Verdict::Unsupported);
    assert_eq!(found.truth(), Truth::Unavailable);
}

#[test]
#[trace("TC-147", "FR-301-AC-3")]
fn a_refinement_is_decided_here_by_observable_traces() {
    let (wide, narrow) = (wide(), narrow());
    // Fewer traces refine; the same traces refine; more do not.
    for (implementation, specification, verdict) in [
        (&narrow, &wide, Verdict::Refines),
        (&wide, &wide, Verdict::Refines),
        (&wide, &narrow, Verdict::Refuted),
    ] {
        let request = asked(implementation, specification);
        assert_eq!(
            decided(implementation, specification, &request).verdict(),
            verdict
        );
    }
    let found = decided(&wide, &narrow, &asked(&wide, &narrow));
    assert_eq!(found.truth(), Truth::Violated);
}

#[test]
#[trace("TC-147", "FR-301-AC-3")]
fn a_protocol_refines_itself_through_the_step_view() {
    let (fixture, twin) = (wide(), wide());
    let request = asked(&fixture, &twin);
    let system = &request.implementation.system;
    assert!(system
        .transitions()
        .iter()
        .any(|step| matches!(step.label, Label::Observable(_))));
    assert!(system
        .transitions()
        .iter()
        .any(|step| step.label == Label::Internal));
    let found = decided(&fixture, &twin, &request);
    assert_eq!(found.verdict(), Verdict::Refines, "{found:?}");
    assert_eq!(found.truth(), Truth::Satisfied);
}

#[test]
#[trace("TC-147", "FR-301-AC-4")]
fn a_refinement_past_its_bound_is_incomplete_and_never_refines() {
    let (wide, narrow) = (wide(), narrow());
    let mut request = asked(&wide, &wide);
    for bound in [0, 1] {
        request.max_states = bound;
        let found = decided(&wide, &wide, &request);
        assert_eq!(found.verdict(), Verdict::Incomplete, "{bound}");
        assert_eq!(found.execution(), AssessmentExecution::ResourceIncomplete);
        assert_eq!(found.truth(), Truth::Unavailable);
        assert_eq!(found.basis(), SettlementBasis::Unavailable);
    }
    // A counterexample found before the bound is kept and refutes.
    let mut refuted = asked(&wide, &narrow);
    let full = decided(&wide, &narrow, &refuted);
    assert!(full.complete);
    refuted.max_states = full.explored - 4;
    let cut = decided(&wide, &narrow, &refuted);
    assert!(!cut.complete);
    assert_eq!(cut.verdict(), Verdict::Refuted);
    request.max_states = 100_000;
    assert_eq!(decided(&wide, &wide, &request).verdict(), Verdict::Refines);
}

#[test]
#[trace("TC-147", "FR-301-AC-4")]
fn a_step_view_past_its_state_limit_is_incomplete_and_never_refines() {
    let (fixture, twin) = (wide(), wide());
    let fragment = fragment(&fixture, &CHANNELS);
    let mut request = asked(&fixture, &twin);
    request.implementation = side(&fixture, &fragment, 3);
    assert!(request.implementation.system.incomplete().is_some());
    assert!(request.implementation.system.transitions().is_empty());
    let found = decided(&fixture, &twin, &request);
    assert_eq!(found.verdict(), Verdict::Incomplete, "{found:?}");
    assert!(found.counterexamples.is_empty(), "nothing is invented");
    assert_eq!(found.execution(), AssessmentExecution::ResourceIncomplete);
}

#[test]
#[trace("TC-147", "FR-301-AC-5")]
fn removing_or_substituting_a_component_refuses_only_that_disposition() {
    let (wide, narrow) = (wide(), narrow());
    let whole = asked(&wide, &narrow);
    let alone = outcome(&wide, &narrow, &whole);
    assert!(matches!(alone, RefinementOutcome::Decided(_)));
    let stranger = foreign(&wide);
    let mut cases: Vec<(&str, RefinementRequest)> = Vec::new();
    let mut vary = |name, change: &dyn Fn(&mut RefinementRequest)| {
        let mut broken = whole.clone();
        change(&mut broken);
        cases.push((name, broken));
    };
    vary("fragment removed", &|r| r.fragment = Fragment::default());
    vary("fragment substituted", &|r| {
        r.fragment.channels = vec![stranger.clone()];
    });
    vary("implementation substituted", &|r| {
        r.implementation.node = stranger.clone();
    });
    vary("assumption substituted", &|r| {
        r.assumptions = vec![stranger.clone()];
    });
    vary("source substituted", &|r| {
        r.sources = vec![stranger.clone()]
    });
    vary("label outside the fragment", &|r| {
        r.fragment.channels = vec![wide.named("channel", "Order")];
    });
    for (name, broken) in cases {
        let settled = settle(
            &wide,
            &narrow,
            vec![
                Claims::Refinement(broken),
                Claims::Refinement(whole.clone()),
            ],
        );
        assert!(
            matches!(
                settled_outcome(settled[0].clone()),
                RefinementOutcome::Refused(_)
            ),
            "{name}"
        );
        assert_eq!(settled_outcome(settled[1].clone()), alone, "{name}");
    }
    let mut unsourced = whole.clone();
    unsourced.sources.clear();
    assert!(matches!(
        outcome(&wide, &narrow, &unsourced),
        RefinementOutcome::Refused(DispositionRefusal::NoSourceIdentity)
    ));
}

#[test]
#[trace("TC-147", "FR-301-AC-5")]
fn a_label_outside_the_claims_fragment_is_refused_naming_its_channel() {
    let (wide, narrow) = (wide(), narrow());
    let mut request = asked(&wide, &narrow);
    // The implementation observes `Pick`; the claim's fragment does not.
    request.fragment.channels = vec![
        wide.named("channel", "Order"),
        wide.named("channel", "Cancelled"),
    ];
    assert!(matches!(
        outcome(&wide, &narrow, &request),
        RefinementOutcome::Refused(DispositionRefusal::UndeclaredLabel(node))
            if node == wide.named("channel", "Pick")
    ));
}

#[test]
#[trace("TC-147", "FR-301-AC-6")]
fn the_specification_is_explored_by_subset_and_internal_steps_are_hidden() {
    // The specification chooses between two `Pick` branches on its own; only
    // one of them continues with `Cancelled`. The implementation takes it.
    let both = format!("{PICK}\n{CANCELLED}");
    let specification = Realizability::worked(&both, PICK2);
    let implementation = Realizability::worked(&both, "");
    let request = asked(&implementation, &specification);
    assert_eq!(
        decided(&implementation, &specification, &request).verdict(),
        Verdict::Refines
    );
    // A channel outside the fragment is hidden, so protocols that differ only
    // there refine one another.
    let (wide, narrow) = (wide(), narrow());
    let request = asked_over(&wide, &narrow, &["Order"]);
    assert_eq!(
        decided(&wide, &narrow, &request).verdict(),
        Verdict::Refines
    );
}

#[test]
#[trace("TC-147", "FR-301-AC-6")]
fn a_deadlock_the_specification_also_reaches_on_one_branch_is_not_a_fault() {
    // The lossy protocol can lose a message and stop; the lossy specification
    // can too, so the implementation that follows that branch refines it.
    let lossy = lossy();
    let twin = self::lossy();
    let request = asked(&lossy, &twin);
    let found = decided(&lossy, &twin, &request);
    assert_eq!(found.verdict(), Verdict::Refines, "{found:?}");
}

#[test]
#[trace("TC-147", "FR-301-AC-7")]
fn only_sends_and_receives_on_the_fragments_channels_are_observable() {
    let fixture = wide();
    let all = asked(&fixture, &fixture);
    let narrowed = fragment(&fixture, &["Order"]);
    let system = step_system(
        fixture.package(),
        fixture.clause(),
        &fixture.subject(),
        &narrowed,
        4096,
    );
    let labels = |system: &filament_protocol::refinement::TransitionSystem| {
        system
            .transitions()
            .iter()
            .filter_map(|step| match &step.label {
                Label::Observable(action) => Some(action.channel.clone()),
                Label::Internal => None,
            })
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(labels(&system), [fixture.named("channel", "Order")].into());
    assert_eq!(
        labels(&all.implementation.system),
        CHANNELS
            .iter()
            .map(|name| fixture.named("channel", name))
            .collect()
    );
    // Hiding a channel hides its steps without removing them.
    assert_eq!(
        system.transitions().len(),
        all.implementation.system.transitions().len()
    );
}

#[test]
#[trace("TC-147", "FR-301-AC-7")]
fn a_send_and_a_receive_on_one_channel_are_different_actions() {
    let order = fragment(&wide(), &["Order"]);
    let fixture = wide();
    let system = step_system(
        fixture.package(),
        fixture.clause(),
        &fixture.subject(),
        &order,
        4096,
    );
    let kinds: std::collections::BTreeSet<_> = system
        .transitions()
        .iter()
        .filter_map(|step| match &step.label {
            Label::Observable(action) => Some(action.kind),
            Label::Internal => None,
        })
        .collect();
    assert_eq!(kinds, [StepKind::Send, StepKind::Receive].into());
    // On one channel, send-receive-send-receive is not send-send-receive-receive.
    let alternating = Realizability::worked(&format!("{PICK}\n{PICK2}"), "");
    let batched = Realizability::worked(
        "send PickIt via Pick as (p: PickMsg) { true };\n\
         send PickIt2 via Pick as (p2: PickMsg) { true };\n\
         receive PickedUp via Pick of PickIt as (q: PickMsg) { true };\n\
         receive PickedUp2 via Pick of PickIt2 as (q2: PickMsg) { true };",
        "",
    );
    let request = asked_over(&batched, &alternating, &["Pick"]);
    let found = decided(&batched, &alternating, &request);
    assert_eq!(found.verdict(), Verdict::Refuted, "{found:?}");
    assert!(found.counterexamples.iter().any(|found| matches!(
        &found.fault,
        Fault::ExtraTrace { action } if action.kind == StepKind::Send
    )));
    // The same channel order refines itself.
    let request = asked_over(&alternating, &alternating, &["Pick"]);
    assert_eq!(
        decided(&alternating, &alternating, &request).verdict(),
        Verdict::Refines
    );
}

// ---------------------------------------------------------------- TC-202

/// The extra trace of `wide` against `narrow`.
fn extra_trace() -> (Realizability, Realizability, RefinementRequest) {
    let (wide, narrow) = (wide(), narrow());
    let request = asked(&wide, &narrow);
    (wide, narrow, request)
}

#[test]
#[trace("TC-202", "FR-202-AC-1")]
fn an_extra_trace_yields_a_pair_of_runs() {
    let (wide, narrow, request) = extra_trace();
    let found = decided(&wide, &narrow, &request);
    assert_eq!(found.verdict(), Verdict::Refuted);
    assert!(!found.counterexamples.is_empty());
    for counter in &found.counterexamples {
        assert_eq!(
            counter.fault,
            Fault::ExtraTrace {
                action: action(&wide, "Pick", StepKind::Send)
            }
        );
        assert_eq!(
            counter.specification.trace,
            vec![
                action(&wide, "Order", StepKind::Send),
                action(&wide, "Order", StepKind::Receive)
            ]
        );
        assert!(counter.implementation.last().is_some_and(|last| {
            last.label == Label::Observable(action(&wide, "Pick", StepKind::Send))
        }));
        assert!(!counter.specification.reached.is_empty());
        assert!(counter.replay(&request));
    }
}

#[test]
#[trace("TC-202", "FR-202-AC-2")]
fn a_deadlock_where_the_specification_continues_yields_a_pair_of_runs() {
    let (lossy, reliable) = (lossy(), wide());
    let request = asked(&lossy, &reliable);
    let found = decided(&lossy, &reliable, &request);
    assert_eq!(found.verdict(), Verdict::Refuted, "{found:?}");
    let deadlocks: Vec<_> = found
        .counterexamples
        .iter()
        .filter(|counter| counter.fault == Fault::Deadlock)
        .collect();
    assert!(!deadlocks.is_empty(), "{found:?}");
    for counter in deadlocks {
        assert!(!counter.specification.reached.is_empty());
        assert!(counter.replay(&request));
    }
}

#[test]
#[trace("TC-202", "FR-202-AC-3")]
fn a_divergence_the_specification_lacks_yields_a_lasso() {
    let (spinning, narrow) = (spinning(), narrow());
    let request = asked_over(&spinning, &narrow, &["Order"]);
    let found = decided(&spinning, &narrow, &request);
    assert_eq!(found.verdict(), Verdict::Refuted, "{found:?}");
    let [counter] = found.counterexamples.as_slice() else {
        panic!("one counterexample: {:?}", found.counterexamples);
    };
    let Fault::Divergence { lasso } = &counter.fault else {
        panic!("a divergence: {counter:?}");
    };
    assert!(!lasso.is_empty());
    assert!(lasso.iter().all(|step| step.label == Label::Internal));
    assert!(counter.replay(&request));
}

#[test]
#[trace("TC-202", "FR-202-AC-4")]
fn behaviour_the_specification_shares_is_not_a_fault() {
    let (wide, narrow, spinning) = (wide(), narrow(), spinning());
    // Stopping in a final state is not a deadlock.
    let stops = asked(&narrow, &wide);
    assert_eq!(decided(&narrow, &wide, &stops).verdict(), Verdict::Refines);
    // Both stop where neither continues.
    let twin = self::wide();
    assert_eq!(
        decided(&wide, &twin, &asked(&wide, &twin)).verdict(),
        Verdict::Refines
    );
    // The specification loops on internal steps too.
    let looping = asked_over(&spinning, &spinning, &["Order"]);
    assert_eq!(
        decided(&spinning, &spinning, &looping).verdict(),
        Verdict::Refines
    );
}

#[test]
#[trace("TC-202", "FR-202-AC-5")]
fn each_verdict_reads_into_the_axes() {
    let (wide, narrow) = (wide(), narrow());
    let read = |implementation: &Realizability,
                specification: &Realizability,
                request: &RefinementRequest| {
        let r = decided(implementation, specification, request);
        (r.verdict(), r.execution(), r.truth(), r.basis())
    };
    assert_eq!(
        read(&wide, &wide, &asked(&wide, &wide)),
        (
            Verdict::Refines,
            AssessmentExecution::Completed,
            Truth::Satisfied,
            SettlementBasis::ClosedScope
        )
    );
    assert_eq!(
        read(&wide, &narrow, &asked(&wide, &narrow)),
        (
            Verdict::Refuted,
            AssessmentExecution::Completed,
            Truth::Violated,
            SettlementBasis::DecisiveCounterexample
        )
    );
    let mut incomplete = asked(&wide, &wide);
    incomplete.max_states = 1;
    assert_eq!(
        read(&wide, &wide, &incomplete),
        (
            Verdict::Incomplete,
            AssessmentExecution::ResourceIncomplete,
            Truth::Unavailable,
            SettlementBasis::Unavailable
        )
    );
}

#[test]
#[trace("TC-202", "FR-202-AC-6")]
fn the_precedence_does_not_depend_on_the_order_of_the_findings() {
    let (wide, lossy) = (wide(), lossy());
    // An extra trace and a deadlock are both kept.
    let narrow_spec = narrow();
    let mut request = asked(&lossy, &narrow_spec);
    let found = decided(&lossy, &narrow_spec, &request);
    assert_eq!(found.verdict(), Verdict::Refuted);
    let kinds: std::collections::HashSet<_> = faults(&found)
        .into_iter()
        .map(std::mem::discriminant)
        .collect();
    assert!(kinds.len() >= 2, "{:?}", found.counterexamples);
    assert_eq!(
        decided(&lossy, &narrow_spec, &request).counterexamples,
        found.counterexamples,
        "deciding again finds the same"
    );
    // Unsupported outranks the refutation and keeps it: a role the fragment
    // names that a component declares as replicated.
    let pool = "role Warehouse each M::Depot from depots max 2 lifetime workflow;\n";
    let replicated = Realizability::delivering("exactly-once-premise", pool, "", PICK, CANCELLED);
    request.fragment.roles = vec![replicated.named("role", "Warehouse")];
    let outranked = match decide(
        &[
            replicated.linked().declarations(),
            lossy.linked().declarations(),
            narrow_spec.linked().declarations(),
        ],
        &request,
    ) {
        RefinementOutcome::Decided(found) => found,
        other => panic!("not decided: {other:?}"),
    };
    assert_eq!(outranked.verdict(), Verdict::Unsupported);
    assert_eq!(outranked.counterexamples, found.counterexamples);
    // A refutation outranks an exhausted bound.
    let mut bounded = asked(&wide, &narrow_spec);
    bounded.max_states = 6;
    let cut = decided(&wide, &narrow_spec, &bounded);
    assert!(!cut.complete);
    assert_eq!(cut.verdict(), Verdict::Refuted);
}

#[test]
#[trace("TC-202", "FR-202-AC-7")]
fn a_counterexample_replays_and_an_altered_one_does_not() {
    let (wide, narrow, lossy, spinning) = (wide(), narrow(), lossy(), spinning());
    let extra = asked(&wide, &narrow);
    let deadlock = asked(&lossy, &wide);
    let divergence = asked_over(&spinning, &narrow, &["Order"]);
    // Each counterexample against a request whose specification shares the
    // behaviour does not replay.
    let others = [
        asked(&wide, &wide),
        asked(&lossy, &lossy),
        asked_over(&spinning, &spinning, &["Order"]),
    ];
    for ((asked, other), (implementation, specification)) in
        [extra.clone(), deadlock.clone(), divergence.clone()]
            .into_iter()
            .zip(others)
            .zip([(&wide, &narrow), (&lossy, &wide), (&spinning, &narrow)])
    {
        let found = decided(implementation, specification, &asked);
        let counter = found.counterexamples[0].clone();
        assert!(counter.replay(&asked));
        assert!(!counter.replay(&other));
        // The spec side is exactly what the trace reaches.
        let mut forged: Counterexample = counter.clone();
        forged.specification.reached.insert(StateId(99_999));
        assert!(!forged.replay(&asked));
        let mut forged = counter.clone();
        forged
            .specification
            .trace
            .push(action(&wide, "Cancelled", StepKind::Send));
        assert!(!forged.replay(&asked));
        // The implementation run must be a run.
        let mut forged = counter.clone();
        forged.implementation.push(Transition {
            from: StateId(99_998),
            label: Label::Internal,
            to: StateId(99_999),
        });
        assert!(!forged.replay(&asked));
        let mut forged = counter.clone();
        forged.implementation.clear();
        assert!(!forged.replay(&asked) || forged.implementation.is_empty());
    }
    // Fault-specific forgeries.
    let counter = decided(&wide, &narrow, &extra).counterexamples[0].clone();
    let mut forged = counter.clone();
    forged.fault = Fault::ExtraTrace {
        action: action(&wide, "Pick", StepKind::Receive),
    };
    assert!(!forged.replay(&extra));
    let mut forged = counter;
    forged.fault = Fault::Deadlock;
    assert!(!forged.replay(&extra));
    let counter = decided(&spinning, &narrow, &divergence).counterexamples[0].clone();
    let Fault::Divergence { lasso } = counter.fault.clone() else {
        panic!("a divergence");
    };
    let mut forged = counter.clone();
    forged.fault = Fault::Divergence { lasso: Vec::new() };
    assert!(!forged.replay(&divergence));
    let mut forged = counter;
    forged.fault = Fault::Divergence {
        lasso: lasso[..1].to_vec(),
    };
    assert!(lasso.len() < 2 || !forged.replay(&divergence));
}

// ----------------------------------------------------------------- TC-148

#[test]
#[trace("TC-148", "FR-302-AC-1")]
fn a_refuted_refinement_carries_the_witness_fsls_record_holds() {
    use fsl_replay::protocol_analysis::AssessedRoot;
    use fsl_replay::SeparatingWitnessRecord;

    let (wide, narrow) = (wide(), narrow());
    let request = asked(&wide, &narrow);
    let found = decided(&wide, &narrow, &request);
    let witness = found
        .witness
        .clone()
        .expect("a refuted refinement carries a witness");
    let root = AssessedRoot::new(Some(assessed_root())).expect("a root");
    let extra = found
        .counterexamples
        .iter()
        .filter(|c| matches!(c.fault, Fault::ExtraTrace { .. }))
        .min_by_key(|c| c.implementation.len())
        .expect("an extra trace");
    let Fault::ExtraTrace { action } = &extra.fault else {
        unreachable!()
    };
    let position = extra.implementation.len() - 1;
    let record: SeparatingWitnessRecord = root
        .record(&action.channel, position)
        .expect("FSL's record of the deciding step");
    assert!(witness.is(&record));
    assert_eq!(witness.deciding_node().as_ref(), Some(&action.channel));
    assert_eq!(witness.trace_position(), Some(position as u64));
    // The bytes are FSL's canonical record, read back strictly.
    let bytes = witness.to_bytes().expect("canonical bytes");
    assert_eq!(
        fsl_replay::RefinementWitness::from_bytes(&bytes).expect("strict read"),
        witness
    );
    // The dispatcher carries it unchanged.
    let settled = settle(&wide, &narrow, vec![Claims::Refinement(request)]);
    let RefinementOutcome::Decided(carried) = settled_outcome(settled[0].clone()) else {
        panic!("decided");
    };
    assert_eq!(carried.witness, Some(witness));
}

#[test]
#[trace("TC-148", "FR-302-AC-2")]
fn a_refuted_refinement_without_its_witness_is_refused() {
    let (wide, narrow) = (wide(), narrow());
    for blank in [
        AssessedSubject::default(),
        AssessedSubject {
            observation: None,
            ..asked(&wide, &narrow).assessed_root
        },
    ] {
        let mut request = asked(&wide, &narrow);
        request.assessed_root = blank;
        let settled = settle(&wide, &narrow, vec![Claims::Refinement(request)]);
        assert!(matches!(
            settled_outcome(settled[0].clone()),
            RefinementOutcome::Refused(DispositionRefusal::DecisiveWithoutWitness)
        ));
    }
    // A claim that is not refuted needs no root and carries no witness.
    let mut request = asked(&wide, &wide);
    request.assessed_root = AssessedSubject::default();
    let refines = decided(&wide, &wide, &request);
    assert_eq!(refines.verdict(), Verdict::Refines);
    assert_eq!(refines.witness, None);
    let mut bounded = asked(&wide, &wide);
    bounded.max_states = 1;
    let incomplete = decided(&wide, &wide, &bounded);
    assert_eq!(incomplete.verdict(), Verdict::Incomplete);
    assert_eq!(incomplete.witness, None);
}
