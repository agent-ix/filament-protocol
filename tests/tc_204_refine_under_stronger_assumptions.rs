// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-204: `refinement` claims over real protocols whose systems rest on a
//! required temporal clause (FR-204-AC-1 to AC-7). The clause `Due` is written
//! in FSL source, required by the protocol with `requires temporal`, compiled
//! through FSL's checked-handoff writer and read back; the assumption of each
//! system is the clause FSL's step view reports as required. No system or
//! assumption is stated by the caller.

mod support {
    pub mod assume_guarantee;
    pub mod dynamic;
    pub mod handoff;
    pub mod projection;
    pub mod realizability;
}

use filament_contract_model::CheckedNodeId;
use filament_protocol::dispatch::{ClaimDisposition, Fragment};
use filament_protocol::refinement::{
    decide, step_system, AssessedSubject, Party, Refinement, RefinementOutcome, RefinementRequest,
    Side, StrongerAssumption, Verdict,
};
use fsl_replay::Truth;
use ix_trace_rs::trace;
use support::assume_guarantee::{Boundary, Claims, Settled};
use support::projection::{assessed_root, CANCELLED, PICK};
use support::realizability::Realizability;

const CHANNELS: [&str; 3] = ["Order", "Pick", "Cancelled"];

/// `Order`, then `Pick` or `Cancelled`, resting on the required clause `Due`.
fn requiring() -> Realizability {
    Realizability::requiring("Flow", "Due")
}

/// The same protocol resting on no assumption.
fn plain() -> Realizability {
    Realizability::worked(PICK, CANCELLED)
}

/// `Order`, then `Cancelled` or nothing: fewer traces than `plain`.
fn narrow() -> Realizability {
    Realizability::worked("", CANCELLED)
}

fn side(of: &Realizability, fragment: &Fragment) -> Side {
    Side {
        node: of.clause().clone(),
        system: step_system(of.package(), of.clause(), &of.subject(), fragment, 4096),
    }
}

fn asked(
    implementation: &Realizability,
    specification: &Realizability,
    assumptions: Vec<CheckedNodeId>,
) -> RefinementRequest {
    let fragment = Fragment {
        channels: CHANNELS
            .iter()
            .map(|name| implementation.named("channel", name))
            .collect(),
        roles: Vec::new(),
    };
    let draft = assessed_root();
    RefinementRequest {
        implementation: side(implementation, &fragment),
        specification: side(specification, &fragment),
        fragment,
        assumptions,
        sources: vec![implementation.clause().clone()],
        max_states: 100_000,
        assessed_root: AssessedSubject {
            observation: draft.observation,
            object: draft.object,
        },
    }
}

fn decided_over(components: &[&Realizability], request: &RefinementRequest) -> Refinement {
    let declarations: Vec<_> = components
        .iter()
        .map(|component| component.linked().declarations())
        .collect();
    match decide(&declarations, request) {
        RefinementOutcome::Decided(found) => found,
        other => panic!("not decided: {other:?}"),
    }
}

/// The one clause `of` requires.
fn due(of: &Realizability) -> CheckedNodeId {
    match of.linked().requirements() {
        [only] => only.clone(),
        other => panic!("one required clause: {other:?}"),
    }
}

#[test]
fn the_step_view_reports_the_required_clause_as_the_systems_assumption() {
    let (rests, free) = (requiring(), plain());
    let request = asked(&rests, &free, Vec::new());
    assert_eq!(request.implementation.system.assumptions(), [due(&rests)]);
    assert!(request.specification.system.assumptions().is_empty());
    assert!(rests.linked().declarations().declares(&due(&rests)));
}

#[test]
#[trace("TC-204", "FR-204-AC-1")]
fn systems_resting_on_exactly_or_a_subset_of_the_offered_assumptions_refine() {
    let (rests, free) = (requiring(), plain());
    let offered = vec![due(&rests)];
    // The implementation rests on exactly the offered assumption.
    let request = asked(&rests, &free, offered.clone());
    let found = decided_over(&[&rests, &free], &request);
    assert!(found.stronger_assumptions.is_empty());
    assert_eq!(found.verdict(), Verdict::Refines, "{found:?}");
    // A subset: the offered set holds a second assumption the system lacks.
    let wider = vec![due(&rests), rests.named("channel", "Order")];
    let request = asked(&rests, &free, wider);
    let found = decided_over(&[&rests, &free], &request);
    assert!(found.stronger_assumptions.is_empty());
    assert_eq!(found.verdict(), Verdict::Refines);
    // Both systems rest on it.
    let twin = requiring();
    let request = asked(&rests, &twin, offered);
    let found = decided_over(&[&rests, &twin], &request);
    assert!(found.stronger_assumptions.is_empty());
    assert_eq!(found.verdict(), Verdict::Refines);
}

#[test]
#[trace("TC-204", "FR-204-AC-2")]
fn an_implementation_resting_on_an_unoffered_assumption_is_refuted_and_each_finding_replays() {
    let (rests, free) = (requiring(), plain());
    let request = asked(&rests, &free, Vec::new());
    let found = decided_over(&[&rests, &free], &request);
    assert!(found.counterexamples.is_empty(), "the traces are the same");
    assert_eq!(
        found.stronger_assumptions,
        vec![StrongerAssumption {
            party: Party::Implementation,
            assumption: due(&rests),
            offered: Vec::new(),
        }]
    );
    assert!(found.stronger_assumptions[0].replay(&request));
    assert_eq!(found.verdict(), Verdict::Refuted);
    assert_eq!(found.truth(), Truth::Violated);
}

#[test]
#[trace("TC-204", "FR-204-AC-3")]
fn offered_assumptions_no_system_rests_on_change_nothing() {
    let (first, second) = (plain(), plain());
    let rests = requiring();
    // Neither system rests on anything; the request offers an assumption the
    // package of another protocol declares.
    let request = asked(&first, &second, vec![due(&rests)]);
    let found = decided_over(&[&first, &second, &rests], &request);
    assert!(found.stronger_assumptions.is_empty());
    assert_eq!(found.verdict(), Verdict::Refines);
    let bare = asked(&first, &second, Vec::new());
    assert_eq!(
        decided_over(&[&first, &second], &bare).verdict(),
        Verdict::Refines
    );
}

#[test]
#[trace("TC-204", "FR-204-AC-4")]
fn an_extra_trace_and_an_unoffered_assumption_are_both_kept() {
    let (rests, narrow) = (requiring(), narrow());
    let request = asked(&rests, &narrow, Vec::new());
    let found = decided_over(&[&rests, &narrow], &request);
    assert!(!found.counterexamples.is_empty(), "the Pick trace is extra");
    assert_eq!(found.stronger_assumptions.len(), 1);
    assert_eq!(found.stronger_assumptions[0].assumption, due(&rests));
    assert_eq!(found.verdict(), Verdict::Refuted);
    // Offering the assumption leaves the extra trace alone.
    let offered = asked(&rests, &narrow, vec![due(&rests)]);
    let found = decided_over(&[&rests, &narrow], &offered);
    assert!(!found.counterexamples.is_empty());
    assert!(found.stronger_assumptions.is_empty());
}

#[test]
#[trace("TC-204", "FR-204-AC-5")]
fn a_stronger_assumption_beside_other_claims_changes_none_of_their_dispositions() {
    let (rests, free) = (requiring(), plain());
    let other = asked(&rests, &free, vec![due(&rests)]);
    let stronger = asked(&rests, &free, Vec::new());
    let settle = |requests: Vec<Claims>| -> Vec<Settled> {
        filament_protocol::dispatch::dispatch_claims(
            &mut Boundary::new(),
            rests.linked().declarations(),
            &[rests.linked().declarations(), free.linked().declarations()],
            &requests,
        )
    };
    let without = settle(vec![
        Claims::GlobalConformance("conformant"),
        Claims::Refinement(other.clone()),
    ]);
    let with = settle(vec![
        Claims::GlobalConformance("conformant"),
        Claims::Refinement(other),
        Claims::Refinement(stronger),
    ]);
    assert_eq!(with[..2], without[..]);
    assert_eq!(with[0], ClaimDisposition::GlobalConformance("conformant"));
    let ClaimDisposition::Refinement(RefinementOutcome::Decided(refuted)) = &with[2] else {
        panic!("a decided refinement: {:?}", with[2]);
    };
    assert_eq!(refuted.verdict(), Verdict::Refuted);
}

#[test]
#[trace("TC-204", "FR-204-AC-6")]
fn a_specification_resting_on_an_unoffered_assumption_is_named_by_the_finding() {
    let (free, rests) = (plain(), requiring());
    let request = asked(&free, &rests, Vec::new());
    let found = decided_over(&[&free, &rests], &request);
    let [finding] = found.stronger_assumptions.as_slice() else {
        panic!("one finding: {:?}", found.stronger_assumptions);
    };
    assert_eq!(finding.party, Party::Specification);
    assert_eq!(finding.assumption, due(&rests));
    assert!(finding.replay(&request));
    assert_eq!(found.verdict(), Verdict::Refuted);
}

#[test]
#[trace("TC-204", "FR-204-AC-7")]
fn an_altered_finding_or_an_offered_assumption_fails_the_replay() {
    let (rests, free) = (requiring(), plain());
    let request = asked(&rests, &free, Vec::new());
    let found = decided_over(&[&rests, &free], &request);
    let finding = found.stronger_assumptions[0].clone();
    assert!(finding.replay(&request));
    let other_system = StrongerAssumption {
        party: Party::Specification,
        ..finding.clone()
    };
    assert!(
        !other_system.replay(&request),
        "the specification rests on nothing"
    );
    let other_assumption = StrongerAssumption {
        assumption: rests.named("channel", "Order"),
        ..finding.clone()
    };
    assert!(!other_assumption.replay(&request));
    let other_side = StrongerAssumption {
        offered: vec![rests.named("channel", "Order")],
        ..finding.clone()
    };
    assert!(
        !other_side.replay(&request),
        "the offered side is the request's"
    );
    let mut offering = request.clone();
    offering.assumptions = vec![due(&rests)];
    assert!(!finding.replay(&offering), "the request now offers it");
}
