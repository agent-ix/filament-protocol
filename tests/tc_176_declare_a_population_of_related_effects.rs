// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-176: a declared population of related effects holds distinct members
//! that are not duplicates of one another (FR-023-AC-7).

use std::num::NonZeroU32;

use filament_protocol::compensation::algebra::{
    AttemptFact, AttemptLedger, AttemptOutcome, Definition, EffectDisposition, ForwardFact, Limits,
    Registry,
};
use filament_protocol::compensation::purity::{Capability, CapabilityInventory, PureAssessment};
use filament_protocol::compensation::recovery::{
    InvalidInput, Limits as RecoveryLimits, PopulationRequirement, Refusal, RelationSelection,
};
use filament_protocol::ids::{AttemptId, DefinitionId, EffectId, RecoveryId};
use ix_trace_rs::trace;

struct Pure;

impl CapabilityInventory for Pure {
    fn capabilities(&self) -> &[Capability] {
        &[]
    }
}

fn pure() -> PureAssessment {
    PureAssessment::checked(&Pure).expect("empty capability surface")
}

fn effect(name: &str) -> EffectId {
    EffectId::new(name.as_bytes(), name)
}

/// A ledger declaring `members` as its population (none when empty) whose
/// attempts, in order, establish `effects` (`None` for an attempt that
/// established none).
fn ledger(effects: &[Option<&str>], members: &[&str]) -> AttemptLedger {
    let forward = effect("effect:forward");
    let definition = Definition {
        identity: DefinitionId::new(b"definition:refund", "refund"),
        paired_forward_effect: forward.clone(),
        retry_bound: NonZeroU32::new(8),
    };
    let events = Registry::default()
        .observe(
            pure(),
            &[definition],
            &ForwardFact::EstablishedEffect(forward),
        )
        .expect("bounded registration");
    let registration = events[0].registration.clone();
    let mut ledger = AttemptLedger::new(registration.clone(), Limits::owner_max()).expect("ledger");
    if !members.is_empty() {
        let population = PopulationRequirement::new(
            "population:partial-refunds",
            "membership-rule:refund-parts",
            "digest:refund-parts",
            members.iter().copied().map(effect).collect(),
            RecoveryLimits::owner_max(),
        )
        .expect("declared population");
        let relation = RelationSelection::new(
            RecoveryId::new(b"recovery:refund", "recovery"),
            registration.identity.clone(),
            b"target:refund".as_slice(),
            b"relation:refund-parts".as_slice(),
            b"authored".as_slice(),
            Some(population),
            RecoveryLimits::owner_max(),
        )
        .expect("selected relation");
        ledger = ledger.with_population(&relation);
    }
    for (number, established) in effects.iter().enumerate() {
        ledger.admit(
            pure(),
            AttemptFact {
                attempt: AttemptId::new(format!("attempt:{number}").as_bytes(), "attempt"),
                registration: registration.identity.clone(),
                established_effect: established.map(effect),
                outcome: Some(AttemptOutcome::Successful),
            },
        );
    }
    ledger
}

const POPULATION: &[&str] = &["refund-1", "refund-2", "refund-3"];

#[test]
#[trace("TC-176", "FR-023-AC-7")]
fn distinct_population_members_are_not_duplicates_of_each_other() {
    let facts = ledger(
        &[Some("refund-1"), Some("refund-2"), Some("refund-3")],
        POPULATION,
    )
    .effects();
    assert_eq!(facts.len(), 3);
    assert!(facts.iter().all(|fact| fact.disposition.is_none()));

    // The same three effects with no declared population are a singleton
    // claimed twice over.
    let undeclared = ledger(&[Some("refund-1"), Some("refund-2"), Some("refund-3")], &[]).effects();
    assert_eq!(
        undeclared[1].disposition,
        Some(EffectDisposition::Duplicate)
    );
    assert_eq!(
        undeclared[2].disposition,
        Some(EffectDisposition::Duplicate)
    );
}

#[test]
#[trace("TC-176", "FR-023-AC-7")]
fn a_member_named_twice_is_replay_and_retained_once() {
    let facts = ledger(
        &[Some("refund-1"), Some("refund-2"), Some("refund-1")],
        POPULATION,
    )
    .effects();
    assert_eq!(facts.len(), 2);
    assert!(facts.iter().all(|fact| fact.disposition.is_none()));
}

#[test]
#[trace("TC-176", "FR-023-AC-7")]
fn effects_outside_the_population_are_recorded_as_outside_never_as_members() {
    let facts = ledger(
        &[
            Some("refund-1"),
            Some("stray-1"),
            Some("refund-2"),
            Some("stray-2"),
        ],
        POPULATION,
    )
    .effects();
    let dispositions: Vec<_> = facts.iter().map(|fact| fact.disposition).collect();
    assert_eq!(
        dispositions,
        [
            None,
            Some(EffectDisposition::OutsidePopulation),
            None,
            Some(EffectDisposition::OutsidePopulation)
        ],
        "membership is unknown for a stray: it is never a member and never a duplicate"
    );
    assert_eq!(facts[1].effect, effect("stray-1"));
}

#[test]
#[trace("TC-176", "FR-023-AC-7")]
fn a_population_with_no_member_or_a_repeated_member_is_refused() {
    let cases = [
        (vec![], InvalidInput::EmptyValue),
        (
            vec![effect("refund-1"), effect("refund-1")],
            InvalidInput::DuplicatePopulationMember,
        ),
    ];
    for (members, cause) in cases {
        let refused = PopulationRequirement::new(
            "population:partial-refunds",
            "membership-rule:refund-parts",
            "digest:refund-parts",
            members,
            RecoveryLimits::owner_max(),
        );
        assert_eq!(
            refused.err(),
            Some(Refusal::Invalid(cause)),
            "an unknown membership is not a population"
        );
    }
}
