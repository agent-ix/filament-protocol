// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-173: the V1-PROT-028, V1-PROT-029 and V1-PROT-030 corpora of IT-005, each
//! run over real protocols compiled through FSL's checked handoff. No system,
//! subject or guarantee label is built by hand: a transition system comes from
//! FSL's protocol step view, a subject from FSL's `protocol_subject`, and a
//! guarantee's label from the result FSL's engine settled.

mod support {
    pub mod assume_guarantee;
    pub mod dynamic;
    pub mod handoff;
    pub mod projection;
    pub mod realizability;
}

use filament_protocol::assume_guarantee::{AssumeGuaranteeRequest, PremiseLists};
use filament_protocol::closure::AssessmentExecution;
use filament_protocol::composition::{
    Composition, CompositionOutcome, Guarantee, Unavailable, Verdict as ComposedVerdict,
};
use filament_protocol::declarations::Declarations;
use filament_protocol::dispatch::{dispatch_claims, BackendResult, ClaimDisposition, Fragment};
use filament_protocol::engine::EngineResult;
use filament_protocol::refinement::{
    decide, step_system, AssessedSubject, Fault, Party, Refinement, RefinementOutcome,
    RefinementRequest, Side, Undecidable, Verdict,
};
use fsl_analyze::protocol_analysis::ProtocolAnalysisArtifact;
use fsl_replay::planner::ClauseForm;
use fsl_replay::protocol_analysis::{AnalysisOutcome, RealizabilityCounterexample};
use fsl_replay::{ItemLabel, SettlementBasis, Truth, UnsteppedConstruct};
use ix_trace_rs::trace;
use support::assume_guarantee::{Boundary, Claims, Settled};
use support::projection::{assessed_root, CANCELLED, PICK};
use support::realizability::{channel, settle as realize, Realizability};

const CHANNELS: [&str; 3] = ["Order", "Pick", "Cancelled"];

// ------------------------------------------------- the reference workflow family

/// `Order`, then `Pick` or `Cancelled`.
fn wide() -> Realizability {
    Realizability::worked(PICK, CANCELLED)
}

/// `Order`, then `Cancelled` or nothing.
fn narrow() -> Realizability {
    Realizability::worked("", CANCELLED)
}

/// `wide` over channels that may lose a message.
fn lossy() -> Realizability {
    Realizability::delivering("at-most-once", "", "", PICK, CANCELLED)
}

/// After `Order`, a loop of `Pick` sends and receives that never ends.
fn spinning() -> Realizability {
    let spin = "repeat Spin by Seller visible () while { true } sequence SpinBody { \
                send PickIt via Pick as (p: PickMsg) { true };\n\
                receive PickedUp via Pick of PickIt as (q: PickMsg) { true }; }";
    Realizability::worked(spin, CANCELLED)
}

/// `wide` with `Warehouse` declared as a replicated role.
fn replicated() -> Realizability {
    let pool = "role Warehouse each M::Depot from depots max 2 lifetime workflow;\n";
    Realizability::delivering("exactly-once-premise", pool, "", PICK, CANCELLED)
}

/// `Pick` and `Cancelled` sent from different roles into one channel, so that
/// the uninformed `Warehouse` cannot tell the branches apart.
fn incompatible_merge() -> Realizability {
    let extra = channel("Ready", "Warehouse", "Seller", "PickMsg");
    let ship = "send R1 via Ready as (r1: PickMsg) { true };\n\
        receive R1r via Ready of R1 as (s1: PickMsg) { true };";
    let cancel = "send R2 via Ready as (r2: PickMsg) { true };\n\
        receive R2r via Ready of R2 as (s2: PickMsg) { true };";
    Realizability::worked_with(&extra, ship, cancel)
}

// ------------------------------------------------------------ refinement corpus

fn fragment(of: &Realizability, names: &[&str]) -> Fragment {
    Fragment {
        channels: names.iter().map(|n| of.named("channel", n)).collect(),
        roles: Vec::new(),
    }
}

fn side(of: &Realizability, fragment: &Fragment) -> Side {
    Side {
        node: of.clause().clone(),
        system: step_system(of.package(), of.clause(), &of.subject(), fragment, 4096),
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
        implementation: side(implementation, &fragment),
        specification: side(specification, &fragment),
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

fn components<'a>(
    implementation: &'a Realizability,
    specification: &'a Realizability,
) -> [&'a Declarations; 2] {
    [
        implementation.linked().declarations(),
        specification.linked().declarations(),
    ]
}

/// What a corpus case is expected to decide.
enum Expect {
    Refines,
    ExtraTrace,
    Deadlock,
    Divergence,
    Unsupported,
    Incomplete,
}

struct RefinementCase {
    name: &'static str,
    implementation: fn() -> Realizability,
    specification: fn() -> Realizability,
    /// The channels of the claim's fragment.
    channels: &'static [&'static str],
    /// A role of the implementation the fragment names.
    role: Option<&'static str>,
    max_states: Option<usize>,
    expect: Expect,
}

const REFINEMENT_CORPUS: [RefinementCase; 6] = [
    RefinementCase {
        name: "narrow refines wide",
        implementation: narrow,
        specification: wide,
        channels: &CHANNELS,
        role: None,
        max_states: None,
        expect: Expect::Refines,
    },
    RefinementCase {
        name: "wide has an extra trace over narrow",
        implementation: wide,
        specification: narrow,
        channels: &CHANNELS,
        role: None,
        max_states: None,
        expect: Expect::ExtraTrace,
    },
    RefinementCase {
        name: "lossy at-most-once deadlocks where wide continues",
        implementation: lossy,
        specification: wide,
        channels: &CHANNELS,
        role: None,
        max_states: None,
        expect: Expect::Deadlock,
    },
    RefinementCase {
        name: "a hidden loop diverges where narrow stops",
        implementation: spinning,
        specification: narrow,
        channels: &["Order"],
        role: None,
        max_states: None,
        expect: Expect::Divergence,
    },
    RefinementCase {
        name: "a replicated role in the fragment is unsupported",
        implementation: replicated,
        specification: wide,
        channels: &CHANNELS,
        role: Some("Warehouse"),
        max_states: None,
        expect: Expect::Unsupported,
    },
    RefinementCase {
        name: "a bound below the explored states is incomplete",
        implementation: wide,
        specification: wide,
        channels: &CHANNELS,
        role: None,
        max_states: Some(1),
        expect: Expect::Incomplete,
    },
];

/// A case built into its two real protocols and its request.
fn built(case: &RefinementCase) -> (Realizability, Realizability, RefinementRequest) {
    let (implementation, specification) = ((case.implementation)(), (case.specification)());
    let mut request = asked_over(&implementation, &specification, case.channels);
    if let Some(role) = case.role {
        request.fragment.roles = vec![implementation.named("role", role)];
    }
    if let Some(bound) = case.max_states {
        request.max_states = bound;
    }
    (implementation, specification, request)
}

fn decided(
    implementation: &Realizability,
    specification: &Realizability,
    request: &RefinementRequest,
) -> Refinement {
    match decide(&components(implementation, specification), request) {
        RefinementOutcome::Decided(found) => found,
        other => panic!("not decided: {other:?}"),
    }
}

/// The outcome the dispatcher settles for `request`, beside a conformance
/// result that must stay as it was.
fn dispatched(
    implementation: &Realizability,
    specification: &Realizability,
    request: &RefinementRequest,
) -> RefinementOutcome {
    let settled: Vec<Settled> = dispatch_claims(
        &mut Boundary::new(),
        implementation.linked().declarations(),
        &components(implementation, specification),
        &[
            Claims::GlobalConformance("conformant"),
            Claims::Refinement(request.clone()),
        ],
    );
    assert_eq!(
        settled[0],
        ClaimDisposition::GlobalConformance("conformant")
    );
    match settled[1].clone() {
        ClaimDisposition::Refinement(outcome) => outcome,
        other => panic!("not a refinement disposition: {other:?}"),
    }
}

fn has_fault(found: &Refinement, wanted: impl Fn(&Fault) -> bool) -> bool {
    found.counterexamples.iter().any(|c| wanted(&c.fault))
}

#[test]
#[trace("TC-173", "FR-301-AC-3")]
fn each_refinement_case_of_the_reference_workflow_family_is_decided_alike_by_decide_and_dispatch() {
    for case in &REFINEMENT_CORPUS {
        let (implementation, specification, request) = built(case);
        let found = decided(&implementation, &specification, &request);
        assert_eq!(
            dispatched(&implementation, &specification, &request),
            RefinementOutcome::Decided(found.clone()),
            "{}",
            case.name
        );
        let (verdict, execution, truth, basis) = match case.expect {
            Expect::Refines => (
                Verdict::Refines,
                AssessmentExecution::Completed,
                Truth::Satisfied,
                SettlementBasis::ClosedScope,
            ),
            Expect::ExtraTrace | Expect::Deadlock | Expect::Divergence => (
                Verdict::Refuted,
                AssessmentExecution::Completed,
                Truth::Violated,
                SettlementBasis::DecisiveCounterexample,
            ),
            Expect::Unsupported => (
                Verdict::Unsupported,
                AssessmentExecution::Unsupported,
                Truth::Unavailable,
                SettlementBasis::Unavailable,
            ),
            Expect::Incomplete => (
                Verdict::Incomplete,
                AssessmentExecution::ResourceIncomplete,
                Truth::Unavailable,
                SettlementBasis::Unavailable,
            ),
        };
        assert_eq!(
            (
                found.verdict(),
                found.execution(),
                found.truth(),
                found.basis()
            ),
            (verdict, execution, truth, basis),
            "{}: {found:?}",
            case.name
        );
    }
}

#[test]
#[trace("TC-173", "FR-202-AC-1")]
fn the_extra_trace_case_carries_a_replayable_pair_of_runs() {
    let case = &REFINEMENT_CORPUS[1];
    let (implementation, specification, request) = built(case);
    let found = decided(&implementation, &specification, &request);
    assert!(has_fault(&found, |f| matches!(f, Fault::ExtraTrace { .. })));
    for counter in &found.counterexamples {
        assert!(counter.replay(&request), "{counter:?}");
    }
    // The same counterexample does not replay against the narrow claim's
    // reverse: the specification is the wide protocol there and accepts it.
    let reversed = asked_over(&specification, &implementation, &CHANNELS);
    for counter in &found.counterexamples {
        assert!(!counter.replay(&reversed), "{counter:?}");
    }
}

#[test]
#[trace("TC-173", "FR-202-AC-2")]
fn the_deadlock_case_carries_a_replayable_pair_of_runs() {
    let (implementation, specification, request) = built(&REFINEMENT_CORPUS[2]);
    let found = decided(&implementation, &specification, &request);
    assert!(has_fault(&found, |f| *f == Fault::Deadlock), "{found:?}");
    for counter in found
        .counterexamples
        .iter()
        .filter(|c| c.fault == Fault::Deadlock)
    {
        assert!(counter.replay(&request));
    }
}

#[test]
#[trace("TC-173", "FR-202-AC-3")]
fn the_divergence_case_carries_a_replayable_lasso_of_hidden_steps() {
    let (implementation, specification, request) = built(&REFINEMENT_CORPUS[3]);
    let found = decided(&implementation, &specification, &request);
    let [counter] = found.counterexamples.as_slice() else {
        panic!("one counterexample: {:?}", found.counterexamples);
    };
    let Fault::Divergence { lasso } = &counter.fault else {
        panic!("a divergence: {counter:?}");
    };
    assert!(!lasso.is_empty());
    assert!(counter.replay(&request));
}

#[test]
#[trace("TC-173", "FR-301-AC-2")]
fn the_unsupported_case_names_the_replicated_role_and_invents_no_counterexample() {
    let (implementation, specification, request) = built(&REFINEMENT_CORPUS[4]);
    let warehouse = implementation.named("role", "Warehouse");
    let found = decided(&implementation, &specification, &request);
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
    assert!(found.counterexamples.is_empty());
}

#[test]
#[trace("TC-173", "FR-301-AC-4")]
fn the_incomplete_case_refines_once_its_bound_is_lifted() {
    let case = &REFINEMENT_CORPUS[5];
    let (implementation, specification, mut request) = built(case);
    assert_eq!(
        decided(&implementation, &specification, &request).verdict(),
        Verdict::Incomplete
    );
    request.max_states = 100_000;
    assert_eq!(
        decided(&implementation, &specification, &request).verdict(),
        Verdict::Refines
    );
}

// ----------------------------------------------------------- realizability corpus

struct RealizabilityCase {
    name: &'static str,
    fixture: fn() -> Realizability,
    truth: Truth,
    basis: SettlementBasis,
}

const REALIZABILITY_CORPUS: [RealizabilityCase; 4] = [
    RealizabilityCase {
        name: "each role distinguishes the branches",
        fixture: wide,
        truth: Truth::Satisfied,
        basis: SettlementBasis::ClosedScope,
    },
    RealizabilityCase {
        name: "lossy delivery defeats realizability",
        fixture: lossy,
        truth: Truth::Violated,
        basis: SettlementBasis::DecisiveCounterexample,
    },
    RealizabilityCase {
        name: "a hidden choice is violated",
        fixture: || Realizability::worked(PICK, ""),
        truth: Truth::Violated,
        basis: SettlementBasis::DecisiveCounterexample,
    },
    RealizabilityCase {
        name: "an incompatible merge is violated",
        fixture: incompatible_merge,
        truth: Truth::Violated,
        basis: SettlementBasis::DecisiveCounterexample,
    },
];

fn settled_truth(result: &EngineResult) -> (Truth, SettlementBasis) {
    (result.truth(), result.basis())
}

#[test]
#[trace("TC-173", "FR-150-AC-17")]
fn each_realizability_case_settles_on_the_real_engine_and_carries_the_witness_unchanged() {
    for case in &REALIZABILITY_CORPUS {
        let result = realize(&(case.fixture)());
        assert_eq!(
            settled_truth(&result),
            (case.truth, case.basis),
            "{}",
            case.name
        );
        let Some(ProtocolAnalysisArtifact::Realizability(found)) = result.artifact() else {
            panic!("{}: a realizability result", case.name);
        };
        match (&found.outcome, case.truth) {
            (AnalysisOutcome::Proved(_), Truth::Satisfied) => {
                assert!(result.witness().is_none(), "{}", case.name);
            }
            (
                AnalysisOutcome::Refuted(
                    RealizabilityCounterexample::IndistinguishableHistories {
                        separating_witness,
                        ..
                    }
                    | RealizabilityCounterexample::EnvironmentDefeat {
                        separating_witness, ..
                    },
                ),
                Truth::Violated,
            ) => assert_eq!(result.witness(), Some(separating_witness), "{}", case.name),
            (other, _) => panic!("{}: unexpected outcome {other:?}", case.name),
        }
    }
}

// ------------------------------------------------------------- composition corpus

type Request = AssumeGuaranteeRequest<filament_contract_model::CheckedNodeId>;

/// FSL's label for the result the real engine settled.
fn label_of(fixture: &Realizability) -> ItemLabel {
    ItemLabel::of(&realize(fixture).settlement().value)
}

/// Three real components, each compiled by FSL's writer into its own package
/// and linked, each declaring channels of its own that serve as guarantee and
/// assumption identities. A and B are realizable; C hides a choice.
struct Components {
    a: Realizability,
    b: Realizability,
    c: Realizability,
}

fn extra(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| channel(name, "Buyer", "Seller", "Ack"))
        .collect()
}

fn components_of_three() -> Components {
    Components {
        a: Realizability::worked_with(&extra(&["GuaranteeA", "EnvA"]), PICK, CANCELLED),
        b: Realizability::worked_with(&extra(&["GuaranteeB", "FairB"]), PICK, CANCELLED),
        c: Realizability::worked_with(&extra(&["GuaranteeC", "Stronger"]), PICK, ""),
    }
}

impl Components {
    fn node(&self, name: &str) -> filament_contract_model::CheckedNodeId {
        let owner = match name {
            "GuaranteeA" | "EnvA" => &self.a,
            "GuaranteeB" | "FairB" => &self.b,
            _ => &self.c,
        };
        owner.named("channel", name)
    }

    fn guarantee(&self, name: &str, form: ClauseForm, uses: &[&str]) -> Guarantee {
        let owner = match name {
            "GuaranteeA" => &self.a,
            "GuaranteeB" => &self.b,
            _ => &self.c,
        };
        Guarantee {
            id: self.node(name),
            form,
            // FSL's label for the realizability of the component that
            // owns the guarantee, as the real engine settled it.
            label: label_of(owner),
            uses: uses.iter().map(|used| self.node(used)).collect(),
        }
    }

    /// A relies on nothing but its environment; B relies on A's guarantee and
    /// its own fairness assumption.
    fn request(&self) -> Request {
        Request {
            fragment: Fragment {
                channels: vec![self.a.named("channel", "Order")],
                roles: Vec::new(),
            },
            subjects: vec![self.a.clause().clone(), self.b.clause().clone()],
            assumptions: vec![self.node("EnvA"), self.node("FairB")],
            premises: PremiseLists {
                environment: vec![self.node("EnvA")],
                relies: vec![self.node("GuaranteeA")],
                liveness_dependencies: vec![self.node("GuaranteeB")],
                fairness: vec![self.node("FairB")],
            },
            guarantees: vec![
                self.guarantee("GuaranteeA", ClauseForm::Safety, &["EnvA"]),
                self.guarantee("GuaranteeB", ClauseForm::Liveness, &["GuaranteeA", "FairB"]),
            ],
            sources: vec![self.a.clause().clone()],
        }
    }

    fn compose(&self, asked: &Request) -> Composition<filament_contract_model::CheckedNodeId> {
        let declarations = [
            self.a.linked().declarations(),
            self.b.linked().declarations(),
            self.c.linked().declarations(),
        ];
        let settled: Vec<Settled> = dispatch_claims(
            &mut Boundary::new(),
            self.a.linked().declarations(),
            &declarations,
            &[Claims::Composition(asked.clone())],
        );
        match settled.into_iter().next() {
            Some(ClaimDisposition::Composition(CompositionOutcome::Decided(found))) => found,
            other => panic!("not decided: {other:?}"),
        }
    }
}

#[test]
#[trace("TC-173", "FR-150-AC-16")]
fn the_guarantee_labels_come_from_the_real_engine_results_of_the_components() {
    let parts = components_of_three();
    assert_eq!(label_of(&parts.a), ItemLabel::Proved);
    assert_eq!(label_of(&parts.b), ItemLabel::Proved);
    assert_eq!(label_of(&parts.c), ItemLabel::Refuted);
}

#[test]
#[trace("TC-173", "FR-200-AC-1")]
fn real_components_whose_premises_are_all_discharged_compose_to_proved() {
    let parts = components_of_three();
    let asked = parts.request();
    let found = parts.compose(&asked);
    assert_eq!(found.verdict(), ComposedVerdict::Proved, "{found:?}");
    assert_eq!(found.discharges.len(), 4);
    assert!(found.undischarged.is_empty());
    assert!(found.counterexamples.is_empty());
    assert_eq!(found.binding.request, asked);
}

#[test]
#[trace("TC-173", "FR-200-AC-6")]
fn a_premise_relying_on_a_guarantee_the_engine_refuted_leaves_the_composition_unproved() {
    let parts = components_of_three();
    let mut asked = parts.request();
    asked.premises.relies = vec![parts.node("GuaranteeC")];
    asked.guarantees = vec![
        parts.guarantee("GuaranteeC", ClauseForm::Safety, &["EnvA"]),
        parts.guarantee("GuaranteeB", ClauseForm::Liveness, &["FairB"]),
    ];
    let found = parts.compose(&asked);
    assert_eq!(found.verdict(), ComposedVerdict::Unproved);
    assert_ne!(found.truth(), Truth::Satisfied);
    assert!(found
        .undischarged
        .iter()
        .any(|u| u.why == Unavailable::NotProved(ItemLabel::Refuted)));
}

#[test]
#[trace("TC-173", "FR-203-AC-1")]
fn real_components_that_prove_each_other_in_a_cycle_compose_to_circular() {
    let parts = components_of_three();
    let mut asked = parts.request();
    asked.assumptions = vec![parts.node("EnvA"), parts.node("FairB")];
    asked.guarantees = vec![
        parts.guarantee("GuaranteeA", ClauseForm::Safety, &["GuaranteeB"]),
        parts.guarantee("GuaranteeB", ClauseForm::Liveness, &["GuaranteeA"]),
    ];
    let found = parts.compose(&asked);
    assert_eq!(found.verdict(), ComposedVerdict::Circular, "{found:?}");
    assert_ne!(found.truth(), Truth::Satisfied);
}

#[test]
#[trace("TC-173", "FR-204-AC-8")]
fn a_proof_resting_on_an_assumption_the_request_did_not_offer_is_incompatible_with_a_replayable_counterexample(
) {
    let parts = components_of_three();
    let mut asked = parts.request();
    asked.guarantees[0].uses.push(parts.node("Stronger"));
    let found = parts.compose(&asked);
    assert_eq!(found.verdict(), ComposedVerdict::Incompatible, "{found:?}");
    // The relied-on guarantee and the liveness dependency that uses it each
    // rest on the unoffered assumption.
    assert_eq!(
        found.counterexamples.len(),
        2,
        "{:?}",
        found.counterexamples
    );
    let mut offered = asked.clone();
    offered.assumptions.push(parts.node("Stronger"));
    for counter in &found.counterexamples {
        assert_eq!(counter.unoffered(), Some(&parts.node("Stronger")));
        assert!(counter.replay(&asked));
        // Offering the stronger assumption removes the counterexample.
        assert!(!counter.replay(&offered));
    }
    assert_eq!(parts.compose(&offered).verdict(), ComposedVerdict::Proved);
}

#[test]
#[trace("TC-173", "FR-204-AC-11")]
fn the_verdict_and_counterexample_do_not_depend_on_the_order_of_guarantees_or_components() {
    let parts = components_of_three();
    let mut asked = parts.request();
    asked.guarantees[0].uses.push(parts.node("Stronger"));
    let found = parts.compose(&asked);
    let mut reversed = asked.clone();
    reversed.guarantees.reverse();
    reversed.subjects.reverse();
    for guarantee in &mut reversed.guarantees {
        guarantee.uses.reverse();
    }
    let again = parts.compose(&reversed);
    assert_eq!(again.verdict(), found.verdict());
    assert_eq!(again.counterexamples, found.counterexamples);
    // The proved case is order independent too.
    let proved = parts.request();
    let mut shuffled = proved.clone();
    shuffled.guarantees.reverse();
    assert_eq!(parts.compose(&shuffled).verdict(), ComposedVerdict::Proved);
}
