// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-179: the scenario record Quoin's intake takes, composed from the live
//! dispositions of real engine claims (FR-206, V1-RUN-005, V1-RUN-006).

mod support {
    pub mod dynamic;
    pub mod handoff;
    pub mod projection;
}

use std::cell::Cell;

use filament_protocol::bounds::inventory::{
    production_inventory, Budget, BudgetDimension, ResourceAccountant,
};
use filament_protocol::dispatch::{
    dispatch_claims, Binding, ClaimDisposition, ClaimRequest, ProviderBoundary,
};
use filament_protocol::engine::{engine_descriptor, EngineRequest, EngineResult, FslEngine};
use filament_protocol::outcome::OperationOutcome;
use filament_protocol::scenario::{compose, Scenario, Unexecuted};
use filament_protocol::version_lock::{EntryKey, ManifestEntry, VersionLockManifest};
use fsl_analyze::protocol_analysis::ProjectionRequest;
use fsl_replay::protocol_analysis::{ProjectionBoundsDraft, RunBudget};
use fsl_route::ProviderOrigin;
use ix_trace_rs::trace;
use quoin_evidence::scenario::{
    write_scenario_evidence, ClaimOutcome, Integration, ResolutionState, ScenarioEvidence,
    INTAKE_CONTRACT,
};
use quoin_evidence::{EvidenceSource, MemoryEvidence};
use support::dynamic::Dynamic;
use support::projection::{assessed_root, claim, worked, CANCELLED, PICK};

type Disposition = ClaimDisposition<(), (), filament_contract_model::CheckedNodeId, EngineResult>;

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

/// Dispatch the claims of `dynamic` through FSL's real engine; claim `n` asks
/// for the profile `profiles[n]` of the `Warehouse` projection.
fn dispatch(dynamic: &Dynamic, profiles: &'static [&'static str]) -> Vec<Disposition> {
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
    let requests: Vec<ClaimRequest<(), (), _>> = profiles
        .iter()
        .map(|_| ClaimRequest::Protocol(claim(dynamic)))
        .collect();
    dispatch_claims(
        &mut boundary,
        dynamic.linked().declarations(),
        &[],
        &requests,
    )
}

fn record_of(dispositions: Vec<Disposition>) -> Result<ScenarioEvidence, Unexecuted> {
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
        &OperationOutcome::Completed(dispositions),
        |()| unreachable!("no global-conformance claim is requested"),
    )
}

fn claim_identities(record: &ScenarioEvidence) -> Vec<&str> {
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
    let record = record_of(dispatch(&dynamic, &["bounded-static", "unbounded"]))
        .expect("every component is retained");
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
    let alone = record_of(dispatch(&dynamic, &["bounded-static"])).expect("composes");
    let together =
        record_of(dispatch(&dynamic, &["bounded-static", "unbounded"])).expect("composes");
    let (alone, together) = (claim_identities(&alone), claim_identities(&together));
    // The refined claim is violated on a decisive counterexample, the request
    // the engine refuses is not a row, and neither changes the other.
    assert_eq!(
        alone[0],
        "local-projection:completed:violated:decisive-counterexample"
    );
    assert_eq!(together[0], alone[0]);
    assert_eq!(together.len(), 2);
    assert!(
        together[1].starts_with("local-projection:refused:"),
        "{together:?}"
    );

    let sound = worked(PICK, CANCELLED);
    let satisfied =
        record_of(dispatch(&sound, &["bounded-static", "unbounded"])).expect("composes");
    let satisfied = claim_identities(&satisfied);
    assert_eq!(
        satisfied[0],
        "local-projection:completed:satisfied:closed-scope"
    );
    // The refusal of the second claim is the same whichever way the first went.
    assert_eq!(satisfied[1], together[1]);
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
            &OperationOutcome::Completed(dispatch(&dynamic, &["bounded-static"])),
            |()| unreachable!("no global-conformance claim is requested"),
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
