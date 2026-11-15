// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-200 to TC-204: `composition` claims decided by this library and
//! `refinement` claims dispatched to the backend FSL's registry selects, their
//! results carried unchanged and bound to the request they were established
//! for (FR-200 to FR-204).

mod support {
    pub mod assume_guarantee;
    pub mod dynamic;
    pub mod handoff;
}

use filament_contract_model::CheckedNodeId;
use filament_protocol::assume_guarantee::{
    read_refinement_label, AssumeGuaranteeKind, IdentityMismatch, PremiseList,
};
use filament_protocol::closure::AssessmentExecution;
use filament_protocol::composition::{
    cycle_members, decide, Composition, CompositionOutcome, DischargedBy, PremiseRef, Unavailable,
    Verdict,
};
use filament_protocol::dispatch::{
    dispatch_claims, ClaimDisposition, ClaimRequest, DispositionRefusal, ProtocolOutcome,
};
use filament_protocol::disposition::{ClaimSettlement, Disposition, FslReport, UnsupportedCause};
use filament_semantic_value::catalog::CatalogCode;
use fsl_eval::simulation::StateKey;
use fsl_foundation::BackendId;
use fsl_replay::planner::ClauseForm;
use fsl_replay::{
    InconclusiveCause, ItemLabel, PairedAssumptionPath, ProtocolAssumptionReason,
    ProtocolAssumptionWitness, SettlementBasis, Truth,
};
use fsl_semantics::check::Capability;
use ix_trace_rs::trace;
use support::assume_guarantee::{
    always, foreign, guarantee, honest, request, serve, Bind, Boundary, Calls, Claims, Entry, Out,
    Req, Settled,
};
use support::dynamic::Dynamic;

const COMPOSITION: AssumeGuaranteeKind = AssumeGuaranteeKind::Composition;
const REFINEMENT: AssumeGuaranteeKind = AssumeGuaranteeKind::Refinement;
const BOTH: [Capability; 2] = [Capability::Composition, Capability::Refinement];
const CONFORMANT: &str = "conformant";

type Outcome = ProtocolOutcome<Out, Bind>;
type Decided = CompositionOutcome<Entry>;

/// Settle `requests` through the one dispatch path, in order.
fn settle(boundary: &mut Boundary, dynamic: &Dynamic, requests: Vec<Claims>) -> Vec<Settled> {
    dispatch_claims(
        boundary,
        dynamic.linked().declarations(),
        &[dynamic.linked().declarations()],
        &requests,
    )
}

fn first(settled: Vec<Settled>) -> Settled {
    settled.into_iter().next().expect("one disposition")
}

/// A refinement claim, settled alone.
fn one(boundary: &mut Boundary, dynamic: &Dynamic, asked: &Req) -> Outcome {
    let settled = first(settle(
        boundary,
        dynamic,
        vec![Claims::AssumeGuarantee(asked.clone())],
    ));
    guaranteed(settled)
}

fn guaranteed(disposition: Settled) -> Outcome {
    match disposition {
        ClaimDisposition::AssumeGuarantee(outcome) => outcome,
        other => panic!("not a refinement disposition: {other:?}"),
    }
}

/// A composition claim, decided alone.
fn compose(dynamic: &Dynamic, asked: &Req) -> Decided {
    composed(first(settle(
        &mut Boundary::new(),
        dynamic,
        vec![Claims::AssumeGuarantee(asked.clone())],
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

fn carried(outcome: Outcome) -> Out {
    match outcome {
        ProtocolOutcome::Settled(ClaimSettlement::Result { result, .. }) => result,
        other => panic!("not a carried result: {other:?}"),
    }
}

fn refused(outcome: Outcome) -> DispositionRefusal {
    match outcome {
        ProtocolOutcome::Refused(refusal) => refusal,
        other => panic!("not refused: {other:?}"),
    }
}

fn disposition(outcome: Outcome) -> Disposition<Bind> {
    match outcome {
        ProtocolOutcome::Settled(ClaimSettlement::Disposition(disposition)) => disposition,
        other => panic!("not a non-success disposition: {other:?}"),
    }
}

fn unsupported_with(cause: UnsupportedCause) -> Boundary {
    let mut boundary = Boundary::new();
    serve(
        &mut boundary,
        "b",
        &BOTH,
        Box::new(move |_| FslReport::Unsupported {
            cause: cause.clone(),
            construct: None,
        }),
    );
    boundary
}

fn proved() -> Out {
    Out::new(ItemLabel::Proved)
}

/// A boundary whose one stand-in serves both kinds and answers `result`
/// shaped to each request.
fn serving(result: Out) -> (Boundary, Calls) {
    let mut boundary = Boundary::new();
    let calls = serve(
        &mut boundary,
        "composer",
        &BOTH,
        Box::new(move |bound| FslReport::Result(honest(&bound.request, result.clone()))),
    );
    (boundary, calls)
}

/// FSL FR-148's typed strengthened-assumption cause and witness.
fn strengthened() -> InconclusiveCause {
    InconclusiveCause::ProtocolAssumptionNotEstablished(ProtocolAssumptionWitness {
        reason: ProtocolAssumptionReason::Strengthened,
        identities: Vec::new(),
        path: PairedAssumptionPath {
            concrete_initial: StateKey::default(),
            protocol_initial: Vec::new(),
            steps: Vec::new(),
            loop_entry: None,
        },
        undecided_locus: None,
    })
}

fn malformed() -> CatalogCode {
    CatalogCode::new("invalid_model_binding", "malformed-declaration")
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
fn a_composition_is_decided_here_and_no_backend_is_called() {
    let dynamic = Dynamic::with("workflow", 3);
    let (mut boundary, calls) = serving(proved());
    let asked = request(&dynamic, COMPOSITION);
    let composition = decided(composed(first(settle(
        &mut boundary,
        &dynamic,
        vec![Claims::AssumeGuarantee(asked.clone())],
    ))));
    assert_eq!(composition.verdict(), Verdict::Proved);
    assert_eq!(composition.discharges.len(), 4);
    assert!(composition.undischarged.is_empty());
    assert_eq!(composition.binding.request, asked);
    assert_eq!(calls.len(), 0);
}

#[test]
#[trace("TC-200", "FR-200-AC-2")]
fn a_composition_needs_no_backend_and_a_named_backend_is_refused() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic, COMPOSITION);
    // The boundary serves nothing, and the claim is decided all the same.
    assert_eq!(
        decided(compose(&dynamic, &asked)).verdict(),
        Verdict::Proved
    );
    let mut named = asked;
    named.backend = Some(BackendId::new("elsewhere"));
    assert_eq!(
        composition_refusal(compose(&dynamic, &named)),
        DispositionRefusal::BackendNamedForComposition(BackendId::new("elsewhere"))
    );
}

#[test]
#[trace("TC-200", "FR-200-AC-3")]
fn a_refused_composition_leaves_conformance_and_refinement_unchanged() {
    let dynamic = Dynamic::with("workflow", 3);
    let mut boundary = Boundary::new();
    serve(
        &mut boundary,
        "refiner",
        &[Capability::Refinement],
        always(proved()),
    );
    let refinement = request(&dynamic, REFINEMENT);
    let conformance = || Claims::GlobalConformance(CONFORMANT);
    let alone = settle(
        &mut boundary,
        &dynamic,
        vec![conformance(), Claims::AssumeGuarantee(refinement.clone())],
    );
    let mut refusing = request(&dynamic, COMPOSITION);
    refusing.sources.clear();
    refusing.backend = Some(BackendId::new("elsewhere"));
    let together = settle(
        &mut boundary,
        &dynamic,
        vec![
            conformance(),
            Claims::AssumeGuarantee(refusing),
            Claims::AssumeGuarantee(refinement),
        ],
    );
    assert!(matches!(
        composed(together[1].clone()),
        CompositionOutcome::Refused(_)
    ));
    assert_eq!(together[0], alone[0]);
    assert_eq!(together[2], alone[1]);
}

#[test]
#[trace("TC-200", "FR-200-AC-4")]
fn a_conformance_result_neither_decides_nor_is_decided_by_a_composition() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic, COMPOSITION);
    let alone = compose(&dynamic, &asked);
    let beside = settle(
        &mut Boundary::new(),
        &dynamic,
        vec![
            Claims::GlobalConformance(CONFORMANT),
            Claims::AssumeGuarantee(asked.clone()),
        ],
    );
    assert_eq!(beside[0], ClaimDisposition::GlobalConformance(CONFORMANT));
    assert_eq!(composed(beside[1].clone()), alone);
    // A composition that is not proved stays so beside a conformant result.
    let mut unproved = asked;
    unproved.guarantees[0].label = ItemLabel::Inconclusive;
    let beside = settle(
        &mut Boundary::new(),
        &dynamic,
        vec![
            Claims::GlobalConformance(CONFORMANT),
            Claims::AssumeGuarantee(unproved),
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
    let whole = request(&dynamic, COMPOSITION);
    let alone = compose(&dynamic, &whole);
    assert_eq!(decided(alone.clone()).verdict(), Verdict::Proved);
    for (name, broken) in broken_components(&dynamic, &whole) {
        let settled = settle(
            &mut Boundary::new(),
            &dynamic,
            vec![
                Claims::AssumeGuarantee(broken),
                Claims::AssumeGuarantee(whole.clone()),
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
    let whole = request(&dynamic, COMPOSITION);
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
        let mut asked = request(&dynamic, COMPOSITION);
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
    let base = request(&dynamic, COMPOSITION);
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
    let mut asked = request(&dynamic, COMPOSITION);
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
    let asked = request(&dynamic, COMPOSITION);
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
    let composition = decided(compose(&dynamic, &request(&dynamic, COMPOSITION)));
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
    let mut asked = request(&dynamic, COMPOSITION);
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
    let mut asked = request(&dynamic, COMPOSITION);
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
    asked.premises = request(&dynamic, COMPOSITION).premises;
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
    let mut asked = request(&dynamic, COMPOSITION);
    asked.assumptions = vec![node(&dynamic, "Block")];
    asked.guarantees.clear();
    let composition = decided(compose(&dynamic, &asked));
    assert!(composition.discharges.is_empty());
    assert_eq!(composition.undischarged.len(), 4);
    assert_eq!(composition.verdict(), Verdict::Unproved);
}

// ---------------------------------------------------------------- TC-202

#[test]
#[trace("TC-202", "FR-202-AC-1")]
fn refinement_certificate_and_witness_appear_byte_equal() {
    let dynamic = Dynamic::with("workflow", 3);
    let out = Out {
        certificate: Some("certificate-bytes"),
        discharge: vec![("witness", "witness-bytes")],
        ..proved()
    };
    let (mut boundary, _) = serving(out.clone());
    let asked = request(&dynamic, REFINEMENT);
    assert_eq!(
        carried(one(&mut boundary, &dynamic, &asked)),
        honest(&asked, out)
    );
}

#[test]
#[trace("TC-202", "FR-202-AC-2")]
fn both_counterexample_halves_and_the_certificate_are_carried() {
    let dynamic = Dynamic::with("workflow", 3);
    let out = Out {
        certificate: Some("simulation-certificate"),
        implementation_half: Some("implementation-trace"),
        specification_half: Some("specification-trace"),
        ..Out::new(ItemLabel::Refuted)
    };
    let (mut boundary, _) = serving(out);
    let result = carried(one(&mut boundary, &dynamic, &request(&dynamic, REFINEMENT)));
    assert_eq!(result.certificate, Some("simulation-certificate"));
    assert_eq!(result.implementation_half, Some("implementation-trace"));
    assert_eq!(result.specification_half, Some("specification-trace"));
}

#[test]
#[trace("TC-202", "FR-202-AC-3")]
fn a_one_halved_counterexample_gains_no_second_half() {
    let dynamic = Dynamic::with("workflow", 3);
    let out = Out {
        implementation_half: Some("implementation-trace"),
        ..Out::new(ItemLabel::Refuted)
    };
    let (mut boundary, _) = serving(out);
    let result = carried(one(&mut boundary, &dynamic, &request(&dynamic, REFINEMENT)));
    assert_eq!(result.implementation_half, Some("implementation-trace"));
    assert_eq!(result.specification_half, None);
}

#[test]
#[trace("TC-202", "FR-202-AC-4")]
fn causes_are_carried_unchanged_and_none_is_a_success() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic, REFINEMENT);
    // The owner's typed inconclusive cause rides inside the result.
    let (mut boundary, _) = serving(Out {
        cause: Some(strengthened()),
        ..Out::new(ItemLabel::Inconclusive)
    });
    let outcome = one(&mut boundary, &dynamic, &asked);
    assert_eq!(carried(outcome).cause, Some(strengthened()));
    // Causes carried as the disposition's own are unchanged and unmapped.
    for cause in [
        UnsupportedCause::requested_capability(),
        UnsupportedCause::unrecognised(b"a-cause-the-library-does-not-recognise"),
    ] {
        let mut boundary = unsupported_with(cause.clone());
        let Disposition::Unsupported(unsupported) =
            disposition(one(&mut boundary, &dynamic, &asked))
        else {
            panic!("not unsupported");
        };
        assert_eq!(unsupported.cause, cause);
        assert_eq!(unsupported.truth(), Truth::Unavailable);
        assert_eq!(unsupported.basis(), SettlementBasis::Unavailable);
    }
}

#[test]
#[trace("TC-202", "FR-202-AC-5")]
fn a_refinement_label_reads_into_no_axis() {
    let dynamic = Dynamic::with("workflow", 3);
    for label in [
        ItemLabel::Proved,
        ItemLabel::Refuted,
        ItemLabel::Inconclusive,
        ItemLabel::Unsupported,
        ItemLabel::Failed,
        ItemLabel::Incomplete,
    ] {
        let (mut boundary, _) = serving(Out {
            certificate: Some("certificate-shaped"),
            ..Out::new(label)
        });
        let reading = one(&mut boundary, &dynamic, &request(&dynamic, REFINEMENT))
            .label_reading()
            .expect("a carried result");
        assert_eq!(
            (
                reading.label,
                reading.execution,
                reading.truth,
                reading.basis
            ),
            (label, None, None, None)
        );
        assert_eq!(read_refinement_label(label), reading);
    }
}

#[test]
#[trace("TC-202", "FR-202-AC-6")]
fn refused_is_never_derived_for_a_refinement() {
    let dynamic = Dynamic::with("workflow", 3);
    let (mut boundary, _) = serving(proved());
    let reading = one(&mut boundary, &dynamic, &request(&dynamic, REFINEMENT))
        .label_reading()
        .expect("a carried result");
    assert_ne!(reading.execution, Some(AssessmentExecution::Refused));
    let mut boundary = Boundary::new();
    serve(
        &mut boundary,
        "b",
        &BOTH,
        Box::new(|_| FslReport::Refusal(malformed())),
    );
    let Disposition::Refused(refusal) =
        disposition(one(&mut boundary, &dynamic, &request(&dynamic, REFINEMENT)))
    else {
        panic!("not refused");
    };
    assert_eq!(refusal.code, malformed());
}

#[test]
#[trace("TC-202", "FR-202-AC-7")]
fn an_unsupported_behaviour_or_missing_correspondence_is_never_satisfied() {
    let dynamic = Dynamic::with("workflow", 3);
    let cause = UnsupportedCause::unrecognised(b"missing-correspondence");
    let mut boundary = unsupported_with(cause.clone());
    let Disposition::Unsupported(unsupported) =
        disposition(one(&mut boundary, &dynamic, &request(&dynamic, REFINEMENT)))
    else {
        panic!("not unsupported");
    };
    assert_eq!(unsupported.cause, cause);
    assert_ne!(unsupported.truth(), Truth::Satisfied);
}

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
        request(dynamic, COMPOSITION),
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
    let own = with_cycle(
        &dynamic,
        request(&dynamic, COMPOSITION),
        &[("Reject", &["Reject"])],
    );
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
        request(&dynamic, COMPOSITION),
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
    let acyclic = request(&dynamic, COMPOSITION);
    let alone = compose(&dynamic, &acyclic);
    let beside = settle(
        &mut Boundary::new(),
        &dynamic,
        vec![
            Claims::AssumeGuarantee(cyclic(&dynamic)),
            Claims::AssumeGuarantee(acyclic),
        ],
    );
    assert_eq!(composed(beside[1].clone()), alone);
}

#[test]
#[trace("TC-203", "FR-203-AC-6")]
fn an_acyclic_composition_lists_no_cycle_whatever_the_declaration_order() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic, COMPOSITION);
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
    let mut asked = request(&dynamic, COMPOSITION);
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
#[trace("TC-200", "FR-200-AC-8")]
fn a_request_of_the_wrong_kind_is_refused() {
    let dynamic = Dynamic::with("workflow", 3);
    let declarations = dynamic.linked().declarations();
    assert_eq!(
        composition_refusal(decide(&[declarations], &request(&dynamic, REFINEMENT))),
        DispositionRefusal::NotAComposition
    );
    let (mut boundary, calls) = serving(proved());
    let mut carrying = request(&dynamic, REFINEMENT);
    carrying.guarantees = request(&dynamic, COMPOSITION).guarantees;
    assert_eq!(
        refused(one(&mut boundary, &dynamic, &carrying)),
        DispositionRefusal::RefinementCarriesCompositionInputs
    );
    let mut premised = request(&dynamic, REFINEMENT);
    premised.premises = request(&dynamic, COMPOSITION).premises;
    assert_eq!(
        refused(one(&mut boundary, &dynamic, &premised)),
        DispositionRefusal::RefinementCarriesCompositionInputs
    );
    assert_eq!(calls.len(), 0);
}

#[test]
#[trace("TC-200", "FR-200-AC-9")]
fn only_relied_guarantees_block_success_but_any_cycle_is_circular() {
    let dynamic = Dynamic::with("workflow", 3);
    let mut asked = request(&dynamic, COMPOSITION);
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

fn retaining(
    dynamic: &Dynamic,
    asked: &Req,
    assumptions: Option<Vec<CheckedNodeId>>,
    label: ItemLabel,
) -> Outcome {
    let mut out = honest(asked, Out::new(label));
    out.assumptions = assumptions;
    let mut boundary = Boundary::new();
    serve(&mut boundary, "b", &BOTH, always(out));
    one(&mut boundary, dynamic, asked)
}

fn stronger(dynamic: &Dynamic, asked: &Req) -> (Vec<CheckedNodeId>, CheckedNodeId) {
    let extra = dynamic.named("channel", "New");
    let mut assumptions = asked.assumptions.clone();
    assumptions.push(extra.clone());
    (assumptions, extra)
}

#[test]
#[trace("TC-204", "FR-204-AC-1")]
fn a_result_retaining_exactly_the_requests_assumptions_is_carried() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic, REFINEMENT);
    let outcome = retaining(
        &dynamic,
        &asked,
        Some(asked.assumptions.clone()),
        ItemLabel::Proved,
    );
    assert_eq!(carried(outcome).label, ItemLabel::Proved);
}

#[test]
#[trace("TC-204", "FR-204-AC-2")]
fn a_proof_under_extra_assumptions_is_refused_naming_each() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic, REFINEMENT);
    let (mut assumptions, extra) = stronger(&dynamic, &asked);
    let second = dynamic.named("channel", "Block");
    assumptions.push(second.clone());
    assert_eq!(
        refused(retaining(
            &dynamic,
            &asked,
            Some(assumptions),
            ItemLabel::Proved
        )),
        DispositionRefusal::IdentityMismatch(IdentityMismatch::Assumptions(vec![extra, second]))
    );
}

#[test]
#[trace("TC-204", "FR-204-AC-3")]
fn a_strict_subset_of_the_requests_assumptions_is_carried() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic, REFINEMENT);
    let subset = vec![asked.assumptions[0].clone()];
    assert!(matches!(
        retaining(&dynamic, &asked, Some(subset), ItemLabel::Proved),
        ProtocolOutcome::Settled(ClaimSettlement::Result { .. })
    ));
}

#[test]
#[trace("TC-204", "FR-204-AC-4")]
fn a_violation_under_an_extra_assumption_is_refused_too() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic, REFINEMENT);
    let (assumptions, extra) = stronger(&dynamic, &asked);
    let outcome = retaining(&dynamic, &asked, Some(assumptions), ItemLabel::Refuted);
    assert_eq!(
        refused(outcome),
        DispositionRefusal::IdentityMismatch(IdentityMismatch::Assumptions(vec![extra]))
    );
}

#[test]
#[trace("TC-204", "FR-204-AC-5")]
fn a_refused_refinement_leaves_other_claims_unchanged_and_calls_no_fallback() {
    let dynamic = Dynamic::with("workflow", 3);
    let mut boundary = Boundary::new();
    let composing = serve(
        &mut boundary,
        "composer",
        &[Capability::Composition],
        always(proved()),
    );
    let (_, extra) = stronger(&dynamic, &request(&dynamic, REFINEMENT));
    let refining = serve(
        &mut boundary,
        "refiner",
        &[Capability::Refinement],
        Box::new(move |bound| {
            let mut out = honest(&bound.request, proved());
            out.assumptions
                .as_mut()
                .expect("retained")
                .push(extra.clone());
            FslReport::Result(out)
        }),
    );
    let composition = request(&dynamic, COMPOSITION);
    let conformance = || ClaimRequest::GlobalConformance(CONFORMANT);
    let alone = settle(
        &mut boundary,
        &dynamic,
        vec![conformance(), Claims::AssumeGuarantee(composition.clone())],
    );
    let beside = settle(
        &mut boundary,
        &dynamic,
        vec![
            conformance(),
            Claims::AssumeGuarantee(request(&dynamic, REFINEMENT)),
            Claims::AssumeGuarantee(composition),
        ],
    );
    assert!(matches!(
        guaranteed(beside[1].clone()),
        ProtocolOutcome::Refused(_)
    ));
    assert_eq!(beside[0], alone[0]);
    assert_eq!(beside[2], alone[1]);
    assert_eq!(refining.len(), 1);
    assert_eq!(composing.len(), 0);
}

#[test]
#[trace("TC-204", "FR-204-AC-6")]
fn the_backends_own_strengthened_assumption_finding_is_carried_not_refused() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic, REFINEMENT);
    let finding = Out {
        cause: Some(strengthened()),
        ..Out::new(ItemLabel::Inconclusive)
    };
    let (mut boundary, _) = serving(finding.clone());
    let result = carried(one(&mut boundary, &dynamic, &asked));
    assert_eq!(result, honest(&asked, finding));
    assert_eq!(result.cause, Some(strengthened()));
}

#[test]
#[trace("TC-204", "FR-204-AC-7")]
fn an_omitted_field_is_refused_and_an_empty_set_is_a_strict_subset() {
    let dynamic = Dynamic::with("workflow", 3);
    let asked = request(&dynamic, REFINEMENT);
    assert_eq!(
        refused(retaining(&dynamic, &asked, None, ItemLabel::Proved)),
        DispositionRefusal::IdentityMismatch(IdentityMismatch::AbsentAssumptions)
    );
    assert!(matches!(
        retaining(&dynamic, &asked, Some(Vec::new()), ItemLabel::Proved),
        ProtocolOutcome::Settled(ClaimSettlement::Result { .. })
    ));
}

/// A composition whose liveness dependency `New` used `Tail`, which the
/// request offers only when `offer_tail`; `New` also used `Reject`, which used
/// `Head`.
fn needing_tail(dynamic: &Dynamic, offer_tail: bool) -> Req {
    let mut asked = request(dynamic, COMPOSITION);
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
