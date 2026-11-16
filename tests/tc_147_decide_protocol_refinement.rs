// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-147, TC-202, TC-204: `refinement` claims decided by this library over
//! two finite transition systems (FR-301, FR-202, FR-204).

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
    Counterexample, Fault, Label, Party, Refinement, RefinementOutcome, RefinementRequest, Side,
    StateId, Transition, TransitionSystem, Undecidable, Verdict,
};
use fsl_eval::simulation::protocol::Unsupported;
use fsl_replay::UnsteppedConstruct;
use fsl_replay::{SettlementBasis, Truth};
use ix_trace_rs::trace;
use support::assume_guarantee::{foreign, Boundary, Claims, Settled};
use support::dynamic::Dynamic;

const CONFORMANT: &str = "conformant";

fn state(n: u32) -> StateId {
    StateId(n)
}

/// An observable step from `from` to `to` on the channel `name`.
fn seen(dynamic: &Dynamic, from: u32, name: &str, to: u32) -> Transition {
    Transition {
        from: state(from),
        label: Label::Observable(dynamic.named("channel", name)),
        to: state(to),
    }
}

fn hidden(from: u32, to: u32) -> Transition {
    Transition {
        from: state(from),
        label: Label::Internal,
        to: state(to),
    }
}

fn system(finals: &[u32], transitions: Vec<Transition>) -> TransitionSystem {
    TransitionSystem {
        initial: state(0),
        final_states: finals.iter().copied().map(state).collect(),
        transitions,
        assumptions: Vec::new(),
        unsupported: Vec::new(),
        incomplete: None,
    }
}

fn request(
    dynamic: &Dynamic,
    implementation: TransitionSystem,
    specification: TransitionSystem,
) -> RefinementRequest {
    RefinementRequest {
        fragment: Fragment {
            channels: vec![dynamic.named("channel", "Head")],
            roles: vec![dynamic.named("role", "S")],
        },
        implementation: Side {
            node: dynamic.named("role", "S"),
            system: implementation,
        },
        specification: Side {
            node: dynamic.named("role", "R"),
            system: specification,
        },
        assumptions: Vec::new(),
        sources: vec![dynamic.clause().clone()],
        max_states: 1_000,
    }
}

fn settle(dynamic: &Dynamic, requests: Vec<Claims>) -> Vec<Settled> {
    dispatch_claims(
        &mut Boundary::new(),
        dynamic.linked().declarations(),
        &[dynamic.linked().declarations()],
        &requests,
    )
}

fn outcome(settled: Settled) -> RefinementOutcome {
    match settled {
        ClaimDisposition::Refinement(outcome) => outcome,
        other => panic!("not a refinement disposition: {other:?}"),
    }
}

fn decide(dynamic: &Dynamic, asked: &RefinementRequest) -> RefinementOutcome {
    outcome(
        settle(dynamic, vec![Claims::Refinement(asked.clone())])
            .into_iter()
            .next()
            .expect("one disposition"),
    )
}

fn decided(dynamic: &Dynamic, asked: &RefinementRequest) -> Refinement {
    match decide(dynamic, asked) {
        RefinementOutcome::Decided(refinement) => refinement,
        other => panic!("not decided: {other:?}"),
    }
}

fn faults(refinement: &Refinement) -> Vec<&Fault> {
    refinement
        .counterexamples
        .iter()
        .map(|c| &c.fault)
        .collect()
}

fn dynamic() -> Dynamic {
    Dynamic::with("workflow", 3)
}

/// The specification: `Head` then `Reject`, both final at the end.
fn spec_ab(d: &Dynamic) -> TransitionSystem {
    system(&[2], vec![seen(d, 0, "Head", 1), seen(d, 1, "Reject", 2)])
}

// ---------------------------------------------------------------- TC-147

#[test]
#[trace("TC-147", "FR-301-AC-1")]
fn a_refinement_neither_carries_nor_is_changed_by_a_conformance_result() {
    let d = dynamic();
    let asked = request(&d, spec_ab(&d), spec_ab(&d));
    let alone = decide(&d, &asked);
    let beside = settle(
        &d,
        vec![
            Claims::GlobalConformance(CONFORMANT),
            Claims::Refinement(asked),
        ],
    );
    assert_eq!(beside[0], ClaimDisposition::GlobalConformance(CONFORMANT));
    assert_eq!(outcome(beside[1].clone()), alone);
    // A refuted refinement leaves the conformant result as it was.
    let bad = request(&d, system(&[1], vec![seen(&d, 0, "Tail", 1)]), spec_ab(&d));
    let beside = settle(
        &d,
        vec![
            Claims::GlobalConformance(CONFORMANT),
            Claims::Refinement(bad),
        ],
    );
    assert_eq!(beside[0], ClaimDisposition::GlobalConformance(CONFORMANT));
}

#[test]
#[trace("TC-147", "FR-301-AC-2")]
fn unsupported_dynamics_never_refine_and_are_unsupported() {
    let d = dynamic();
    let identical = || request(&d, spec_ab(&d), spec_ab(&d));
    assert_eq!(decided(&d, &identical()).verdict(), Verdict::Refines);
    // A replicated role in the fragment.
    let mut dynamic_role = identical();
    dynamic_role.fragment.roles = vec![d.named("role", "R")];
    // A channel of symbolic capacity in the fragment.
    let mut unbounded = identical();
    unbounded.fragment.channels = vec![d.named("channel", "Symbolic")];
    // A construct the system builder could not step.
    let mut construct = identical();
    construct.specification.system.unsupported =
        vec![UnsteppedConstruct::Construct(Unsupported::SymbolicCapacity)];
    for (asked, expected) in [
        (dynamic_role, Undecidable::DynamicRole(d.named("role", "R"))),
        (
            unbounded,
            Undecidable::UnboundedQueue(d.named("channel", "Symbolic")),
        ),
        (
            construct,
            Undecidable::Construct(
                Party::Specification,
                UnsteppedConstruct::Construct(Unsupported::SymbolicCapacity),
            ),
        ),
    ] {
        let refinement = decided(&d, &asked);
        assert_eq!(refinement.undecidable, vec![expected]);
        assert_eq!(refinement.verdict(), Verdict::Unsupported);
        assert_eq!(refinement.execution(), AssessmentExecution::Unsupported);
        assert_eq!(refinement.truth(), Truth::Unavailable);
        assert_eq!(refinement.basis(), SettlementBasis::Unavailable);
    }
    // Beside a conformance result, which stays as it was.
    let mut unbounded = identical();
    unbounded.fragment.channels = vec![d.named("channel", "Symbolic")];
    let beside = settle(
        &d,
        vec![
            Claims::GlobalConformance(CONFORMANT),
            Claims::Refinement(unbounded),
        ],
    );
    assert_eq!(beside[0], ClaimDisposition::GlobalConformance(CONFORMANT));
}

#[test]
#[trace("TC-147", "FR-301-AC-3")]
fn a_refinement_is_decided_here_by_observable_traces() {
    let d = dynamic();
    // Fewer traces refine; the same traces refine; more do not.
    let prefix = system(&[1], vec![seen(&d, 0, "Head", 1)]);
    assert_eq!(
        decided(&d, &request(&d, prefix, spec_ab(&d))).verdict(),
        Verdict::Refines
    );
    assert_eq!(
        decided(&d, &request(&d, spec_ab(&d), spec_ab(&d))).verdict(),
        Verdict::Refines
    );
    let more = system(
        &[2],
        vec![
            seen(&d, 0, "Head", 1),
            seen(&d, 1, "Reject", 2),
            seen(&d, 1, "Tail", 2),
        ],
    );
    let refinement = decided(&d, &request(&d, more, spec_ab(&d)));
    assert_eq!(refinement.verdict(), Verdict::Refuted);
    assert_eq!(refinement.truth(), Truth::Violated);
}

#[test]
#[trace("TC-147", "FR-301-AC-4")]
fn a_refinement_past_its_bound_is_incomplete_and_never_refines() {
    let d = dynamic();
    let mut asked = request(&d, spec_ab(&d), spec_ab(&d));
    for bound in [0, 1] {
        asked.max_states = bound;
        let refinement = decided(&d, &asked);
        assert_eq!(refinement.verdict(), Verdict::Incomplete, "{bound}");
        assert_eq!(
            refinement.execution(),
            AssessmentExecution::ResourceIncomplete
        );
        assert_eq!(refinement.truth(), Truth::Unavailable);
        assert_eq!(refinement.basis(), SettlementBasis::Unavailable);
    }
    // A counterexample found before the bound is kept and refutes.
    let extra = system(&[1], vec![seen(&d, 0, "Tail", 1), seen(&d, 1, "Head", 1)]);
    let mut refuted = request(&d, extra, spec_ab(&d));
    refuted.max_states = 1;
    assert_eq!(decided(&d, &refuted).verdict(), Verdict::Refuted);
    asked.max_states = 3;
    assert_eq!(decided(&d, &asked).verdict(), Verdict::Refines);
}

#[test]
#[trace("TC-147", "FR-301-AC-5")]
fn removing_or_substituting_a_component_refuses_only_that_disposition() {
    let d = dynamic();
    let whole = request(&d, spec_ab(&d), spec_ab(&d));
    let alone = decide(&d, &whole);
    assert!(matches!(alone, RefinementOutcome::Decided(_)));
    let stranger = foreign(&d);
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
    vary("label substituted", &|r| {
        r.implementation.system.transitions[0].label = Label::Observable(stranger.clone());
    });
    for (name, broken) in cases {
        let settled = settle(
            &d,
            vec![
                Claims::Refinement(broken),
                Claims::Refinement(whole.clone()),
            ],
        );
        assert!(
            matches!(outcome(settled[0].clone()), RefinementOutcome::Refused(_)),
            "{name}"
        );
        assert_eq!(outcome(settled[1].clone()), alone, "{name}");
    }
    let mut unsourced = whole;
    unsourced.sources.clear();
    assert!(matches!(
        decide(&d, &unsourced),
        RefinementOutcome::Refused(DispositionRefusal::NoSourceIdentity)
    ));
    // The refusal names the node.
    let mut foreign_label = request(&d, spec_ab(&d), spec_ab(&d));
    foreign_label.specification.system.transitions[0].label = Label::Observable(stranger.clone());
    assert!(matches!(
        decide(&d, &foreign_label),
        RefinementOutcome::Refused(DispositionRefusal::UndeclaredLabel(node)) if node == stranger
    ));
}

#[test]
#[trace("TC-147", "FR-301-AC-6")]
fn the_specification_is_explored_by_subset_and_internal_steps_are_hidden() {
    let d = dynamic();
    // The specification chooses between two `Head` branches on its own; only
    // one of them continues with `Reject`. The implementation takes that one.
    let nondeterministic = system(
        &[2, 3],
        vec![
            seen(&d, 0, "Head", 1),
            seen(&d, 0, "Head", 3),
            seen(&d, 1, "Reject", 2),
        ],
    );
    assert_eq!(
        decided(&d, &request(&d, spec_ab(&d), nondeterministic)).verdict(),
        Verdict::Refines
    );
    // Internal steps on either side change no observable trace.
    let slow_impl = system(
        &[3],
        vec![
            hidden(0, 1),
            seen(&d, 1, "Head", 2),
            hidden(2, 4),
            seen(&d, 4, "Reject", 3),
        ],
    );
    let slow_spec = system(
        &[3],
        vec![
            hidden(0, 1),
            seen(&d, 1, "Head", 2),
            seen(&d, 2, "Reject", 3),
        ],
    );
    assert_eq!(
        decided(&d, &request(&d, slow_impl, slow_spec)).verdict(),
        Verdict::Refines
    );
}

// ---------------------------------------------------------------- TC-202

/// An extra trace: the implementation takes `Head` then `Tail`; the
/// specification allows `Head` then `Reject` only.
fn extra_trace(d: &Dynamic) -> RefinementRequest {
    request(
        d,
        system(&[2], vec![seen(d, 0, "Head", 1), seen(d, 1, "Tail", 2)]),
        spec_ab(d),
    )
}

/// A deadlock: the implementation stops after `Head` in a non-final state,
/// where the specification can still take `Reject`.
fn deadlock(d: &Dynamic) -> RefinementRequest {
    request(d, system(&[], vec![seen(d, 0, "Head", 1)]), spec_ab(d))
}

/// A divergence: after `Head` the implementation loops on internal steps; the
/// specification finishes.
fn divergence(d: &Dynamic) -> RefinementRequest {
    request(
        d,
        system(&[], vec![seen(d, 0, "Head", 1), hidden(1, 2), hidden(2, 1)]),
        system(&[1], vec![seen(d, 0, "Head", 1)]),
    )
}

#[test]
#[trace("TC-202", "FR-202-AC-1")]
fn an_extra_trace_yields_a_pair_of_runs() {
    let d = dynamic();
    let asked = extra_trace(&d);
    let refinement = decided(&d, &asked);
    assert_eq!(refinement.verdict(), Verdict::Refuted);
    let [counter] = refinement.counterexamples.as_slice() else {
        panic!("one counterexample: {:?}", refinement.counterexamples);
    };
    assert_eq!(
        counter.fault,
        Fault::ExtraTrace {
            action: d.named("channel", "Tail")
        }
    );
    assert_eq!(
        counter.implementation,
        vec![seen(&d, 0, "Head", 1), seen(&d, 1, "Tail", 2)]
    );
    assert_eq!(
        counter.specification.trace,
        vec![d.named("channel", "Head")]
    );
    assert_eq!(counter.specification.reached, [state(1)].into());
    assert!(counter.replay(&asked));
}

#[test]
#[trace("TC-202", "FR-202-AC-2")]
fn a_deadlock_where_the_specification_continues_yields_a_pair_of_runs() {
    let d = dynamic();
    let asked = deadlock(&d);
    let refinement = decided(&d, &asked);
    let [counter] = refinement.counterexamples.as_slice() else {
        panic!("one counterexample: {:?}", refinement.counterexamples);
    };
    assert_eq!(counter.fault, Fault::Deadlock);
    assert_eq!(counter.implementation, vec![seen(&d, 0, "Head", 1)]);
    assert_eq!(counter.specification.reached, [state(1)].into());
    assert!(counter.replay(&asked));
}

#[test]
#[trace("TC-202", "FR-202-AC-3")]
fn a_divergence_the_specification_lacks_yields_a_lasso() {
    let d = dynamic();
    let asked = divergence(&d);
    let refinement = decided(&d, &asked);
    let [counter] = refinement.counterexamples.as_slice() else {
        panic!("one counterexample: {:?}", refinement.counterexamples);
    };
    assert_eq!(
        counter.fault,
        Fault::Divergence {
            lasso: vec![hidden(1, 2), hidden(2, 1)]
        }
    );
    assert!(counter.replay(&asked));
}

#[test]
#[trace("TC-202", "FR-202-AC-4")]
fn behaviour_the_specification_shares_is_not_a_fault() {
    let d = dynamic();
    // Stopping in a final state is not a deadlock.
    let stops_final = request(&d, system(&[1], vec![seen(&d, 0, "Head", 1)]), spec_ab(&d));
    assert_eq!(decided(&d, &stops_final).verdict(), Verdict::Refines);
    // Both stop where neither continues.
    let both_stuck = request(&d, system(&[], vec![]), system(&[], vec![]));
    assert_eq!(decided(&d, &both_stuck).verdict(), Verdict::Refines);
    // The specification loops on internal steps too.
    let looping_spec = system(&[], vec![hidden(0, 1), hidden(1, 0)]);
    let looping_impl = system(&[], vec![hidden(0, 0)]);
    assert_eq!(
        decided(&d, &request(&d, looping_impl, looping_spec)).verdict(),
        Verdict::Refines
    );
}

#[test]
#[trace("TC-202", "FR-202-AC-5")]
fn each_verdict_reads_into_the_axes() {
    let d = dynamic();
    let read = |asked: &RefinementRequest| {
        let r = decided(&d, asked);
        (r.verdict(), r.execution(), r.truth(), r.basis())
    };
    assert_eq!(
        read(&request(&d, spec_ab(&d), spec_ab(&d))),
        (
            Verdict::Refines,
            AssessmentExecution::Completed,
            Truth::Satisfied,
            SettlementBasis::ClosedScope
        )
    );
    assert_eq!(
        read(&extra_trace(&d)),
        (
            Verdict::Refuted,
            AssessmentExecution::Completed,
            Truth::Violated,
            SettlementBasis::DecisiveCounterexample
        )
    );
    let mut incomplete = request(&d, spec_ab(&d), spec_ab(&d));
    incomplete.max_states = 1;
    assert_eq!(
        read(&incomplete),
        (
            Verdict::Incomplete,
            AssessmentExecution::ResourceIncomplete,
            Truth::Unavailable,
            SettlementBasis::Unavailable
        )
    );
    let mut unsupported = request(&d, spec_ab(&d), spec_ab(&d));
    unsupported.fragment.roles = vec![d.named("role", "R")];
    assert_eq!(
        read(&unsupported),
        (
            Verdict::Unsupported,
            AssessmentExecution::Unsupported,
            Truth::Unavailable,
            SettlementBasis::Unavailable
        )
    );
}

#[test]
#[trace("TC-202", "FR-202-AC-6")]
fn the_findings_and_precedence_do_not_depend_on_order() {
    let d = dynamic();
    // One system with all three faults: an extra trace, a deadlock after
    // `Head`, and a divergence after `Reject`.
    let implementation = system(
        &[],
        vec![
            seen(&d, 0, "Head", 1),
            seen(&d, 0, "Tail", 4),
            seen(&d, 1, "Reject", 2),
            hidden(2, 3),
            hidden(3, 2),
            seen(&d, 2, "Head", 5),
        ],
    );
    let mut asked = request(
        &d,
        implementation,
        system(
            &[3],
            vec![
                seen(&d, 0, "Head", 1),
                seen(&d, 1, "Reject", 2),
                seen(&d, 2, "Head", 3),
                seen(&d, 3, "Tail", 6),
            ],
        ),
    );
    let found = decided(&d, &asked);
    assert_eq!(found.verdict(), Verdict::Refuted);
    let kinds: Vec<_> = faults(&found)
        .into_iter()
        .map(std::mem::discriminant)
        .collect();
    assert!(kinds.len() >= 3, "{:?}", found.counterexamples);
    // Reversing every list changes nothing.
    asked.implementation.system.transitions.reverse();
    asked.specification.system.transitions.reverse();
    asked.implementation.system.final_states.reverse();
    assert_eq!(decided(&d, &asked).counterexamples, found.counterexamples);
    // Unsupported outranks the refutation and keeps it.
    asked.fragment.roles = vec![d.named("role", "R")];
    let outranked = decided(&d, &asked);
    assert_eq!(outranked.verdict(), Verdict::Unsupported);
    assert_eq!(outranked.counterexamples, found.counterexamples);
    // A refutation outranks an exhausted bound.
    asked.fragment.roles = vec![d.named("role", "S")];
    asked.max_states = 2;
    let bounded = decided(&d, &asked);
    assert!(!bounded.complete);
    assert_eq!(bounded.verdict(), Verdict::Refuted);
}

#[test]
#[trace("TC-202", "FR-202-AC-7")]
fn a_counterexample_replays_and_an_altered_one_does_not() {
    let d = dynamic();
    for asked in [extra_trace(&d), deadlock(&d), divergence(&d)] {
        let counter = decided(&d, &asked).counterexamples[0].clone();
        assert!(counter.replay(&asked));
        // Against a request whose systems differ the same value fails.
        let mut other = asked.clone();
        other.implementation.system.transitions.clear();
        assert!(!counter.replay(&other));
        // The spec side is exactly what the trace reaches.
        let mut forged: Counterexample = counter.clone();
        forged.specification.reached.insert(state(9));
        assert!(!forged.replay(&asked));
        let mut forged = counter.clone();
        forged.specification.trace.push(d.named("channel", "Tail"));
        assert!(!forged.replay(&asked));
        // The implementation run must be a run.
        let mut forged = counter.clone();
        forged.implementation.push(seen(&d, 7, "Head", 8));
        assert!(!forged.replay(&asked));
        let mut forged = counter.clone();
        forged.implementation.clear();
        let _ = forged.replay(&asked);
        assert!(!forged.replay(&asked) || forged.implementation.is_empty());
    }
    // Fault-specific forgeries.
    let asked = extra_trace(&d);
    let counter = decided(&d, &asked).counterexamples[0].clone();
    let mut forged = counter.clone();
    forged.fault = Fault::ExtraTrace {
        action: d.named("channel", "Head"),
    };
    assert!(!forged.replay(&asked));
    let mut forged = counter;
    forged.fault = Fault::Deadlock;
    assert!(!forged.replay(&asked));
    let asked = divergence(&d);
    let counter = decided(&d, &asked).counterexamples[0].clone();
    let mut forged = counter.clone();
    forged.fault = Fault::Divergence { lasso: Vec::new() };
    assert!(!forged.replay(&asked));
    let mut forged = counter;
    forged.fault = Fault::Divergence {
        lasso: vec![hidden(1, 2)],
    };
    assert!(!forged.replay(&asked));
}

// ---------------------------------------------------------------- TC-204

fn assuming(d: &Dynamic, implementation: &[&str], specification: &[&str]) -> RefinementRequest {
    let mut asked = request(d, spec_ab(d), spec_ab(d));
    let nodes = |names: &[&str]| -> Vec<CheckedNodeId> {
        names.iter().map(|n| d.named("channel", n)).collect()
    };
    asked.implementation.system.assumptions = nodes(implementation);
    asked.specification.system.assumptions = nodes(specification);
    asked.assumptions = vec![d.named("channel", "Head"), d.named("channel", "Tail")];
    asked
}

#[test]
#[trace("TC-204", "FR-204-AC-1")]
fn systems_resting_on_exactly_the_offered_assumptions_refine() {
    let d = dynamic();
    let refinement = decided(&d, &assuming(&d, &["Head", "Tail"], &["Head"]));
    assert!(refinement.stronger_assumptions.is_empty());
    assert_eq!(refinement.verdict(), Verdict::Refines);
}

#[test]
#[trace("TC-204", "FR-204-AC-2")]
fn an_implementation_resting_on_an_unoffered_assumption_is_refuted_naming_it() {
    let d = dynamic();
    let asked = assuming(&d, &["Head", "Block", "New"], &[]);
    let refinement = decided(&d, &asked);
    assert_eq!(refinement.verdict(), Verdict::Refuted);
    assert_eq!(refinement.truth(), Truth::Violated);
    let named: Vec<_> = refinement
        .stronger_assumptions
        .iter()
        .map(|s| (s.party, s.assumption.clone()))
        .collect();
    let mut expected = vec![
        (Party::Implementation, d.named("channel", "Block")),
        (Party::Implementation, d.named("channel", "New")),
    ];
    expected.sort();
    assert_eq!(named, expected);
    assert!(refinement
        .stronger_assumptions
        .iter()
        .all(|s| s.replay(&asked)));
}

#[test]
#[trace("TC-204", "FR-204-AC-3")]
fn offered_assumptions_no_system_used_change_nothing() {
    let d = dynamic();
    let refinement = decided(&d, &assuming(&d, &["Head"], &[]));
    assert_eq!(refinement.verdict(), Verdict::Refines);
    let none = decided(&d, &assuming(&d, &[], &[]));
    assert_eq!(none.verdict(), Verdict::Refines);
}

#[test]
#[trace("TC-204", "FR-204-AC-4")]
fn a_stronger_assumption_and_a_counterexample_are_both_kept() {
    let d = dynamic();
    let mut asked = assuming(&d, &["Block"], &[]);
    asked.implementation.system = {
        let mut bad = system(&[1], vec![seen(&d, 0, "Tail", 1)]);
        bad.assumptions = asked.implementation.system.assumptions.clone();
        bad
    };
    let refinement = decided(&d, &asked);
    assert_eq!(refinement.stronger_assumptions.len(), 1);
    assert_eq!(refinement.counterexamples.len(), 1);
    assert!(refinement.counterexamples[0].replay(&asked));
}

#[test]
#[trace("TC-204", "FR-204-AC-5")]
fn a_refinement_with_stronger_assumptions_leaves_other_claims_unchanged() {
    let d = dynamic();
    let sound = assuming(&d, &[], &[]);
    let alone = settle(
        &d,
        vec![
            Claims::GlobalConformance(CONFORMANT),
            Claims::Refinement(sound.clone()),
        ],
    );
    let beside = settle(
        &d,
        vec![
            Claims::GlobalConformance(CONFORMANT),
            Claims::Refinement(assuming(&d, &["Block"], &[])),
            Claims::Refinement(sound),
        ],
    );
    assert_eq!(beside[0], alone[0]);
    assert_eq!(beside[2], alone[1]);
    assert_ne!(beside[1], alone[1]);
}

#[test]
#[trace("TC-204", "FR-204-AC-6")]
fn a_specification_resting_on_an_unoffered_assumption_is_named_as_such() {
    let d = dynamic();
    let asked = assuming(&d, &[], &["Block"]);
    let refinement = decided(&d, &asked);
    assert_eq!(refinement.stronger_assumptions.len(), 1);
    assert_eq!(
        refinement.stronger_assumptions[0].party,
        Party::Specification
    );
    assert_eq!(refinement.verdict(), Verdict::Refuted);
}

#[test]
#[trace("TC-204", "FR-204-AC-7")]
fn a_stronger_assumption_fails_replay_when_altered() {
    let d = dynamic();
    let asked = assuming(&d, &["Block"], &[]);
    let found = decided(&d, &asked).stronger_assumptions[0].clone();
    assert!(found.replay(&asked));
    let mut forged = found.clone();
    forged.party = Party::Specification;
    assert!(!forged.replay(&asked));
    let mut forged = found.clone();
    forged.assumption = d.named("channel", "Head");
    assert!(!forged.replay(&asked));
    let mut forged = found.clone();
    forged.offered.push(d.named("channel", "Block"));
    assert!(!forged.replay(&asked));
    // Once the assumption is offered the finding no longer replays.
    let mut offered = asked;
    offered.assumptions.push(d.named("channel", "Block"));
    assert!(!found.replay(&offered));
}

// ------------------------------------------- systems from FSL's step view

mod viewed {
    use super::*;
    use filament_protocol::refinement::{decide, step_system};
    use fsl_replay::SeparatingWitnessRecord;
    use support::projection::{assessed_root, CANCELLED, PICK};
    use support::realizability::Realizability;

    const CHANNELS: [&str; 3] = ["Order", "Pick", "Cancelled"];

    pub fn asked(
        implementation: &Realizability,
        specification: &Realizability,
        max_states: usize,
    ) -> RefinementRequest {
        let fragment = Fragment {
            channels: CHANNELS
                .iter()
                .map(|name| implementation.named("channel", name))
                .collect(),
            roles: Vec::new(),
        };
        let side = |fixture: &Realizability| Side {
            node: fixture.clause().clone(),
            system: step_system(
                fixture.package(),
                fixture.clause(),
                &fixture.subject(),
                &fragment,
                max_states,
            ),
        };
        RefinementRequest {
            implementation: side(implementation),
            specification: side(specification),
            fragment: fragment.clone(),
            assumptions: Vec::new(),
            sources: vec![implementation.clause().clone()],
            max_states: 100_000,
        }
    }

    pub fn settled(
        implementation: &Realizability,
        specification: &Realizability,
        asked: &RefinementRequest,
    ) -> Refinement {
        let components = [
            implementation.linked().declarations(),
            specification.linked().declarations(),
        ];
        match decide(&components, asked) {
            RefinementOutcome::Decided(found) => found,
            other => panic!("not decided: {other:?}"),
        }
    }

    #[test]
    #[trace("TC-147", "FR-301-AC-3")]
    fn a_protocol_refines_itself_through_the_step_view() {
        let fixture = Realizability::worked(PICK, CANCELLED);
        let twin = Realizability::worked(PICK, CANCELLED);
        let asked = asked(&fixture, &twin, 4096);
        assert!(asked
            .implementation
            .system
            .transitions
            .iter()
            .any(|step| matches!(step.label, Label::Observable(_))));
        assert!(asked
            .implementation
            .system
            .transitions
            .iter()
            .any(|step| step.label == Label::Internal));
        let found = settled(&fixture, &twin, &asked);
        assert_eq!(found.verdict(), Verdict::Refines, "{found:?}");
        assert_eq!(found.truth(), Truth::Satisfied);
    }

    #[test]
    #[trace("TC-147", "FR-301-AC-3")]
    fn an_extra_message_the_specification_lacks_is_an_extra_trace() {
        let fixture = Realizability::worked(PICK, CANCELLED);
        let narrower = Realizability::worked("", CANCELLED);
        let asked = asked(&fixture, &narrower, 4096);
        let found = settled(&fixture, &narrower, &asked);
        assert_eq!(found.verdict(), Verdict::Refuted, "{found:?}");
        let pick = fixture.named("channel", "Pick");
        assert!(found.counterexamples.iter().any(|found| matches!(
            &found.fault,
            Fault::ExtraTrace { action } if *action == pick
        )));
        assert!(found
            .counterexamples
            .iter()
            .all(|counterexample| counterexample.replay(&asked)));
        // The other way round, the narrower protocol refines the wider.
        let reverse = self::asked(&narrower, &fixture, 4096);
        assert_eq!(
            settled(&narrower, &fixture, &reverse).verdict(),
            Verdict::Refines
        );
    }

    #[test]
    #[trace("TC-147", "FR-301-AC-3")]
    fn a_message_that_can_be_lost_is_a_deadlock_the_reliable_specification_lacks() {
        let reliable = Realizability::worked(PICK, CANCELLED);
        let lossy = Realizability::delivering("at-most-once", "", "", PICK, CANCELLED);
        let asked = asked(&lossy, &reliable, 4096);
        let found = settled(&lossy, &reliable, &asked);
        assert_eq!(found.verdict(), Verdict::Refuted, "{found:?}");
        assert!(faults(&found).contains(&&Fault::Deadlock), "{found:?}");
        assert!(found
            .counterexamples
            .iter()
            .all(|counterexample| counterexample.replay(&asked)));
    }

    #[test]
    #[trace("TC-147", "FR-301-AC-2")]
    fn a_replicated_role_in_the_protocol_is_unsupported_and_never_refines() {
        let pool = "role Warehouse each M::Depot from depots max 2 lifetime workflow;\n";
        let replicated =
            Realizability::delivering("exactly-once-premise", pool, "", PICK, CANCELLED);
        let plain = Realizability::worked(PICK, CANCELLED);
        let asked = asked(&replicated, &plain, 4096);
        let pool = replicated.named("role", "Warehouse");
        assert_eq!(
            asked.implementation.system.unsupported,
            vec![UnsteppedConstruct::ReplicatedRole(pool.clone())]
        );
        assert!(asked.implementation.system.transitions.is_empty());
        let found = settled(&replicated, &plain, &asked);
        assert_eq!(
            found.undecidable,
            vec![Undecidable::Construct(
                Party::Implementation,
                UnsteppedConstruct::ReplicatedRole(pool)
            )]
        );
        assert!(found.counterexamples.is_empty(), "nothing is invented");
        assert_eq!(found.verdict(), Verdict::Unsupported);
        assert_eq!(found.truth(), Truth::Unavailable);
    }

    #[test]
    #[trace("TC-147", "FR-301-AC-4")]
    fn a_step_view_past_its_state_limit_is_incomplete_and_never_refines() {
        let fixture = Realizability::worked(PICK, CANCELLED);
        let twin = Realizability::worked(PICK, CANCELLED);
        let asked = asked(&fixture, &twin, 3);
        assert!(asked.implementation.system.incomplete.is_some());
        assert!(asked.implementation.system.transitions.is_empty());
        let found = settled(&fixture, &twin, &asked);
        assert_eq!(found.verdict(), Verdict::Incomplete, "{found:?}");
        assert!(found.counterexamples.is_empty(), "nothing is invented");
        assert_eq!(found.execution(), AssessmentExecution::ResourceIncomplete);
    }

    #[test]
    #[trace("TC-147", "FR-301-AC-7")]
    fn only_sends_and_receives_on_the_fragments_channels_are_observable() {
        let fixture = Realizability::worked(PICK, CANCELLED);
        let all = asked(&fixture, &fixture, 4096);
        let mut narrowed = all.fragment.clone();
        narrowed.channels = vec![fixture.named("channel", "Order")];
        let system = step_system(
            fixture.package(),
            fixture.clause(),
            &fixture.subject(),
            &narrowed,
            4096,
        );
        let labels = |system: &TransitionSystem| {
            system
                .transitions
                .iter()
                .filter_map(|step| match &step.label {
                    Label::Observable(node) => Some(node.clone()),
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
            system.transitions.len(),
            all.implementation.system.transitions.len()
        );
    }

    #[test]
    #[trace("TC-148", "FR-302-AC-1")]
    fn a_refuted_refinement_carries_the_witness_fsls_record_holds() {
        let fixture = Realizability::worked(PICK, CANCELLED);
        let narrower = Realizability::worked("", CANCELLED);
        let asked = asked(&fixture, &narrower, 4096);
        let found = settled(&fixture, &narrower, &asked);
        let root = fsl_replay::protocol_analysis::AssessedRoot::new(Some(assessed_root()))
            .expect("a root");
        let witness = found
            .witness(&root)
            .expect("FSL writes the record")
            .expect("a refuted refinement carries a witness");
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
            .record(action, position)
            .expect("FSL's record of the deciding step");
        assert!(witness.is(&record));
        assert_eq!(witness.deciding_node().as_ref(), Some(action));
        assert_eq!(witness.trace_position(), Some(position as u64));
        // The bytes are FSL's canonical record, read back strictly.
        let bytes = witness.to_bytes().expect("canonical bytes");
        assert_eq!(
            fsl_replay::RefinementWitness::from_bytes(&bytes).expect("strict read"),
            witness
        );
    }

    #[test]
    #[trace("TC-148", "FR-302-AC-2")]
    fn a_refinement_that_is_not_refuted_carries_no_witness() {
        let fixture = Realizability::worked(PICK, CANCELLED);
        let twin = Realizability::worked(PICK, CANCELLED);
        let root = fsl_replay::protocol_analysis::AssessedRoot::new(Some(assessed_root()))
            .expect("a root");
        let refines = settled(&fixture, &twin, &asked(&fixture, &twin, 4096));
        assert_eq!(refines.witness(&root).expect("no record"), None);
        let bounded = settled(&fixture, &twin, &asked(&fixture, &twin, 3));
        assert_eq!(bounded.witness(&root).expect("no record"), None);
    }
}
