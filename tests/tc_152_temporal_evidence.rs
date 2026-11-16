// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-152: temporal evidence of recipe R from the checked package's own
//! nodes (FR-046).

mod support {
    pub mod handoff;
    pub mod limits;
}

use std::collections::BTreeMap;
use std::path::Path;

use filament_contract_model::{
    read_checked_package, CheckedNodeId, CheckedPackageDispatchResult, CheckedPackageReadLimits,
    CheckedPackageRefusalCause,
};
use filament_eval::{CallAdmissionFailure, CallFailure};
use filament_exact::Meter;
use filament_protocol::checked_package::{admit, AdmittedHandoff, HandoffFiles};
use filament_protocol::evidence::{assess, package, EvidenceRecord, EvidenceRefusal};
use filament_spec_language::checked_handoff::Corpus;
use fsl_eval::temporal::evidence::{
    ActivationState, CaptureFact, CapturedValue, CompletenessRecord, EvidencePackage,
    EvidencePosition, EvidenceRef, EvidenceRequest, ExecutionDisposition, ExecutionOrigin,
    Indeterminate, Payload, Receipt, Relation, SemanticTriggerIdentity, TemporalEvidenceLimits,
    TraceBinding, TriggerDelivery, TriggerEvidence,
};
use fsl_eval::temporal::{ClockCover, ProgressClosure, ProgressCompleteness, Valuation};
use fsl_package::{BarePackage, BareSupply};
use ix_trace_rs::trace;
use support::handoff::{join_budget, written};
use support::limits::UNLIMITED;

fn handoff(kind: &str) -> AdmittedHandoff {
    let handoff = written(kind, Corpus::None);
    let files = HandoffFiles {
        checked_package: &handoff.files["checked-package.json"],
        selection: &handoff.files["selection.json"],
        identities: &handoff.files["identities.json"],
        equality: &handoff.files["equality.json"],
    };
    admit(
        &files,
        &handoff.evidence,
        CheckedPackageReadLimits::bounded(),
        &mut join_budget(),
    )
    .expect("R admits")
}

fn bare(admitted: &AdmittedHandoff) -> BarePackage<'_> {
    BarePackage::new(admitted.package())
        .with_supply(
            BareSupply {
                identities: admitted.identities(),
                equality: Some(admitted.equality()),
            },
            &mut join_budget(),
        )
        .expect("the supply joins within the test budget")
}

fn id(text: &str) -> SemanticTriggerIdentity {
    SemanticTriggerIdentity::new(text.as_bytes()).expect("a non-empty identity")
}

/// An origin-activated request over `clause`, valuing its first atom true at
/// each of `ticks` positions and asserting the clause's own clock row.
fn request(package: &EvidencePackage<'_>, clause: &CheckedNodeId, ticks: u64) -> EvidenceRequest {
    let subject = package.subject(clause, None).expect("the clause reads");
    let row = subject.occurrences[0].clock.expect("a clocked clause");
    EvidenceRequest {
        trigger: Some(id("o1")),
        correspondence: EvidenceRef::named("corr"),
        positions: (0..ticks)
            .map(|index| EvidencePosition {
                observation: EvidenceRef::named(format!("obs{index}")),
                position: index,
                valuations: BTreeMap::from([(0, Valuation::True)]),
                ..EvidencePosition::default()
            })
            .collect(),
        anchor: Some("obs0".to_owned()),
        deliveries: Vec::new(),
        origins: vec![ExecutionOrigin {
            identity: id("o1"),
            payload: Payload::new(b"a".as_slice()),
            captures: Vec::new(),
        }],
        trigger_evidence: TriggerEvidence::Admitted,
        trigger_scope: ProgressClosure::Closed,
        decision_progress: Some(100),
        decision_closure: Some(ProgressClosure::Closed),
        surrounding_progress: Some(100),
        surrounding_closure: Some(ProgressClosure::Closed),
        execution: ExecutionDisposition::Completed,
        completeness: Some(CompletenessRecord {
            reference: EvidenceRef::named("complete"),
            state: ProgressCompleteness::Complete { through: 100 },
            facts: Vec::new(),
        }),
        authoritative_origin: Some(true),
        evicted: Vec::new(),
        cover: ClockCover::Exact,
        progress_ledger: None,
        ledger: None,
        relation: Relation::Original,
        control: None,
        binding: TraceBinding {
            profile: subject.facts.profile().identity().into(),
            clock_name: row.clock_name.clone(),
            parameters: row.clock.clone(),
        },
        occurrence: None,
        limits: TemporalEvidenceLimits::default(),
    }
}

fn clauses(package: &EvidencePackage<'_>) -> Vec<CheckedNodeId> {
    let mut ids: Vec<CheckedNodeId> = package
        .rows()
        .names()
        .iter()
        .map(|found| found.declaration.clone())
        .collect();
    ids.dedup();
    ids
}

fn unlimited() -> Meter {
    Meter::new(UNLIMITED)
}

/// FR-046-AC-1: over R's clauses the record is the activation, truth,
/// settlement and support FSL's evidence call reports, and a changed valuation
/// changes it.
#[test]
#[trace("FR-046-AC-1", "TC-152")]
fn the_record_is_what_fsls_evidence_call_reports() {
    let admitted = handoff("event_position");
    let bare = bare(&admitted);
    let evidence = package(&bare, &mut join_budget()).expect("R's package constructs");
    let ids = clauses(&evidence);
    assert_eq!(ids.len(), 2, "R's two named clauses");
    for clause in &ids {
        let asked = request(&evidence, clause, 3);
        let direct = evidence
            .evaluate_temporal_evidence(clause, &asked, &mut unlimited())
            .expect("FSL's call");
        let record = assess(&evidence, clause, &asked, &mut unlimited()).expect("the record");
        assert_eq!(record, EvidenceRecord::from(direct.result()));
        assert_eq!(record.activation, ActivationState::Active);
        assert_eq!(
            record.truth,
            Some(fsl_eval::temporal::evidence::Truth::True)
        );
        assert_eq!(
            record.settlement,
            Some(fsl_eval::temporal::evidence::Settlement::ClosedScope)
        );
        assert_eq!(record.support.len(), 3, "one supporting atom per position");

        let mut falsified = asked.clone();
        for position in &mut falsified.positions {
            position.valuations = BTreeMap::from([(0, Valuation::False)]);
        }
        let changed = assess(&evidence, clause, &falsified, &mut unlimited()).expect("record");
        assert_ne!(changed, record, "a changed valuation changes the record");
    }
}

fn rust_sources(dir: &Path, found: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).expect("src directory") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            rust_sources(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push((
                path.display().to_string(),
                std::fs::read_to_string(&path).expect("source"),
            ));
        }
    }
}

fn source_lines() -> Vec<(String, String)> {
    let mut files = Vec::new();
    rust_sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() > 10, "the scan reads the crate's sources");
    files
}

/// FR-046-AC-2: no source holds a retired contract string (spelled here from
/// parts so this file is not its own finding).
#[test]
#[trace("FR-046-AC-2", "TC-152")]
fn no_source_holds_a_retired_producer_contract() {
    let retired: Vec<String> = ["request", "result"]
        .iter()
        .flat_map(|kind| {
            ["v1", "v2"]
                .iter()
                .map(move |version| format!("filament.native-temporal-{kind}/{version}"))
        })
        .collect();
    assert_eq!(retired.len(), 4);
    for (path, text) in source_lines() {
        for contract in &retired {
            assert!(!text.contains(contract.as_str()), "{path} holds {contract}");
        }
    }
    // The control: the scan does see a string the crate does hold.
    assert!(source_lines()
        .iter()
        .any(|(_, text)| text.contains("ByteDigest")));
}

/// FR-046-AC-3: no source names the retired temporal types.
#[test]
#[trace("FR-046-AC-3", "TC-152")]
fn no_source_names_the_retired_native_temporal_types() {
    for (path, text) in source_lines() {
        let code: String = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("native_temporal"),
            "{path} names native_temporal"
        );
    }
}

fn cause(failure: &CallFailure) -> String {
    match failure {
        CallFailure::Admission(failure) => match &**failure {
            CallAdmissionFailure::Refused(record) => record
                .record
                .fields
                .get("cause")
                .cloned()
                .expect("a refusal names its cause"),
            other => panic!("a refusal, not {other:?}"),
        },
        other => panic!("an admission refusal, not {other:?}"),
    }
}

/// FR-046-AC-4: each cause of FSL FR-516's table that a request over R can
/// reach comes back by its name, unchanged, with nothing charged. The causes
/// a request over R cannot reach are not claimed here.
#[test]
#[trace("FR-046-AC-4", "TC-152")]
fn call_refusals_keep_their_cause_names_and_charge_nothing() {
    let admitted = handoff("event_position");
    let bare = bare(&admitted);
    let evidence = package(&bare, &mut join_budget()).expect("constructs");
    let ids = clauses(&evidence);
    let (clause, other) = (&ids[0], &ids[1]);
    let model = admitted
        .package()
        .graph()
        .nodes
        .iter()
        .find(|node| &*node.node_tag == "model")
        .expect("a model node")
        .node_id
        .clone();

    let base = request(&evidence, clause, 3);
    let mut no_trigger = base.clone();
    no_trigger.trigger = None;
    let mut unknown_trigger = base.clone();
    unknown_trigger.trigger = Some(id("nobody"));
    let mut unknown_occurrence = base.clone();
    unknown_occurrence.occurrence = Some(fsl_package::CheckedOccurrence {
        role: fsl_package::CheckedOccurrenceRole::Claim,
        ordinal: 99,
    });
    let mut missing_mapping = base.clone();
    missing_mapping.control = Some(model.clone());
    let mut foreign = request(&evidence, other, 3);
    foreign.binding.clock_name = "full-recovery".into();

    let cases: [(&str, &CheckedNodeId, &EvidenceRequest); 6] = [
        ("trigger-required", clause, &no_trigger),
        ("trigger-unknown", clause, &unknown_trigger),
        ("occurrence-unknown", clause, &unknown_occurrence),
        ("activation-mapping-missing", clause, &missing_mapping),
        ("clause-root-unresolved", &model, &base),
        ("binding-mismatch", clause, &foreign),
    ];
    for (expected, node, asked) in cases {
        let mut meter = unlimited();
        let direct = evidence
            .evaluate_temporal_evidence(node, asked, &mut unlimited())
            .expect_err(expected);
        assert_eq!(cause(&direct), expected, "FSL's own cause");
        let Err(EvidenceRefusal::Call(failure)) = assess(&evidence, node, asked, &mut meter) else {
            panic!("{expected}: a call refusal");
        };
        assert_eq!(
            failure, direct,
            "{expected}: the refusal is carried unchanged"
        );
        assert!(
            filament_exact::LimitKind::ALL
                .iter()
                .all(|kind| meter.consumed(*kind) == 0),
            "{expected}: nothing charged"
        );
    }
    // The control: the unedited request is answered.
    assert!(assess(&evidence, clause, &base, &mut unlimited()).is_ok());
}

/// FR-046-AC-5: the bytes reader refuses two name rows for one clause
/// occurrence as an invalid package with the cause `ConflictingDefinition`
/// (the reader's own cause for it), so no admitted package holds the rows that
/// would make FSL's package construction refuse as `AmbiguousName`.
#[test]
#[trace("FR-046-AC-5", "TC-152")]
fn the_reader_refuses_the_rows_that_would_be_ambiguous() {
    let handoff = written("event_position", Corpus::None);
    let mut json: serde_json::Value =
        serde_json::from_slice(&handoff.files["checked-package.json"]).expect("package JSON");
    let names = json["temporal_clause_names"]
        .as_array_mut()
        .expect("name rows");
    let first = names[0]["declaration"].clone();
    names[1]["declaration"] = first;
    let bytes = serde_json::to_vec(&json).expect("edited package");
    let result = read_checked_package(
        &bytes,
        CheckedPackageReadLimits::bounded(),
        &handoff.evidence,
    );
    assert!(
        matches!(
            &result,
            CheckedPackageDispatchResult::Refused(refusal)
                if refusal.cause == Some(CheckedPackageRefusalCause::ConflictingDefinition)
        ),
        "{result:?}"
    );
}

/// FR-046-AC-6 (the missing-anchor half): an anchor the request does not
/// hold, and a request with no positions, settle as a missing non-value and
/// never as a truth.
#[test]
#[trace("FR-046-AC-6", "TC-152")]
fn a_missing_anchor_settles_as_a_non_value_never_a_boolean() {
    let admitted = handoff("event_position");
    let bare = bare(&admitted);
    let evidence = package(&bare, &mut join_budget()).expect("constructs");
    let clause = &clauses(&evidence)[0];
    let base = request(&evidence, clause, 3);
    let held = assess(&evidence, clause, &base, &mut unlimited()).expect("a record");
    assert_eq!(held.truth, Some(fsl_eval::temporal::evidence::Truth::True));

    let mut no_anchor = base.clone();
    no_anchor.anchor = None;
    let mut unknown_anchor = base.clone();
    unknown_anchor.anchor = Some("withdrawn".to_owned());
    let mut no_positions = base.clone();
    no_positions.positions.clear();
    for (name, asked) in [
        ("no anchor", no_anchor),
        ("withdrawn anchor", unknown_anchor),
        ("no positions", no_positions),
    ] {
        let record = assess(&evidence, clause, &asked, &mut unlimited()).expect("a record");
        assert_eq!(record.truth, None, "{name} has no truth");
        assert_eq!(record.settlement, None, "{name} settles nothing");
        assert_eq!(record.activation, ActivationState::Unavailable, "{name}");
        assert_eq!(
            record.non_value.map(|value| value.kind),
            Some(fsl_eval::temporal::evidence::NonValueKind::Missing),
            "{name}"
        );
    }
}

/// FR-046-AC-1 (the pass-through half only; FR-046-AC-7 stays owed): the
/// request carries `Position.instance_valuations` on each position, FSL's
/// evidence call accepts it, and the record the library returns is the one the
/// call reports. R's clauses read their atoms from the shared valuations, so
/// the per-instance entries are accepted and not read; this test does not show
/// `instance_valuations` being read.
#[test]
#[trace("FR-046-AC-1", "TC-152")]
fn instance_valuations_cross_the_retained_surface_to_fsls_call() {
    let admitted = handoff("event_position");
    let bare = bare(&admitted);
    let evidence = package(&bare, &mut join_budget()).expect("R's package constructs");
    for clause in &clauses(&evidence) {
        let shared = request(&evidence, clause, 3);
        let mut carried = shared.clone();
        for position in &mut carried.positions {
            position.instance_valuations =
                BTreeMap::from([(id("o1"), BTreeMap::from([(0, Valuation::False)]))]);
        }
        let direct = evidence
            .evaluate_temporal_evidence(clause, &carried, &mut unlimited())
            .expect("FSL accepts the per-instance valuations");
        let record = assess(&evidence, clause, &carried, &mut unlimited()).expect("the record");
        assert_eq!(record, EvidenceRecord::from(direct.result()));
        let baseline = assess(&evidence, clause, &shared, &mut unlimited()).expect("baseline");
        assert_eq!(record, baseline, "R's atoms are shared, not instance-bound");
    }
}

/// FR-046-AC-7: the clause of the `instance_bound` kind reads an atom of the
/// trigger's binder, so FSL's evidence call reads that atom from
/// `Position.instance_valuations[trigger][atom]` and nowhere else: a true
/// entry, a false entry and an absent entry give three different records, and
/// the absent one is a missing valuation naming the instance.
#[test]
#[trace("FR-046-AC-7", "TC-152")]
fn the_evaluation_reads_the_instance_valuation_of_the_trigger() {
    let admitted = handoff("instance_bound");
    let bare = bare(&admitted);
    let evidence = package(&bare, &mut join_budget()).expect("the package constructs");
    let ids = clauses(&evidence);
    let [clause] = ids.as_slice() else {
        panic!("one clause: {ids:?}");
    };
    let trigger = id("t1");
    let asked = |entry: Option<Valuation>| {
        let mut asked = request(&evidence, clause, 4);
        asked.trigger = Some(trigger.clone());
        asked.origins = Vec::new();
        asked.deliveries = vec![TriggerDelivery {
            identity: trigger.clone(),
            receipt: Receipt::new(b"r1".as_slice()),
            anchor: "obs0".to_owned(),
            payload: Payload::new(b"a".as_slice()),
            guard: Some(true),
            captures: vec![CaptureFact::Value {
                anchor: "obs0".to_owned(),
                value: CapturedValue::new(b"5".as_slice()),
            }],
        }];
        for position in &mut asked.positions {
            position.valuations = BTreeMap::new();
            position.instance_valuations = entry
                .iter()
                .map(|valuation| (trigger.clone(), BTreeMap::from([(0, valuation.clone())])))
                .collect();
        }
        asked
    };
    let record =
        |entry| assess(&evidence, clause, &asked(entry), &mut unlimited()).expect("record");
    let held = record(Some(Valuation::True));
    let broken = record(Some(Valuation::False));
    let absent = record(None);
    assert_eq!(held.activation, ActivationState::Active);
    assert_eq!(held.truth, Some(fsl_eval::temporal::evidence::Truth::True));
    assert_eq!(
        broken.truth,
        Some(fsl_eval::temporal::evidence::Truth::False)
    );
    assert_ne!(held, broken);
    assert_eq!(absent.truth, None, "an absent entry gives no truth");
    assert!(
        matches!(
            absent.indeterminate,
            Some(Indeterminate::MissingValuation { atom: 0, instance: Some(ref named), .. })
                if *named == trigger
        ),
        "{:?}",
        absent.indeterminate
    );
}
