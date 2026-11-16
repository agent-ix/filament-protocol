// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-200 to TC-204: `composition` claims decided by this library (FR-200 to
//! FR-204).

mod support {
    pub mod assume_guarantee;
    pub mod dynamic;
    pub mod handoff;
}

use filament_contract_model::CheckedNodeId;
use filament_protocol::assume_guarantee::PremiseList;
use filament_protocol::closure::AssessmentExecution;
use filament_protocol::composition::{
    cycle_members, Composition, CompositionOutcome, DischargedBy, PremiseRef, Unavailable, Verdict,
};
use filament_protocol::dispatch::{dispatch_claims, ClaimDisposition, DispositionRefusal};
use fsl_replay::planner::ClauseForm;
use fsl_replay::{ItemLabel, SettlementBasis, Truth};
use ix_trace_rs::trace;
use support::assume_guarantee::{
    foreign, guarantee, request, Boundary, Claims, Entry, Req, Settled,
};
use support::dynamic::Dynamic;

const CONFORMANT: &str = "conformant";

type Decided = CompositionOutcome<Entry>;

/// Settle `requests` through the one dispatch path, in order.
fn settle(dynamic: &Dynamic, requests: Vec<Claims>) -> Vec<Settled> {
    dispatch_claims(
        &mut Boundary::new(),
        dynamic.linked().declarations(),
        &[dynamic.linked().declarations()],
        &requests,
    )
}

fn first(settled: Vec<Settled>) -> Settled {
    settled.into_iter().next().expect("one disposition")
}

/// A composition claim, decided alone.
fn compose(dynamic: &Dynamic, asked: &Req) -> Decided {
    composed(first(settle(
        dynamic,
        vec![Claims::Composition(asked.clone())],
    )))
}

fn composed(disposition: Settled) -> Decided {
    match disposition {
        ClaimDisposition::Composition(outcome) => outcome,
        other => panic!("not a composition disposition: {other:?}"),
    }
}

fn decided(outcome: Decided) -> Composition<Entry> {
    match outcome {
        CompositionOutcome::Decided(composition) => composition,
        other => panic!("not decided: {other:?}"),
    }
}

fn composition_refusal(outcome: Decided) -> DispositionRefusal {
    match outcome {
        CompositionOutcome::Refused(refusal) => refusal,
        other => panic!("not refused: {other:?}"),
    }
}

fn node(dynamic: &Dynamic, name: &str) -> CheckedNodeId {
    dynamic.named("channel", name)
}

/// The undischarged cause of the premise at `list` and `index`.
fn cause(composition: &Composition<Entry>, list: PremiseList, index: usize) -> Option<Unavailable> {
    let premise = PremiseRef { list, index };
    composition
        .undischarged
        .iter()
        .find(|u| u.premise == premise)
        .map(|u| u.why.clone())
}

// ---------------------------------------------------------------- TC-200

#[test]
#[trace("TC-200", "FR-200-AC-1")]
fn a_composition_is_decided_here_with_a_discharge_record() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic);
    let composition = decided(compose(&dynamic, &asked));
    assert_eq!(composition.verdict(), Verdict::Proved);
    assert_eq!(composition.discharges.len(), 4);
    assert!(composition.undischarged.is_empty());
    assert_eq!(composition.binding.request, asked);
}

#[test]
#[trace("TC-200", "FR-200-AC-2")]
fn a_composition_needs_no_backend_and_is_never_unsupported() {
    let dynamic = Dynamic::with("workflow", 3);
    // The boundary serves no capability, and the claim is decided all the
    // same, even with every premise unmet.
    let mut asked = request(&dynamic);
    asked.guarantees.clear();
    assert_eq!(
        decided(compose(&dynamic, &asked)).verdict(),
        Verdict::Unproved
    );
}

#[test]
#[trace("TC-200", "FR-200-AC-3")]
fn a_refused_composition_leaves_another_claim_unchanged() {
    let dynamic = Dynamic::with("workflow", 3);
    let conformance = || Claims::GlobalConformance(CONFORMANT);
    let sound = request(&dynamic);
    let alone = settle(
        &dynamic,
        vec![conformance(), Claims::Composition(sound.clone())],
    );
    let mut refusing = request(&dynamic);
    refusing.sources.clear();
    let together = settle(
        &dynamic,
        vec![
            conformance(),
            Claims::Composition(refusing),
            Claims::Composition(sound),
        ],
    );
    assert!(matches!(
        composed(together[1].clone()),
        CompositionOutcome::Refused(_)
    ));
    assert_eq!(together[0], alone[0]);
    assert_eq!(together[2], alone[1]);
}

// ---------------------------------------------------------------- TC-200

#[test]
#[trace("TC-200", "FR-200-AC-4")]
fn a_conformance_result_neither_decides_nor_is_decided_by_a_composition() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic);
    let alone = compose(&dynamic, &asked);
    let beside = settle(
        &dynamic,
        vec![
            Claims::GlobalConformance(CONFORMANT),
            Claims::Composition(asked.clone()),
        ],
    );
    assert_eq!(beside[0], ClaimDisposition::GlobalConformance(CONFORMANT));
    assert_eq!(composed(beside[1].clone()), alone);
    // A composition that is not proved stays so beside a conformant result.
    let mut unproved = asked;
    unproved.guarantees[0].label = ItemLabel::Inconclusive;
    let beside = settle(
        &dynamic,
        vec![
            Claims::GlobalConformance(CONFORMANT),
            Claims::Composition(unproved),
        ],
    );
    assert_eq!(
        decided(composed(beside[1].clone())).verdict(),
        Verdict::Unproved
    );
}

/// One claim per component, each with that component removed or substituted.
fn broken_components(dynamic: &Dynamic, whole: &Req) -> Vec<(&'static str, Req)> {
    let stranger = foreign(dynamic);
    let mut cases = Vec::new();
    let mut vary = |name, change: &dyn Fn(&mut Req)| {
        let mut broken = whole.clone();
        change(&mut broken);
        cases.push((name, broken));
    };
    vary("fragment removed", &|r| r.fragment = Default::default());
    vary("fragment substituted", &|r| {
        r.fragment.channels = vec![stranger.clone()];
    });
    vary("assumption substituted", &|r| {
        r.assumptions = vec![stranger.clone()];
    });
    vary("source substituted", &|r| {
        r.sources = vec![stranger.clone()]
    });
    vary("guarantee substituted", &|r| {
        r.guarantees[0].id = stranger.clone();
    });
    vary("proof edge substituted", &|r| {
        r.guarantees[0].uses = vec![stranger.clone()];
    });
    cases
}

#[test]
#[trace("TC-200", "FR-200-AC-5")]
fn removing_or_substituting_a_component_refuses_only_that_disposition() {
    let dynamic = Dynamic::with("workflow", 3);
    let whole = request(&dynamic);
    let alone = compose(&dynamic, &whole);
    assert_eq!(decided(alone.clone()).verdict(), Verdict::Proved);
    for (name, broken) in broken_components(&dynamic, &whole) {
        let settled = settle(
            &dynamic,
            vec![
                Claims::Composition(broken),
                Claims::Composition(whole.clone()),
            ],
        );
        let first = composed(settled[0].clone());
        assert!(matches!(first, CompositionOutcome::Refused(_)), "{name}");
        assert_eq!(composed(settled[1].clone()), alone, "{name}");
    }
    // A decisive verdict retaining no source identity is refused.
    let mut unsourced = whole;
    unsourced.sources.clear();
    assert_eq!(
        composition_refusal(compose(&dynamic, &unsourced)),
        DispositionRefusal::NoSourceIdentity
    );
}

#[test]
#[trace("TC-200", "FR-200-AC-5")]
fn an_empty_subject_list_or_fragment_is_refused_by_name() {
    let dynamic = Dynamic::with("workflow", 3);
    let whole = request(&dynamic);
    let mut no_subjects = whole.clone();
    no_subjects.subjects.clear();
    assert_eq!(
        composition_refusal(compose(&dynamic, &no_subjects)),
        DispositionRefusal::NoSubjects
    );
    let mut no_fragment = whole.clone();
    no_fragment.fragment = Default::default();
    assert_eq!(
        composition_refusal(compose(&dynamic, &no_fragment)),
        DispositionRefusal::NoFragment
    );
    // A component of a package whose declarations were not supplied has no
    // owner to be checked against.
    let mut elsewhere = dynamic.named("role", "S");
    elsewhere.package = "another-checked-package".into();
    let mut other_package = whole.clone();
    other_package.subjects = vec![elsewhere.clone()];
    assert_eq!(
        composition_refusal(compose(&dynamic, &other_package)),
        DispositionRefusal::UndeclaredSubject(elsewhere)
    );
    let stranger = foreign(&dynamic);
    let mut substituted = whole;
    substituted.subjects = vec![stranger.clone()];
    assert_eq!(
        composition_refusal(compose(&dynamic, &substituted)),
        DispositionRefusal::UndeclaredSubject(stranger)
    );
}

#[test]
#[trace("TC-200", "FR-200-AC-6")]
fn a_composition_is_never_proved_while_a_relied_on_guarantee_is_not() {
    let dynamic = Dynamic::with("workflow", 3);
    for label in [
        ItemLabel::Unsupported,
        ItemLabel::Inconclusive,
        ItemLabel::Failed,
        ItemLabel::Refuted,
    ] {
        let mut asked = request(&dynamic);
        asked.guarantees[0].label = label;
        let composition = decided(compose(&dynamic, &asked));
        assert_eq!(composition.verdict(), Verdict::Unproved, "{label:?}");
        assert_ne!(composition.truth(), Truth::Satisfied);
        assert_eq!(
            cause(&composition, PremiseList::Relies, 0),
            Some(Unavailable::NotProved(label))
        );
        // The liveness dependency used that guarantee, so it is lost too.
        assert_eq!(
            cause(&composition, PremiseList::LivenessDependencies, 0),
            Some(Unavailable::DependsOn(node(&dynamic, "Reject")))
        );
    }
}

#[test]
#[trace("TC-200", "FR-200-AC-7")]
fn the_four_verdicts_read_into_the_axes() {
    let dynamic = Dynamic::with("workflow", 3);
    let reading = |asked: &Req| {
        let c = decided(compose(&dynamic, asked));
        (c.verdict(), c.execution(), c.truth(), c.basis())
    };
    let base = request(&dynamic);
    assert_eq!(
        reading(&base),
        (
            Verdict::Proved,
            AssessmentExecution::Completed,
            Truth::Satisfied,
            SettlementBasis::DecisiveWitness
        )
    );
    let mut stronger = base.clone();
    stronger.assumptions = vec![node(&dynamic, "Tail")];
    assert_eq!(
        reading(&stronger),
        (
            Verdict::Incompatible,
            AssessmentExecution::Completed,
            Truth::Violated,
            SettlementBasis::DecisiveWitness
        )
    );
    let mut unproved = base.clone();
    unproved.guarantees[0].label = ItemLabel::Inconclusive;
    assert_eq!(
        reading(&unproved),
        (
            Verdict::Unproved,
            AssessmentExecution::Completed,
            Truth::Pending,
            SettlementBasis::Unsettled
        )
    );
    let mut circular = base;
    circular.guarantees[0].uses.push(node(&dynamic, "New"));
    assert_eq!(
        reading(&circular),
        (
            Verdict::Circular,
            AssessmentExecution::Refused,
            Truth::Unavailable,
            SettlementBasis::Unavailable
        )
    );
}

// ---------------------------------------------------------------- TC-201

#[test]
#[trace("TC-201", "FR-201-AC-1")]
fn each_list_discharges_by_its_own_rule() {
    let dynamic = Dynamic::with("workflow", 3);
    let mut asked = request(&dynamic);
    // `Head` is offered, so it discharges an environment premise; it is no
    // declared guarantee, so it fails a rely premise.
    asked.premises.relies = vec![node(&dynamic, "Head")];
    let composition = decided(compose(&dynamic, &asked));
    assert_eq!(
        composition.discharges[0].by,
        DischargedBy::Assumption(node(&dynamic, "Head"))
    );
    assert_eq!(
        cause(&composition, PremiseList::Relies, 0),
        Some(Unavailable::NotDeclared)
    );
    // `Block` is offered by no one, so it fails a fairness premise.
    asked.premises.fairness = vec![node(&dynamic, "Block")];
    let composition = decided(compose(&dynamic, &asked));
    assert_eq!(
        cause(&composition, PremiseList::Fairness, 0),
        Some(Unavailable::NotOffered)
    );
}

#[test]
#[trace("TC-201", "FR-201-AC-2")]
fn the_four_lists_stay_separate_and_each_discharge_is_recorded_against_its_own() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic);
    let composition = decided(compose(&dynamic, &asked));
    assert_eq!(composition.binding.request.premises, asked.premises);
    let lists: Vec<_> = composition.discharges.iter().map(|d| d.premise).collect();
    assert_eq!(
        lists,
        [
            PremiseList::Environment,
            PremiseList::Relies,
            PremiseList::LivenessDependencies,
            PremiseList::Fairness
        ]
        .map(|list| PremiseRef { list, index: 0 })
    );
}

#[test]
#[trace("TC-201", "FR-201-AC-3")]
fn the_discharge_record_names_the_discharger_of_each_premise() {
    let dynamic = Dynamic::with("workflow", 3);
    let composition = decided(compose(&dynamic, &request(&dynamic)));
    let by: Vec<_> = composition
        .discharges
        .iter()
        .map(|d| d.by.clone())
        .collect();
    assert_eq!(
        by,
        vec![
            DischargedBy::Assumption(node(&dynamic, "Head")),
            DischargedBy::Guarantee(node(&dynamic, "Reject")),
            DischargedBy::Guarantee(node(&dynamic, "New")),
            DischargedBy::Assumption(node(&dynamic, "Tail")),
        ]
    );
}

#[test]
#[trace("TC-201", "FR-201-AC-4")]
fn a_premise_naming_an_undeclared_node_is_refused_naming_it() {
    let dynamic = Dynamic::with("workflow", 3);
    let stranger = foreign(&dynamic);
    let mut asked = request(&dynamic);
    asked.premises.relies = vec![stranger.clone()];
    assert_eq!(
        composition_refusal(compose(&dynamic, &asked)),
        DispositionRefusal::UndeclaredPremise(stranger)
    );
}

#[test]
#[trace("TC-201", "FR-201-AC-5")]
fn a_guarantee_of_the_other_form_or_unproved_leaves_the_premise_undischarged() {
    let dynamic = Dynamic::with("workflow", 3);
    let mut asked = request(&dynamic);
    // The liveness guarantee `New` named by a rely, the safety guarantee
    // `Reject` named by a liveness dependency.
    asked.premises.relies = vec![node(&dynamic, "New")];
    asked.premises.liveness_dependencies = vec![node(&dynamic, "Reject")];
    let composition = decided(compose(&dynamic, &asked));
    assert_eq!(
        cause(&composition, PremiseList::Relies, 0),
        Some(Unavailable::WrongForm(ClauseForm::Liveness))
    );
    assert_eq!(
        cause(&composition, PremiseList::LivenessDependencies, 0),
        Some(Unavailable::WrongForm(ClauseForm::Safety))
    );
    assert_eq!(composition.verdict(), Verdict::Unproved);
    asked.premises = request(&dynamic).premises;
    asked.guarantees[1].label = ItemLabel::Inconclusive;
    let composition = decided(compose(&dynamic, &asked));
    assert_eq!(
        cause(&composition, PremiseList::LivenessDependencies, 0),
        Some(Unavailable::NotProved(ItemLabel::Inconclusive))
    );
    assert_ne!(composition.verdict(), Verdict::Proved);
}

#[test]
#[trace("TC-201", "FR-201-AC-6")]
fn no_discharger_leaves_an_empty_record_and_every_premise_undischarged() {
    let dynamic = Dynamic::with("workflow", 3);
    let mut asked = request(&dynamic);
    asked.assumptions = vec![node(&dynamic, "Block")];
    asked.guarantees.clear();
    let composition = decided(compose(&dynamic, &asked));
    assert!(composition.discharges.is_empty());
    assert_eq!(composition.undischarged.len(), 4);
    assert_eq!(composition.verdict(), Verdict::Unproved);
}

// ---------------------------------------------------------------- TC-202

// ---------------------------------------------------------------- TC-203

/// Replace the proof edges of the guarantees named by `edges` (`from`, `uses`).
fn with_cycle(dynamic: &Dynamic, mut asked: Req, edges: &[(&str, &[&str])]) -> Req {
    asked.guarantees = edges
        .iter()
        .map(|(name, uses)| guarantee(dynamic, name, ClauseForm::Safety, uses))
        .collect();
    asked
}

fn cyclic(dynamic: &Dynamic) -> Req {
    // `Reject` and `Tail` use one another; `Head` is offered.
    let mut asked = with_cycle(
        dynamic,
        request(dynamic),
        &[("Reject", &["Tail", "Head"]), ("Tail", &["Reject"])],
    );
    asked.premises.liveness_dependencies.clear();
    asked.premises.fairness.clear();
    asked.assumptions = vec![node(dynamic, "Head")];
    asked
}

#[test]
#[trace("TC-203", "FR-203-AC-1")]
fn a_cycle_is_refused_as_circular_and_a_self_use_is_a_cycle_of_one() {
    let dynamic = Dynamic::with("workflow", 3);
    let composition = decided(compose(&dynamic, &cyclic(&dynamic)));
    assert_eq!(
        composition.cycles,
        vec![{
            let mut pair = vec![node(&dynamic, "Reject"), node(&dynamic, "Tail")];
            pair.sort();
            pair
        }]
    );
    assert_eq!(composition.verdict(), Verdict::Circular);
    assert_ne!(composition.truth(), Truth::Satisfied);
    let own = with_cycle(&dynamic, request(&dynamic), &[("Reject", &["Reject"])]);
    let composition = decided(compose(&dynamic, &own));
    assert_eq!(composition.cycles, vec![vec![node(&dynamic, "Reject")]]);
    assert_eq!(composition.verdict(), Verdict::Circular);
}

#[test]
#[trace("TC-203", "FR-203-AC-2")]
fn a_proved_guarantee_on_a_cycle_discharges_nothing() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = cyclic(&dynamic);
    assert!(asked
        .guarantees
        .iter()
        .all(|g| g.label == ItemLabel::Proved));
    let composition = decided(compose(&dynamic, &asked));
    assert_eq!(
        cause(&composition, PremiseList::Relies, 0),
        Some(Unavailable::InCycle)
    );
    assert!(composition
        .discharges
        .iter()
        .all(|d| !matches!(&d.by, DischargedBy::Guarantee(_))));
}

#[test]
#[trace("TC-203", "FR-203-AC-3")]
fn a_cycle_guarantee_is_never_an_assumption() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = cyclic(&dynamic);
    let member = node(&dynamic, "Reject");
    assert!(cycle_members(&asked.guarantees).contains(&member));
    let mut offered = asked;
    offered.assumptions.push(member.clone());
    assert_eq!(
        composition_refusal(compose(&dynamic, &offered)),
        DispositionRefusal::CycleGuaranteeAsAssumption(member)
    );
}

#[test]
#[trace("TC-203", "FR-203-AC-4")]
fn a_guarantee_that_used_a_cycle_member_is_unavailable_too() {
    let dynamic = Dynamic::with("workflow", 3);
    let mut asked = with_cycle(
        &dynamic,
        request(&dynamic),
        &[
            ("Reject", &["Tail"]),
            ("Tail", &["Reject"]),
            ("New", &["Reject"]),
        ],
    );
    asked.premises.relies = vec![node(&dynamic, "New")];
    asked.premises.liveness_dependencies.clear();
    asked.premises.fairness.clear();
    asked.assumptions = vec![node(&dynamic, "Head")];
    let composition = decided(compose(&dynamic, &asked));
    assert_eq!(
        cause(&composition, PremiseList::Relies, 0),
        Some(Unavailable::DependsOn(node(&dynamic, "Reject")))
    );
    assert_ne!(composition.verdict(), Verdict::Proved);
}

#[test]
#[trace("TC-203", "FR-203-AC-5")]
fn a_cyclic_claim_leaves_an_acyclic_claim_unchanged() {
    let dynamic = Dynamic::with("workflow", 3);
    let acyclic = request(&dynamic);
    let alone = compose(&dynamic, &acyclic);
    let beside = settle(
        &dynamic,
        vec![
            Claims::Composition(cyclic(&dynamic)),
            Claims::Composition(acyclic),
        ],
    );
    assert_eq!(composed(beside[1].clone()), alone);
}

#[test]
#[trace("TC-203", "FR-203-AC-6")]
fn an_acyclic_composition_lists_no_cycle_whatever_the_declaration_order() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic);
    let mut reversed = asked.clone();
    reversed.guarantees.reverse();
    let forward = decided(compose(&dynamic, &asked));
    let backward = decided(compose(&dynamic, &reversed));
    assert!(forward.cycles.is_empty());
    assert_eq!(forward.verdict(), backward.verdict());
    assert_eq!(forward.discharges, backward.discharges);
}

#[test]
#[trace("TC-203", "FR-203-AC-7")]
fn an_unrelied_cycle_makes_the_verdict_circular_beside_a_counterexample() {
    let dynamic = Dynamic::with("workflow", 3);
    let mut asked = request(&dynamic);
    asked.assumptions = vec![node(&dynamic, "Head")];
    asked.premises.liveness_dependencies.clear();
    asked.premises.fairness.clear();
    asked.guarantees = vec![
        guarantee(&dynamic, "Reject", ClauseForm::Safety, &["Head", "Block"]),
        guarantee(&dynamic, "New", ClauseForm::Safety, &["Tail"]),
        guarantee(&dynamic, "Tail", ClauseForm::Safety, &["New"]),
    ];
    let composition = decided(compose(&dynamic, &asked));
    assert_eq!(composition.cycles.len(), 1);
    assert_eq!(composition.counterexamples.len(), 1);
    assert_eq!(composition.verdict(), Verdict::Circular);
}

#[test]
#[trace("TC-200", "FR-200-AC-9")]
fn only_relied_guarantees_block_success_but_any_cycle_is_circular() {
    let dynamic = Dynamic::with("workflow", 3);
    let mut asked = request(&dynamic);
    let mut idle = guarantee(&dynamic, "Block", ClauseForm::Safety, &[]);
    idle.label = ItemLabel::Inconclusive;
    asked.guarantees.push(idle);
    assert_eq!(
        decided(compose(&dynamic, &asked)).verdict(),
        Verdict::Proved
    );
    asked.guarantees.push(guarantee(
        &dynamic,
        "BagNew",
        ClauseForm::Safety,
        &["BagNew"],
    ));
    assert_eq!(
        decided(compose(&dynamic, &asked)).verdict(),
        Verdict::Circular
    );
}

// ---------------------------------------------------------------- TC-204

/// A composition whose liveness dependency `New` used `Tail`, which the
/// request offers only when `offer_tail`; `New` also used `Reject`, which used
/// `Head`.
fn needing_tail(dynamic: &Dynamic, offer_tail: bool) -> Req {
    let mut asked = request(dynamic);
    asked.assumptions = if offer_tail {
        vec![node(dynamic, "Head"), node(dynamic, "Tail")]
    } else {
        vec![node(dynamic, "Head")]
    };
    // The fairness premise is not under test.
    asked.premises.fairness.clear();
    asked
}

#[test]
#[trace("TC-204", "FR-204-AC-8")]
fn a_composition_resting_on_an_unoffered_assumption_yields_a_counterexample() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = needing_tail(&dynamic, false);
    let composition = decided(compose(&dynamic, &asked));
    assert_eq!(composition.verdict(), Verdict::Incompatible);
    assert_ne!(composition.truth(), Truth::Satisfied);
    let [counter] = composition.counterexamples.as_slice() else {
        panic!("one counterexample");
    };
    assert_eq!(
        counter.premise,
        PremiseRef {
            list: PremiseList::LivenessDependencies,
            index: 0
        }
    );
    assert_eq!(counter.offered, asked.assumptions);
    assert_eq!(counter.unoffered(), Some(&node(&dynamic, "Tail")));
    assert_eq!(counter.path.len(), 1);
    // Through a sibling guarantee the path is longer and ends at the same node.
    let mut deep = asked;
    deep.guarantees[0].uses.push(node(&dynamic, "Block"));
    deep.premises.liveness_dependencies.clear();
    let composition = decided(compose(&dynamic, &deep));
    let [counter] = composition.counterexamples.as_slice() else {
        panic!("one counterexample");
    };
    assert_eq!(counter.unoffered(), Some(&node(&dynamic, "Block")));
    assert_eq!(counter.path.len(), 1);
    let mut through = needing_tail(&dynamic, false);
    through.premises.relies.clear();
    through.guarantees[1].uses = vec![node(&dynamic, "Reject")];
    through.guarantees[0].uses = vec![node(&dynamic, "Block")];
    let composition = decided(compose(&dynamic, &through));
    let [counter] = composition.counterexamples.as_slice() else {
        panic!("one counterexample");
    };
    assert_eq!(counter.path.len(), 2);
    assert_eq!(counter.unoffered(), Some(&node(&dynamic, "Block")));
}

#[test]
#[trace("TC-204", "FR-204-AC-9")]
fn a_counterexample_replays_and_an_altered_one_does_not() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = needing_tail(&dynamic, false);
    let composition = decided(compose(&dynamic, &asked));
    let counter = composition.counterexamples[0].clone();
    assert!(counter.replay(&asked));
    let mut other_step = counter.clone();
    other_step.path[0].uses = node(&dynamic, "Block");
    assert!(!other_step.replay(&asked));
    // Repointed to another valid premise, which another guarantee discharges.
    let mut other_premise = counter.clone();
    other_premise.premise = PremiseRef {
        list: PremiseList::Relies,
        index: 0,
    };
    assert_eq!(asked.premises.relies.len(), 1);
    assert!(!other_premise.replay(&asked));
    let mut other_offer = counter.clone();
    other_offer.offered.push(node(&dynamic, "Block"));
    assert!(!other_offer.replay(&asked));
    // Once the assumption is offered the path no longer ends unoffered.
    assert!(!counter.replay(&needing_tail(&dynamic, true)));
    let mut empty = counter;
    empty.path.clear();
    assert!(!empty.replay(&asked));
}

#[test]
#[trace("TC-204", "FR-204-AC-10")]
fn proofs_using_only_offered_assumptions_are_not_incompatible() {
    let dynamic = Dynamic::with("workflow", 3);
    let mut asked = needing_tail(&dynamic, true);
    // An offered assumption no proof used changes nothing.
    asked.assumptions.push(node(&dynamic, "Block"));
    let composition = decided(compose(&dynamic, &asked));
    assert!(composition.counterexamples.is_empty());
    assert_eq!(composition.verdict(), Verdict::Proved);
}

#[test]
#[trace("TC-204", "FR-204-AC-11")]
fn the_counterexample_does_not_depend_on_order_or_on_the_relied_guarantees_label() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = needing_tail(&dynamic, false);
    let found = decided(compose(&dynamic, &asked)).counterexamples;
    assert_eq!(found.len(), 1);
    // Reversing the declared guarantees and the nodes a proof used changes nothing.
    let mut reversed = asked.clone();
    reversed.guarantees.reverse();
    for guarantee in &mut reversed.guarantees {
        guarantee.uses.reverse();
    }
    assert_eq!(decided(compose(&dynamic, &reversed)).counterexamples, found);
    // Two unoffered nodes: the same one is named in either order.
    let mut two = asked.clone();
    two.guarantees[1].uses = vec![node(&dynamic, "Tail"), node(&dynamic, "Block")];
    let mut two_reversed = two.clone();
    two_reversed.guarantees[1].uses.reverse();
    assert_eq!(
        decided(compose(&dynamic, &two)).counterexamples,
        decided(compose(&dynamic, &two_reversed)).counterexamples
    );
    // A relied guarantee that is not `proved` still yields it.
    let mut unproved = asked.clone();
    unproved.guarantees[1].label = ItemLabel::Inconclusive;
    let composition = decided(compose(&dynamic, &unproved));
    assert_eq!(composition.counterexamples, found);
    assert_eq!(composition.verdict(), Verdict::Incompatible);
    // So does one that reaches the unoffered node through a cycle member.
    let mut through = needing_tail(&dynamic, false);
    through.assumptions = vec![node(&dynamic, "Head")];
    through.guarantees = vec![
        guarantee(&dynamic, "Reject", ClauseForm::Safety, &["Tail"]),
        guarantee(&dynamic, "Tail", ClauseForm::Safety, &["Reject", "Block"]),
    ];
    through.premises.liveness_dependencies.clear();
    let composition = decided(compose(&dynamic, &through));
    let [counter] = composition.counterexamples.as_slice() else {
        panic!("one counterexample");
    };
    assert_eq!(counter.unoffered(), Some(&node(&dynamic, "Block")));
    assert_eq!(counter.path.len(), 2);
    assert!(counter.replay(&through));
}

#[test]
#[trace("TC-204", "FR-204-AC-12")]
fn a_forged_counterexample_fails_replay() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = needing_tail(&dynamic, false);
    let counter = decided(compose(&dynamic, &asked)).counterexamples[0].clone();
    assert!(counter.replay(&asked));
    // An environment or fairness premise is never relied on.
    for list in [PremiseList::Environment, PremiseList::Fairness] {
        let mut forged = counter.clone();
        forged.premise = PremiseRef { list, index: 0 };
        assert!(!forged.replay(&asked), "{list:?}");
    }
    // The first guarantee is liveness, but the list needs a safety one.
    let mut other_form = asked.clone();
    other_form.premises.relies = vec![node(&dynamic, "New")];
    let mut forged = counter.clone();
    forged.premise = PremiseRef {
        list: PremiseList::Relies,
        index: 0,
    };
    assert!(!forged.replay(&other_form));
    // A path that returns to a guarantee on it.
    let mut looping = asked;
    looping.guarantees = vec![
        guarantee(&dynamic, "Reject", ClauseForm::Safety, &["New"]),
        guarantee(&dynamic, "New", ClauseForm::Safety, &["Reject", "Block"]),
    ];
    looping.premises.liveness_dependencies.clear();
    let step = |from: &str, to: &str| filament_protocol::composition::DependencyStep {
        guarantee: node(&dynamic, from),
        uses: node(&dynamic, to),
    };
    let mut forged = counter;
    forged.premise = PremiseRef {
        list: PremiseList::Relies,
        index: 0,
    };
    forged.path = vec![
        step("Reject", "New"),
        step("New", "Reject"),
        step("Reject", "New"),
    ];
    assert!(!forged.replay(&looping));
    forged.path = vec![step("Reject", "New"), step("New", "Block")];
    assert!(forged.replay(&looping));
}
