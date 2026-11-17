// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-179: the scenario record Quoin's intake takes, composed from the live
//! dispositions of real engine claims (FR-206, V1-RUN-005, V1-RUN-006).

mod support {
    pub mod dynamic;
    pub mod handoff;
    pub mod projection;
    pub mod realizability;
}

use std::cell::Cell;

use filament_contract_model::{CheckedNodeId, CheckedNodePathSegment};
use filament_protocol::assume_guarantee::{AssumeGuaranteeRequest, PremiseLists};
use filament_protocol::bounds::inventory::{
    production_inventory, Budget, BudgetDimension, ResourceAccountant,
};
use filament_protocol::conformance::{
    engine_descriptor as conformance_descriptor, ConformanceResult, FslProtocolCheck,
};
use filament_protocol::dispatch::{
    dispatch, dispatch_claims, Binding, ClaimDisposition, ClaimRequest, DispositionRefusal,
    Fragment, ProtocolClaimKind, ProtocolOutcome, ProviderBoundary,
};
use filament_protocol::disposition::ClaimSettlement;
use filament_protocol::engine::{engine_descriptor, EngineRequest, EngineResult, FslEngine};
use filament_protocol::outcome::OperationOutcome;
use filament_protocol::scenario::{compose, Scenario, Unexecuted};
use filament_protocol::version_lock::{EntryKey, ManifestEntry, VersionLockManifest};
use fsl_analyze::analyze::ProtocolItem;
use fsl_analyze::model_check::ProtocolCheckRequest;
use fsl_analyze::protocol_analysis::ProjectionRequest;
use fsl_foundation::BackendId;
use fsl_replay::protocol_analysis::{ProjectionBoundsDraft, RunBudget};
use fsl_replay::{ModelCheckItem, ModelCheckLimits, ProtocolCertificateRequest, ReplayRefusal};
use fsl_route::ProviderOrigin;
use ix_trace_rs::trace;
use quoin_evidence::scenario::{
    write_scenario_evidence, ClaimOutcome, Integration, ResolutionState, ScenarioEvidence,
    INTAKE_CONTRACT,
};
use quoin_evidence::{EvidenceSource, MemoryEvidence};
use support::dynamic::Dynamic;
use support::projection::{assessed_root, claim, claim_of, worked, CANCELLED, PICK};
use support::realizability::Realizability;

type Request = ClaimRequest<ConformanceResult, (), CheckedNodeId>;
type Disposition = ClaimDisposition<ConformanceResult, (), CheckedNodeId, EngineResult>;

const SCENARIO: &str = "IT-001-SC-01";
const CRITERION: &str = "FR-206-AC-2";
const VALUE: &str = "ix://example/producer-value";

fn key() -> EntryKey {
    EntryKey::new(b"example-owner", b"1").expect("a key")
}

fn manifest() -> VersionLockManifest {
    VersionLockManifest::new(vec![ManifestEntry::pinned_interface(
        key(),
        Some(b"rev-1"),
        Some(b"published"),
        None,
    )])
}

fn node(name: &str) -> CheckedNodeId {
    CheckedNodeId {
        package: "ix://p".into(),
        path: vec![CheckedNodePathSegment::Name(name.into())],
        ordinal: 0,
    }
}

/// Dispatch the claims of `dynamic` through FSL's real engine; claim `n` asks
/// for the profile `profiles[n]` of the `Warehouse` projection, and `tamper`
/// may alter the requests.
fn dispatch_projections(
    dynamic: &Dynamic,
    profiles: &'static [&'static str],
    tamper: impl Fn(usize, &mut filament_protocol::dispatch::ProtocolRequest<()>),
) -> (Vec<Request>, Vec<Disposition>) {
    let call = Cell::new(0usize);
    let mut boundary = ProviderBoundary::<Binding<()>, EngineResult>::new();
    let engine = FslEngine::new(dynamic.package(), |_: &Binding<()>| {
        let profile = profiles[call.replace(call.get() + 1)];
        EngineRequest::Projection(Box::new(ProjectionRequest {
            protocol: dynamic.clause().clone(),
            definition_key: Some("ix://test/Flow".to_owned()),
            role: dynamic.named("role", "Warehouse"),
            profile: Some(profile.to_owned()),
            bounds: Some(ProjectionBoundsDraft {
                max_depth: Some(64),
            }),
            assessed_root: Some(assessed_root()),
            budget: RunBudget::default(),
        }))
    })
    .expect("FSL's linked engines register");
    boundary
        .register(
            engine_descriptor(ProviderOrigin::Linked).expect("FSL's own descriptor"),
            engine,
        )
        .expect("registered");
    let requests: Vec<Request> = (0..profiles.len())
        .map(|n| {
            let mut request = claim(dynamic);
            tamper(n, &mut request);
            ClaimRequest::Protocol(request)
        })
        .collect();
    let dispositions = dispatch_claims(
        &mut boundary,
        dynamic.linked().declarations(),
        &[],
        &requests,
    );
    (requests, dispositions)
}

fn untampered(_: usize, _: &mut filament_protocol::dispatch::ProtocolRequest<()>) {}

fn record_of(
    requests: &[Request],
    dispositions: Vec<Disposition>,
) -> Result<ScenarioEvidence, Unexecuted> {
    let budget = Budget::nothing_granted()
        .grant(BudgetDimension::Node, 1000)
        .expect("a positive grant");
    let inventory = production_inventory().expect("the inventory reconciles");
    let usage = ResourceAccountant::new(budget.clone(), &inventory).usage();
    let manifest = manifest();
    let key = key();
    compose(
        &Scenario {
            integration: Integration::It001,
            scenario: SCENARIO,
            manifest: &manifest,
            value_owners: &[(VALUE, &key)],
            budget: &budget,
            usage,
            criterion: CRITERION,
        },
        requests,
        &OperationOutcome::Completed(dispositions),
    )
}

fn identities(record: &ScenarioEvidence) -> Vec<&str> {
    record
        .outcome
        .claims
        .iter()
        .map(|claim| match claim {
            ClaimOutcome::Row { identity }
            | ClaimOutcome::Refusal { identity }
            | ClaimOutcome::ResourceIncomplete { identity } => identity.as_str(),
        })
        .collect()
}

#[test]
#[trace("TC-179", "FR-206-AC-1", "FR-206-AC-2")]
fn a_record_from_real_engine_dispositions_is_accepted_by_quoins_intake() {
    let dynamic = worked(PICK, "");
    let (requests, dispositions) =
        dispatch_projections(&dynamic, &["bounded-static", "unbounded"], untampered);
    let record = record_of(&requests, dispositions).expect("every component is retained");
    assert_eq!(record.contract, INTAKE_CONTRACT);
    assert!(matches!(record.resolution, ResolutionState::Full { .. }));
    assert_eq!(record.value_owners[0].owner, "example-owner");
    assert_eq!(record.value_owners[0].revision, "rev-1");
    // An absent grant stays absent; a grant stays a grant.
    let figure = |name: &str| {
        record
            .budget
            .iter()
            .find(|figure| figure.dimension == name)
            .expect("every dimension is retained")
    };
    assert_eq!(figure("node").granted, Some(1000));
    assert_eq!(figure("edge").granted, None);

    let mut store = MemoryEvidence::new();
    let document = serde_json::to_value(&record).expect("the owner's value serialises");
    let stored = write_scenario_evidence(&mut store, &document).expect("the intake accepts it");
    assert_eq!(stored.record, record);
    let text = store
        .read(&stored.path)
        .expect("a read")
        .expect("the record is filed");
    let filed: ScenarioEvidence = serde_json::from_str(&text).expect("the owner's shape");
    assert_eq!(filed, record);
}

#[test]
#[trace("TC-179", "FR-206-AC-2")]
fn the_axes_of_one_claim_do_not_change_with_another_claim() {
    let dynamic = worked(PICK, "");
    let (requests, dispositions) = dispatch_projections(&dynamic, &["bounded-static"], untampered);
    let alone = record_of(&requests, dispositions).expect("composes");
    let (requests, dispositions) =
        dispatch_projections(&dynamic, &["bounded-static", "unbounded"], untampered);
    let together = record_of(&requests, dispositions).expect("composes");
    let (alone, together) = (identities(&alone), identities(&together));
    // The projected claim is violated on a decisive counterexample, the request
    // the engine refuses is not a row, and neither changes the other.
    assert_eq!(
        alone[0],
        "0/local-projection:completed:violated:decisive-counterexample"
    );
    assert_eq!(together[0], alone[0]);
    assert_eq!(together.len(), 2);
    assert!(
        together[1].starts_with("1/local-projection:refused:"),
        "{together:?}"
    );

    let sound = worked(PICK, CANCELLED);
    let (requests, dispositions) =
        dispatch_projections(&sound, &["bounded-static", "unbounded"], untampered);
    let satisfied = record_of(&requests, dispositions).expect("composes");
    let satisfied = identities(&satisfied);
    assert_eq!(
        satisfied[0],
        "0/local-projection:completed:satisfied:closed-scope"
    );
    // The refusal of the second claim is the same whichever way the first went.
    assert_eq!(satisfied[1], together[1]);
}

#[test]
#[trace("TC-179", "FR-206-AC-2")]
fn two_claims_of_one_kind_and_axes_are_told_apart_by_position() {
    let dynamic = worked(PICK, "");
    let (requests, dispositions) =
        dispatch_projections(&dynamic, &["bounded-static", "bounded-static"], untampered);
    let record = record_of(&requests, dispositions).expect("composes");
    let identities = identities(&record);
    assert_eq!(identities.len(), 2);
    assert_ne!(identities[0], identities[1]);
    assert_eq!(
        identities[0].split_once('/').map(|(_, rest)| rest),
        identities[1].split_once('/').map(|(_, rest)| rest)
    );
}

#[test]
#[trace("TC-179", "FR-206-AC-2")]
fn a_refused_claim_keeps_its_kind() {
    let dynamic = worked(PICK, CANCELLED);
    // A fragment naming a role the checked package does not declare is refused
    // under FR-030 before any backend is asked.
    let (requests, dispositions) =
        dispatch_projections(&dynamic, &["bounded-static"], |_, request| {
            request.fragment.roles.push(node("NoSuchRole"));
        });
    assert!(matches!(
        &dispositions[0],
        ClaimDisposition::Protocol(ProtocolOutcome::Refused(
            DispositionRefusal::UndeclaredRole(_)
        ))
    ));
    let record = record_of(&requests, dispositions).expect("composes");
    assert_eq!(
        identities(&record),
        ["0/local-projection:refused:undeclared-role"]
    );
    assert!(matches!(
        record.outcome.claims[0],
        ClaimOutcome::Refusal { .. }
    ));

    // A composition that names no subject is refused with its own kind.
    let composition: Request = ClaimRequest::Composition(AssumeGuaranteeRequest {
        fragment: Fragment::default(),
        subjects: Vec::new(),
        assumptions: Vec::new(),
        premises: PremiseLists::default(),
        guarantees: Vec::new(),
        sources: Vec::new(),
    });
    let mut boundary = ProviderBoundary::<Binding<()>, EngineResult>::new();
    let dispositions = dispatch_claims(
        &mut boundary,
        dynamic.linked().declarations(),
        &[dynamic.linked().declarations()],
        std::slice::from_ref(&composition),
    );
    let ClaimDisposition::Composition(refused) = &dispositions[0] else {
        panic!("a composition disposition");
    };
    let code = match refused {
        filament_protocol::composition::CompositionOutcome::Refused(refusal) => refusal.code(),
        other => panic!("refused under FR-030: {other:?}"),
    };
    let record = record_of(std::slice::from_ref(&composition), dispositions).expect("composes");
    assert_eq!(
        identities(&record),
        [format!("0/composition:refused:{code}")]
    );
}

type Checked<'p> = ProviderBoundary<'p, Binding<()>, ConformanceResult>;

/// A deadlock-freedom item over the fixture's protocol; a handoff carries no
/// source to recompile, so the caller's replay refuses.
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

fn checked(fixture: &Realizability) -> Checked<'_> {
    let mut boundary = Checked::new();
    boundary
        .register(
            conformance_descriptor(ProviderOrigin::Linked).expect("FSL's own descriptor"),
            FslProtocolCheck::new(move |_: &Binding<()>| item(fixture)),
        )
        .expect("the registration is accepted");
    boundary
}

fn conformance_claim(fixture: &Realizability) -> filament_protocol::dispatch::ProtocolRequest<()> {
    claim_of(
        ProtocolClaimKind::GlobalConformance,
        fixture.linked(),
        fixture.clause(),
        |form, name| fixture.named(form, name),
    )
}

/// The real checker's result for the fixture, if it settles one.
fn conformance_result(fixture: &Realizability) -> Option<ConformanceResult> {
    match dispatch(
        &mut checked(fixture),
        fixture.linked().declarations(),
        &conformance_claim(fixture),
    ) {
        ProtocolOutcome::Settled(ClaimSettlement::Result { result, .. }) => Some(result),
        _ => None,
    }
}

fn compose_conformance(
    requests: &[ClaimRequest<ConformanceResult, (), CheckedNodeId>],
    dispositions: Vec<ClaimDisposition<ConformanceResult, (), CheckedNodeId, ConformanceResult>>,
) -> ScenarioEvidence {
    let budget = Budget::nothing_granted();
    let inventory = production_inventory().expect("the inventory reconciles");
    let usage = ResourceAccountant::new(budget.clone(), &inventory).usage();
    let manifest = manifest();
    compose(
        &Scenario {
            integration: Integration::It001,
            scenario: SCENARIO,
            manifest: &manifest,
            value_owners: &[],
            budget: &budget,
            usage,
            criterion: CRITERION,
        },
        requests,
        &OperationOutcome::Completed(dispositions),
    )
    .expect("composes")
}

#[test]
#[trace("TC-179", "FR-206-AC-2")]
fn a_real_global_conformance_outcome_is_read_with_its_own_execution_and_truth() {
    // A passed-through result of FSL's checker is read, not assumed completed.
    let sound = Realizability::worked(PICK, CANCELLED);
    let result = conformance_result(&sound).expect("the checker decides the claim");
    let record = compose_conformance(
        &[ClaimRequest::GlobalConformance(result.clone())],
        vec![ClaimDisposition::GlobalConformance(result)],
    );
    assert_eq!(
        identities(&record),
        ["0/global-conformance:completed:satisfied:closed-scope"]
    );

    // A deadlock whose counterexample FSL cannot replay is not violated.
    let extra = support::realizability::channel("Cap", "Seller", "Warehouse", "PickMsg")
        .replace("capacity 4 overflow reject", "capacity 1 overflow block");
    let ship = "send C1 via Cap as (c1: PickMsg) { true };\n\
        send C2 via Cap as (c2: PickMsg) { true };";
    let deadlocked = Realizability::worked_with(&extra, ship, CANCELLED);
    let result = conformance_result(&deadlocked).expect("the checker settles a record");
    let record = compose_conformance(
        &[ClaimRequest::GlobalConformance(result.clone())],
        vec![ClaimDisposition::GlobalConformance(result)],
    );
    assert!(!identities(&record)[0].contains(":violated:"));

    // A requires-bound outcome of the same checker is a refusal, not a row and
    // not completed.
    let unbounded = Realizability::unbounded();
    let request = ClaimRequest::Protocol(conformance_claim(&unbounded));
    let dispositions = dispatch_claims::<ConformanceResult, (), CheckedNodeId, ConformanceResult>(
        &mut checked(&unbounded),
        unbounded.linked().declarations(),
        &[],
        std::slice::from_ref(&request),
    );
    let record = compose_conformance(std::slice::from_ref(&request), dispositions);
    assert_eq!(
        identities(&record),
        ["0/temporal-satisfaction:refused:requires-bound"]
    );
    assert!(matches!(
        record.outcome.claims[0],
        ClaimOutcome::Refusal { .. }
    ));
}

#[test]
#[trace("TC-179", "FR-206-AC-3")]
fn a_scenario_that_cannot_retain_a_component_is_unexecuted() {
    let dynamic = worked(PICK, CANCELLED);
    let budget = Budget::nothing_granted();
    let inventory = production_inventory().expect("the inventory reconciles");
    let usage = ResourceAccountant::new(budget.clone(), &inventory).usage();
    let key = key();
    let compose_with = |manifest: &VersionLockManifest, owners: &[(&str, &EntryKey)]| {
        let (requests, dispositions) =
            dispatch_projections(&dynamic, &["bounded-static"], untampered);
        compose(
            &Scenario {
                integration: Integration::It001,
                scenario: SCENARIO,
                manifest,
                value_owners: owners,
                budget: &budget,
                usage,
                criterion: CRITERION,
            },
            &requests,
            &OperationOutcome::Completed(dispositions),
        )
    };
    assert_eq!(
        compose_with(&VersionLockManifest::new(Vec::new()), &[]).err(),
        Some(Unexecuted::NoResolution)
    );
    assert_eq!(
        compose_with(
            &manifest(),
            &[(VALUE, &EntryKey::new(b"other", b"1").expect("a key"))]
        )
        .err(),
        Some(Unexecuted::ValueOwnerUnresolved(VALUE.to_owned()))
    );
    let no_revision = VersionLockManifest::new(vec![ManifestEntry::pinned_interface(
        key.clone(),
        None,
        None,
        None,
    )]);
    assert_eq!(
        compose_with(&no_revision, &[]).err(),
        Some(Unexecuted::NoRevision)
    );
    let not_text = VersionLockManifest::new(vec![ManifestEntry::pinned_interface(
        EntryKey::new(&[0xff, 0xfe], b"1").expect("a key"),
        Some(b"rev-1"),
        None,
        None,
    )]);
    let refused = compose_with(&not_text, &[]).err();
    assert_eq!(refused, Some(Unexecuted::NotText("owner")));
    assert!(!refused.expect("refused").to_string().is_empty());

    // Dispositions that are not one per requested claim are not a record.
    let (requests, mut dispositions) =
        dispatch_projections(&dynamic, &["bounded-static"], untampered);
    dispositions.clear();
    assert_eq!(
        record_of(&requests, dispositions).err(),
        Some(Unexecuted::DispositionsNotPerRequest {
            requested: 1,
            returned: 0
        })
    );

    // A scenario that refused during resolution carries the partial state and
    // the causing entry, and is not unexecuted for lacking a full manifest.
    let refused = VersionLockManifest::new(vec![
        ManifestEntry::pinned_interface(key.clone(), Some(b"rev-1"), None, None),
        ManifestEntry::unresolved_interface(
            EntryKey::new(b"late-owner", b"2").expect("a key"),
            b"not published",
        ),
    ]);
    let record = compose_with(&refused, &[(VALUE, &key)]).expect("a partial state is present");
    let ResolutionState::Partial {
        reached,
        causing_entry,
    } = record.resolution
    else {
        panic!("a partial state");
    };
    assert_eq!(reached.len(), 1);
    assert_eq!(causing_entry.owner, "late-owner");
    assert_eq!(causing_entry.reason, "not published");
}

/// The refusal codes are a closed wire vocabulary: a variant renamed in source
/// keeps its code, and this table is what notices a code that moved.
#[test]
#[trace("TC-179", "FR-206-AC-2")]
fn every_disposition_refusal_has_its_pinned_code() {
    let id = || node("N");
    let cases = [
        (
            DispositionRefusal::UndeclaredChannel(id()),
            "undeclared-channel",
        ),
        (DispositionRefusal::UndeclaredRole(id()), "undeclared-role"),
        (
            DispositionRefusal::AbsentChannelPremise(id()),
            "absent-channel-premise",
        ),
        (
            DispositionRefusal::AbsentRoleInternality(id()),
            "absent-role-internality",
        ),
        (DispositionRefusal::NoSourceIdentity, "no-source-identity"),
        (
            DispositionRefusal::SubstitutedChannelPremise(id()),
            "substituted-channel-premise",
        ),
        (
            DispositionRefusal::DecisiveWithoutWitness,
            "decisive-without-witness",
        ),
        (
            DispositionRefusal::WitnessWithoutDecisiveBasis,
            "witness-without-decisive-basis",
        ),
        (
            DispositionRefusal::WitnessNotTheRecords,
            "witness-not-the-records",
        ),
        (
            DispositionRefusal::RecordNotTheResults,
            "record-not-the-results",
        ),
        (
            DispositionRefusal::ReplayWithoutItsRecord,
            "replay-without-its-record",
        ),
        (
            DispositionRefusal::NoProvider(BackendId::new("backend")),
            "no-provider",
        ),
        (DispositionRefusal::NoSubjects, "no-subjects"),
        (DispositionRefusal::NoFragment, "no-fragment"),
        (
            DispositionRefusal::UndeclaredSubject(id()),
            "undeclared-subject",
        ),
        (
            DispositionRefusal::UndeclaredAssumption(id()),
            "undeclared-assumption",
        ),
        (
            DispositionRefusal::UndeclaredSource(id()),
            "undeclared-source",
        ),
        (
            DispositionRefusal::CycleGuaranteeAsAssumption(id()),
            "cycle-guarantee-as-assumption",
        ),
        (
            DispositionRefusal::UndeclaredGuarantee(id()),
            "undeclared-guarantee",
        ),
        (
            DispositionRefusal::UndeclaredPremise(id()),
            "undeclared-premise",
        ),
        (
            DispositionRefusal::DuplicateGuarantee(id()),
            "duplicate-guarantee",
        ),
        (
            DispositionRefusal::UndeclaredLabel(id()),
            "undeclared-label",
        ),
    ];
    for (refusal, code) in &cases {
        assert_eq!(refusal.code(), *code);
    }
    let codes: std::collections::BTreeSet<_> = cases.iter().map(|(_, code)| *code).collect();
    assert_eq!(codes.len(), cases.len());
}
