// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-151: the compensation slots of recipe R evaluated through the
//! checked-package evaluation entry (FR-045).

mod support {
    pub mod handoff;
    pub mod limits;
}

use ix_trace_rs::trace;
use std::num::NonZeroU32;
use std::path::Path;

use filament_contract_model::CheckedPackageReadLimits;
use filament_eval::CallFailure;
use filament_exact::{
    from_admitted, CardinalityBound, CollectionKind, CollectionType, FieldValue, Integer,
    LimitKind, Meter, NodeKey, Quantity, Rational, Value, ValueType,
};
use filament_protocol::checked_package::{admit, AdmittedHandoff, HandoffFiles};
use filament_protocol::compensation::algebra::{
    AttemptAdmission, AttemptFact, AttemptLedger, AttemptOutcome, Definition, ForwardFact,
    Limits as AlgebraLimits, Registry,
};
use filament_protocol::compensation::purity::{Capability, CapabilityInventory, PureAssessment};
use filament_protocol::ids::{AttemptId, DefinitionId, EffectId};
use filament_protocol::slot::{attempts_maximum, evaluate, SlotEvaluation, SlotOutcome};
use fsl_command::checked_handoff::Corpus;
use fsl_eval::value::{ProtocolSlot, ProtocolSuppliedValue, ProtocolSupply, SuppliedValue};
use fsl_package::{BarePackage, BareSupply, ProtocolSlotDescriptor};
use fsl_semantics::check::wire_id::WireGraph;
use support::handoff::{join_budget, written};
use support::limits::UNLIMITED;

fn handoff() -> AdmittedHandoff {
    let handoff = written("event_position", Corpus::None);
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

fn compensation_key(admitted: &AdmittedHandoff, name: &str) -> NodeKey {
    let package = admitted.package();
    let node = package
        .resolved_graph()
        .nodes
        .iter()
        .find(|node| {
            &*node.semantic_form == "compensation"
                && node.declaration.as_ref().is_some_and(|declaration| {
                    declaration.qualified_name.last().map(|part| &**part) == Some(name)
                })
        })
        .expect("compensation node");
    WireGraph::new(
        package
            .resolved_graph()
            .nodes
            .iter()
            .map(|node| &node.node_id),
    )
    .expect("wire ids")
    .key(&node.node_id)
    .expect("key")
}

fn int(value: i64) -> Value {
    Value::Integer(value.into())
}

/// A slot of the compensation `name` and the supply of `values` by binder
/// name; `None` supplies the binder as unavailable.
fn call(
    bare: &BarePackage<'_>,
    node: &NodeKey,
    member: &str,
    values: Vec<(&str, Option<Value>)>,
) -> (ProtocolSlot, ProtocolSupply<'static>) {
    let selected = bare
        .select_protocol_slot(&ProtocolSlotDescriptor::new(node.clone(), member, None))
        .expect("slot selects");
    let binders = values
        .into_iter()
        .map(|(name, value)| {
            let source = selected
                .sources()
                .iter()
                .find(|source| source.name() == name)
                .unwrap_or_else(|| panic!("{member} reads {name}"));
            let supplied = match value {
                Some(value) => SuppliedValue::Value(value),
                None => {
                    SuppliedValue::Unavailable(filament_semantic_value::catalog::CatalogCode::new(
                        "unavailable_observation",
                        "missing-required-artifact",
                    ))
                }
            };
            (
                source.binder().clone(),
                ProtocolSuppliedValue {
                    value: supplied,
                    references: Vec::new(),
                },
            )
        })
        .collect();
    (
        ProtocolSlot {
            node: node.clone(),
            member: member.to_owned(),
            index: None,
        },
        ProtocolSupply {
            binders,
            observation: None,
        },
    )
}

fn record_key(bare: &BarePackage<'_>, node: &NodeKey, member: &str, binder: &str) -> NodeKey {
    let selected = bare
        .select_protocol_slot(&ProtocolSlotDescriptor::new(node.clone(), member, None))
        .expect("slot selects");
    let source = selected
        .sources()
        .iter()
        .find(|source| source.name() == binder)
        .expect("binder");
    let ValueType::Composite(key) = source.value_type() else {
        panic!("{binder} is a record");
    };
    key.clone()
}

fn notice(bare: &BarePackage<'_>, node: &NodeKey, name: &str, ready: bool) -> Value {
    let key = record_key(bare, node, "trigger.guard", &format!("trigger{name}"));
    bare.world()
        .expect("world")
        .types()
        .record(
            key,
            vec![("ready", FieldValue::Present(Value::Boolean(ready)))],
        )
        .expect("Notice")
}

fn attempt(bare: &BarePackage<'_>, node: &NodeKey, name: &str, tally: i64) -> Value {
    let key = record_key(bare, node, "retry.relation", &format!("earlier{name}"));
    bare.world()
        .expect("world")
        .types()
        .record(key, vec![("tally", FieldValue::Present(int(tally)))])
        .expect("Attempt")
}

fn guard(admitted: &AdmittedHandoff, name: &str, ready: bool) -> SlotEvaluation {
    let bare = bare(admitted);
    let node = compensation_key(admitted, name);
    let value = notice(&bare, &node, name, ready);
    let (slot, supply) = call(
        &bare,
        &node,
        "trigger.guard",
        vec![(&format!("trigger{name}"), Some(value))],
    );
    evaluate(&bare, &slot, &supply, &mut Meter::new(UNLIMITED)).expect("guard evaluates")
}

fn retry(
    admitted: &AdmittedHandoff,
    name: &str,
    tallies: (i64, i64),
    target: i64,
) -> SlotEvaluation {
    let bare = bare(admitted);
    let node = compensation_key(admitted, name);
    let values = [
        (
            format!("target{name}"),
            Some(total(&bare, &node, name, target)),
        ),
        (format!("activated{name}"), Some(Value::Boolean(false))),
        (
            format!("earlier{name}"),
            Some(attempt(&bare, &node, name, tallies.0)),
        ),
        (
            format!("later{name}"),
            Some(attempt(&bare, &node, name, tallies.1)),
        ),
    ];
    let values = values
        .iter()
        .map(|(n, v)| (n.as_str(), v.clone()))
        .collect();
    let (slot, supply) = call(&bare, &node, "retry.relation", values);
    evaluate(&bare, &slot, &supply, &mut Meter::new(UNLIMITED)).expect("retry evaluates")
}

/// A `Total` of `amount` for the retry relation of the compensation `name`.
fn total(bare: &BarePackage<'_>, node: &NodeKey, name: &str, amount: i64) -> Value {
    let unit = unit_of(bare, node, "retry.relation", &format!("target{name}"));
    quantity(&unit, amount)
}

fn holds(evaluation: &SlotEvaluation) -> bool {
    match &evaluation.outcome {
        SlotOutcome::Holds => true,
        SlotOutcome::DoesNotHold => false,
        other => panic!("expected a Boolean outcome, got {other:?}"),
    }
}

/// FR-045-AC-1: the outcome-map rows over R's `Full` and `Partial`
/// compensations that evaluate today: true and false `trigger.guard` and
/// `retry.relation`, an unavailable binder, a wrongly typed value and a meter
/// one below measured usage. R's `recovery.predicate` rows are in
/// `the_recovery_predicate_rows_have_their_successor_outcomes`.
#[test]
#[trace("FR-045-AC-1", "TC-151")]
fn every_outcome_row_has_its_successor_outcome() {
    let admitted = handoff();
    for name in ["Full", "Partial"] {
        assert!(holds(&guard(&admitted, name, false)), "{name} guard true");
        assert!(!holds(&guard(&admitted, name, true)), "{name} guard false");
        assert!(
            holds(&retry(&admitted, name, (0, 1), 5)),
            "{name} retry true"
        );
        assert!(
            !holds(&retry(&admitted, name, (3, 1), 5)),
            "{name} retry false"
        );
        assert!(
            !holds(&retry(&admitted, name, (0, 2), 5)),
            "{name} retry: tally must advance by one"
        );
    }

    let bare = bare(&admitted);
    let node = compensation_key(&admitted, "Full");

    // A binder supplied unavailable is the owner's Unavailable, naming it.
    let unavailable = {
        let notice = notice(&bare, &node, "Full", false);
        let _ = notice;
        let (slot, supply) = call(&bare, &node, "trigger.guard", vec![("triggerFull", None)]);
        evaluate(&bare, &slot, &supply, &mut Meter::new(UNLIMITED)).expect("call")
    };
    let SlotOutcome::Unavailable { binder, cause } = &unavailable.outcome else {
        panic!("expected Unavailable, got {:?}", unavailable.outcome);
    };
    let selected = bare
        .select_protocol_slot(&ProtocolSlotDescriptor::new(
            node.clone(),
            "trigger.guard",
            None,
        ))
        .expect("selects");
    assert_eq!(binder, selected.sources()[0].binder());
    assert_eq!(
        *cause,
        filament_semantic_value::catalog::CatalogCode::new(
            "unavailable_observation",
            "missing-required-artifact"
        )
    );

    // A value of the wrong type is refused at admission with nothing charged.
    let (slot, mut supply) = call(
        &bare,
        &node,
        "trigger.guard",
        vec![("triggerFull", Some(notice(&bare, &node, "Full", false)))],
    );
    for (_, supplied) in &mut supply.binders {
        supplied.value = SuppliedValue::Value(Value::Boolean(true));
    }
    let mut meter = Meter::new(UNLIMITED);
    let failure = evaluate(&bare, &slot, &supply, &mut meter).expect_err("wrong type");
    assert!(matches!(failure, CallFailure::Admission(_)), "{failure:?}");
    assert_eq!(meter.consumed(LimitKind::WorkUnits), 0, "nothing charged");

    // A meter one below measured usage stops the call as Incomplete; the
    // exact usage completes.
    let (slot, supply) = call(
        &bare,
        &node,
        "trigger.guard",
        vec![("triggerFull", Some(notice(&bare, &node, "Full", false)))],
    );
    let mut measured = Meter::new(UNLIMITED);
    evaluate(&bare, &slot, &supply, &mut measured).expect("measured");
    let used = measured.consumed(LimitKind::WorkUnits);
    assert!(used > 0);
    let mut below = UNLIMITED;
    below.work_units = used - 1;
    let stopped = evaluate(&bare, &slot, &supply, &mut Meter::new(below)).expect("call");
    assert!(
        matches!(stopped.outcome, SlotOutcome::Incomplete(_)),
        "{:?}",
        stopped.outcome
    );
    let mut exact = UNLIMITED;
    exact.work_units = used;
    let at_budget = evaluate(&bare, &slot, &supply, &mut Meter::new(exact)).expect("call");
    assert!(holds(&at_budget), "the exact budget completes");
}

/// FR-045-AC-3 (the part that runs today): an unavailable binder of the retry
/// relation is never a true result, and no source converts an evaluation to
/// `bool`. The recovery supplies are in
/// `a_recovery_supply_that_is_not_complete_never_yields_a_true_recovery`.
#[test]
#[trace("FR-045-AC-3", "TC-151")]
fn an_unavailable_binder_is_never_a_true_result() {
    let admitted = handoff();
    let bare = bare(&admitted);
    let node = compensation_key(&admitted, "Full");
    for missing in ["targetFull", "activatedFull", "earlierFull", "laterFull"] {
        let values = [
            ("targetFull", total(&bare, &node, "Full", 5)),
            ("activatedFull", Value::Boolean(false)),
            ("earlierFull", attempt(&bare, &node, "Full", 0)),
            ("laterFull", attempt(&bare, &node, "Full", 1)),
        ]
        .into_iter()
        .map(|(name, value)| (name, (name != missing).then_some(value)))
        .collect();
        let (slot, supply) = call(&bare, &node, "retry.relation", values);
        let result = evaluate(&bare, &slot, &supply, &mut Meter::new(UNLIMITED)).expect("call");
        assert!(
            matches!(result.outcome, SlotOutcome::Unavailable { .. }),
            "{missing}: {:?}",
            result.outcome
        );
    }
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    for (path, text) in &files {
        for forbidden in ["unwrap_or(false)", "as_bool", "Into<bool>"] {
            assert!(!text.contains(forbidden), "{path} names {forbidden}");
        }
    }
}

fn sources(dir: &Path, found: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).expect("src directory") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push((
                path.display().to_string(),
                std::fs::read_to_string(&path).expect("source"),
            ));
        }
    }
}

#[derive(Default)]
struct NoCapabilities(Vec<Capability>);

impl CapabilityInventory for NoCapabilities {
    fn capabilities(&self) -> &[Capability] {
        &self.0
    }
}

/// FR-045-AC-4: the bound read from `attempts maximum` admits exactly that
/// many attempts, counting the initial one, and one more is the refusal.
#[test]
#[trace("FR-045-AC-4", "TC-151")]
fn the_attempts_maximum_bounds_the_attempt_ledger() {
    let admitted = handoff();
    let package = admitted.package();
    let mut bounds = Vec::new();
    for name in ["Full", "Partial"] {
        let node = package
            .graph()
            .nodes
            .iter()
            .find(|node| {
                &*node.semantic_form == "compensation"
                    && node
                        .declaration
                        .as_ref()
                        .is_some_and(|d| d.qualified_name.last().map(|part| &**part) == Some(name))
            })
            .expect("compensation");
        bounds.push(attempts_maximum(node).expect("authored bound"));
    }
    assert_eq!(bounds, [NonZeroU32::new(3), NonZeroU32::new(3)]);

    let inventory = NoCapabilities::default();
    let pure = || PureAssessment::checked(&inventory).expect("empty surface");
    let forward = EffectId::new(b"effect:forward", "forward");
    let definition = Definition {
        identity: DefinitionId::new(b"definition:full", "Full"),
        paired_forward_effect: forward.clone(),
        retry_bound: bounds[0],
    };
    let registration = Registry::default()
        .observe(
            pure(),
            &[definition],
            &ForwardFact::EstablishedEffect(forward),
        )
        .expect("registers")[0]
        .registration
        .clone();
    let mut ledger =
        AttemptLedger::new(registration.clone(), AlgebraLimits::owner_max()).expect("ledger");
    let fact = |index: u8| AttemptFact {
        attempt: AttemptId::new(&[index], "attempt"),
        registration: registration.identity.clone(),
        established_effect: None,
        outcome: Some(AttemptOutcome::Failed),
    };
    for index in 1..=3u8 {
        assert!(
            matches!(
                ledger.admit(pure(), fact(index)),
                AttemptAdmission::Admitted { position, .. } if position == usize::from(index)
            ),
            "attempt {index} of 3"
        );
    }
    assert!(matches!(
        ledger.admit(pure(), fact(4)),
        AttemptAdmission::RetryBoundExceeded { bound, .. } if bound.get() == 3
    ));
    assert_eq!(ledger.attempts().len(), 3);
}

/// FR-045-AC-6: no source file names the retired state module's items.
#[test]
#[trace("FR-045-AC-6", "TC-151")]
fn no_source_names_the_retired_state_module() {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() > 10);
    for (path, text) in &files {
        for retired in [
            "StateView",
            "EvaluationReport",
            "state::evaluate_v2",
            "ACCOUNTING_VERSION",
        ] {
            let named = text
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .any(|line| line.contains(retired));
            assert!(!named, "{path} names {retired}");
        }
    }
}

/// FR-045-AC-8: the charge log has an entry for each binder read, none for a
/// binder the expression did not read, and the entries with the unattributed
/// one equal the call's total; the log carries its accounting version.
#[test]
#[trace("FR-045-AC-8", "TC-151")]
fn the_charge_log_attributes_each_binder_read() {
    let admitted = handoff();
    let bare = bare(&admitted);
    let node = compensation_key(&admitted, "Full");
    let values = vec![
        ("targetFull", Some(total(&bare, &node, "Full", 5))),
        ("activatedFull", Some(Value::Boolean(false))),
        ("earlierFull", Some(attempt(&bare, &node, "Full", 0))),
        ("laterFull", Some(attempt(&bare, &node, "Full", 1))),
    ];
    let (slot, supply) = call(&bare, &node, "retry.relation", values);
    let mut meter = Meter::new(UNLIMITED);
    let result = evaluate(&bare, &slot, &supply, &mut meter).expect("call");
    let charges = &result.charges;
    assert_eq!(
        charges.accounting_version(),
        fsl_eval::value::BINDER_ACCOUNTING_VERSION
    );
    let selected = bare
        .select_protocol_slot(&ProtocolSlotDescriptor::new(
            node.clone(),
            "retry.relation",
            None,
        ))
        .expect("selects");
    let work = |name: &str| {
        let source = selected
            .sources()
            .iter()
            .find(|source| source.name() == name)
            .expect("binder");
        charges
            .binder(source.binder())
            .map(|amounts| amounts.amount(LimitKind::WorkUnits))
    };
    for read in ["targetFull", "activatedFull", "earlierFull", "laterFull"] {
        assert!(work(read).is_some(), "{read} has an entry");
    }
    assert!(work("earlierFull").is_some_and(|units| units > 0));
    for kind in LimitKind::ALL {
        assert_eq!(
            charges.total().amount(kind),
            meter.consumed(kind),
            "entries and the unattributed entry equal the call's total for {kind:?}"
        );
    }
}

#[derive(Debug)]
struct Unsupported;

impl filament_semantic_value::catalog::CatalogCoded for Unsupported {
    fn catalog_code(&self) -> filament_semantic_value::catalog::CatalogCode {
        filament_semantic_value::catalog::CatalogCode::new("unsupported_construct", "unsupported")
    }

    fn catalog_fields(&self) -> Option<std::collections::BTreeMap<&'static str, String>> {
        None
    }
}

/// FR-045-AC-1 and AC-3: each owner outcome maps to its own row and none to
/// a Boolean. A non-Boolean completion, an undefined, a refusal, an
/// incomplete and a family result are each carried with the owner's value;
/// no admitted input of R reaches them through a slot (R's slots complete
/// with a Boolean or stop on the meter), so the mapping is called directly.
#[test]
#[trace("FR-045-AC-1", "FR-045-AC-3", "TC-151")]
fn each_owner_outcome_maps_to_its_row_and_none_to_a_boolean() {
    use filament_eval::{FamilyOutcome, FamilyResult};
    use filament_exact::{Incomplete, InexactTarget, IntegerInterval, Outcome, Refusal, Undefined};
    use filament_protocol::slot::outcome_of;

    let evaluated = |outcome| FamilyOutcome::Evaluated(outcome);
    assert!(matches!(
        outcome_of(evaluated(Outcome::Completed(Value::Boolean(true)))),
        SlotOutcome::Holds
    ));
    assert!(matches!(
        outcome_of(evaluated(Outcome::Completed(Value::Boolean(false)))),
        SlotOutcome::DoesNotHold
    ));
    let SlotOutcome::NotBoolean(Value::Integer(seven)) =
        outcome_of(evaluated(Outcome::Completed(int(7))))
    else {
        panic!("a non-Boolean completion is NotBoolean");
    };
    assert_eq!(seven, 7_i64.into());
    let SlotOutcome::Undefined(undefined) =
        outcome_of(evaluated(Outcome::Undefined(Undefined::DivisionByZero)))
    else {
        panic!("an undefined result is Undefined");
    };
    assert_eq!(undefined, Undefined::DivisionByZero);
    let interval = IntegerInterval::new(0_i64.into(), 3_i64.into()).expect("interval");
    let refusal = Refusal::InexactDecimal {
        target: InexactTarget::Integer(Box::new(interval)),
    };
    let SlotOutcome::Refused(held) = outcome_of(evaluated(Outcome::Refused(refusal.clone())))
    else {
        panic!("a refusal is Refused");
    };
    assert_eq!(held, refusal);
    let stopped = Incomplete {
        limit_kind: LimitKind::WorkUnits,
        limit: 4,
        consumed: 4,
        next_charge: 1_i64.into(),
        charge_point: filament_exact::ChargePoint::CollectionElement,
    };
    let SlotOutcome::Incomplete(held) = outcome_of(evaluated(Outcome::Incomplete(stopped.clone())))
    else {
        panic!("an incomplete is Incomplete");
    };
    assert_eq!(held, stopped);
    let SlotOutcome::Family(FamilyResult::Refused(family)) = outcome_of(
        FamilyOutcome::FamilyEvaluated(FamilyResult::Refused(Box::new(Unsupported))),
    ) else {
        panic!("a family result is Family");
    };
    assert_eq!(format!("{family:?}"), "Unsupported");
}

/// FR-045-AC-4: a node that authors no bound reads as no bound, a node that
/// is no application is the typed refusal naming it, and the authored
/// maximum is read exactly. The reader refuses a zero `maximum` and an
/// `attempts` argument with no `maximum`, so no admitted package reaches
/// those two refusals.
#[test]
#[trace("FR-045-AC-4", "TC-151")]
fn a_bound_that_is_absent_or_malformed_is_read_exactly() {
    use filament_contract_model::{read_checked_package, CheckedPackageDispatchResult};
    use filament_protocol::slot::BoundRefusal;

    let admitted = handoff();
    let nodes = &admitted.package().graph().nodes;
    let role = nodes
        .iter()
        .find(|node| &*node.node_tag == "protocol" && &*node.semantic_form == "role")
        .expect("a role node");
    assert_eq!(attempts_maximum(role), Ok(None), "no authored bound");
    let model = nodes
        .iter()
        .find(|node| &*node.node_tag == "model")
        .expect("a model node");
    assert_eq!(
        attempts_maximum(model),
        Err(BoundRefusal {
            node: model.node_id.clone()
        }),
        "a node that is no application"
    );

    let written = written("event_position", Corpus::None);
    let edited = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut json: serde_json::Value =
            serde_json::from_slice(&written.files["checked-package.json"]).expect("JSON");
        for node in json["semantic_graph"]["nodes"]
            .as_array_mut()
            .expect("nodes")
        {
            if node["semantic_form"] == "compensation" {
                let arguments = node["body"]["arguments"].as_array_mut().expect("arguments");
                let attempts = arguments
                    .iter_mut()
                    .find(|argument| argument["name"] == "attempts")
                    .expect("attempts");
                edit(&mut attempts["value"]);
            }
        }
        let bytes = serde_json::to_vec(&json).expect("bytes");
        match read_checked_package(
            &bytes,
            CheckedPackageReadLimits::bounded(),
            &written.evidence,
        ) {
            CheckedPackageDispatchResult::AdmittedV2(read) => Some(read),
            CheckedPackageDispatchResult::Refused(_) => None,
            other => panic!("neither admitted nor refused: {other:?}"),
        }
    };
    let bounds = |package: &filament_contract_model::CheckedPackageV2| {
        package
            .graph()
            .nodes
            .iter()
            .filter(|node| &*node.semantic_form == "compensation")
            .map(|node| attempts_maximum(node).map_err(|refusal| refusal.node == node.node_id))
            .collect::<Vec<_>>()
    };
    let zero = edited(&|value| {
        for member in value["members"].as_array_mut().expect("members") {
            if member["name"] == "maximum" {
                member["value"]["value"] = serde_json::Value::from("0");
            }
        }
    });
    assert!(zero.is_none(), "the reader refuses a zero maximum");
    let absent = edited(&|value| {
        value["members"]
            .as_array_mut()
            .expect("members")
            .retain(|member| member["name"] != "maximum");
    });
    assert!(
        absent.is_none(),
        "the reader refuses an attempts argument with no maximum"
    );
    let seven = edited(&|value| {
        for member in value["members"].as_array_mut().expect("members") {
            if member["name"] == "maximum" {
                member["value"]["value"] = serde_json::Value::from("7");
            }
        }
    });
    assert_eq!(
        bounds(&seven.expect("seven admitted")),
        [Ok(NonZeroU32::new(7)), Ok(NonZeroU32::new(7))],
        "the authored seven is read"
    );
}

/// FR-045-AC-4: the retry behavior of FR-022 that `tc_003` asserts on the
/// retired report, restated on the outcome map. The `retry.relation` slot and
/// the ledger's bound are checked separately: `AttemptLedger::admit` never
/// consults the relation. The relation holds between consecutive attempts, does
/// not hold for a skipped tally, and is Unavailable (never a Boolean) when a
/// binder is missing; the bound read from `attempts maximum` ends the
/// admissions.
#[test]
#[trace("FR-045-AC-4", "TC-151")]
fn the_retry_and_bound_behaviors_hold_on_the_outcome_map() {
    let admitted = handoff();
    let bare = bare(&admitted);
    let node = compensation_key(&admitted, "Full");
    let bound = admitted
        .package()
        .graph()
        .nodes
        .iter()
        .find(|node| {
            &*node.semantic_form == "compensation"
                && node.declaration.as_ref().is_some_and(|declaration| {
                    declaration.qualified_name.last().map(|part| &**part) == Some("Full")
                })
        })
        .map(|node| attempts_maximum(node).expect("authored bound"))
        .and_then(|found| found)
        .expect("the compensation Full authors a bound");
    assert_eq!(bound.get(), 3);

    let relation = |earlier: i64, later: i64, supplied: bool| {
        let values = vec![
            ("targetFull", Some(total(&bare, &node, "Full", 5))),
            ("activatedFull", Some(Value::Boolean(false))),
            ("earlierFull", Some(attempt(&bare, &node, "Full", earlier))),
            (
                "laterFull",
                supplied.then(|| attempt(&bare, &node, "Full", later)),
            ),
        ];
        let (slot, supply) = call(&bare, &node, "retry.relation", values);
        evaluate(&bare, &slot, &supply, &mut Meter::new(UNLIMITED)).expect("evaluates")
    };

    let forward = Definition {
        identity: DefinitionId::new(b"definition:full", "full"),
        paired_forward_effect: EffectId::new(b"effect:forward", "forward"),
        retry_bound: Some(bound),
    };
    let registration = Registry::default()
        .observe(
            PureAssessment::checked(&NoCapabilities::default()).expect("pure"),
            &[forward],
            &ForwardFact::EstablishedEffect(EffectId::new(b"effect:forward", "forward")),
        )
        .expect("registers")[0]
        .registration
        .clone();
    let mut ledger =
        AttemptLedger::new(registration.clone(), AlgebraLimits::owner_max()).expect("ledger");
    let fact = |number: i64| AttemptFact {
        attempt: AttemptId::new(format!("attempt:{number}").as_bytes(), "attempt"),
        registration: registration.identity.clone(),
        established_effect: None,
        outcome: Some(AttemptOutcome::Failed),
    };
    let pure = || PureAssessment::checked(&NoCapabilities::default()).expect("pure");

    // The first attempt needs no relation; each retry needs it to hold.
    assert!(matches!(
        ledger.admit(pure(), fact(0)),
        AttemptAdmission::Admitted { .. }
    ));
    for (earlier, later) in [(0, 1), (1, 2)] {
        assert!(holds(&relation(earlier, later, true)), "{earlier}->{later}");
        assert!(matches!(
            ledger.admit(pure(), fact(later)),
            AttemptAdmission::Admitted { .. }
        ));
    }
    // Past the bound the relation may still hold; the bound refuses it.
    assert!(holds(&relation(2, 3, true)));
    assert!(matches!(
        ledger.admit(pure(), fact(3)),
        AttemptAdmission::RetryBoundExceeded { .. }
    ));
    assert_eq!(ledger.attempts().len(), 3);

    // A relation that does not hold, or whose binder is unavailable, is not
    // a Boolean and admits nothing.
    assert!(!holds(&relation(0, 2, true)), "skipping a tally");
    let missing = relation(0, 1, false);
    assert!(
        matches!(missing.outcome, SlotOutcome::Unavailable { .. }),
        "{:?}",
        missing.outcome
    );
}

/// FR-045-AC-5: the compensation assessment performs no I/O. The assessment
/// runs here (the slot calls and the ledger above and in the other tests of
/// this file complete in a plain process), and the module that holds the
/// compensation algebra, the recovery decision and the slot mapping names no
/// network, storage, process, environment, clock-sleep or telemetry path, and
/// the crate declares no I/O dependency.
#[test]
#[trace("FR-045-AC-5", "TC-151")]
fn the_compensation_modules_name_no_io_path_or_crate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    sources(&root.join("src").join("compensation"), &mut files);
    files.push((
        "src/slot.rs".to_owned(),
        std::fs::read_to_string(root.join("src").join("slot.rs")).expect("slot source"),
    ));
    assert!(
        files.len() >= 6,
        "the compensation modules and the slot mapping"
    );
    for (path, text) in &files {
        for io in [
            "std::net",
            "std::fs",
            "std::process",
            "std::env",
            "std::io",
            "std::thread",
            "tokio",
            "reqwest",
            "hyper",
            "ureq",
            "tracing",
            "opentelemetry",
            "sqlx",
            "rusqlite",
        ] {
            let named = text
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .any(|line| line.contains(io));
            assert!(!named, "{path} names {io}");
            for imported in use_paths(text) {
                let hit = imported == io || imported.starts_with(&format!("{io}::"));
                assert!(!hit, "{path} imports {imported}");
            }
        }
    }
    // The crate's own normal dependencies (cargo metadata, offline) include no
    // I/O crate; third-party transitive crates are out of this control's scope.
    let output = std::process::Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--offline", "--locked"])
        .current_dir(root)
        .output()
        .expect("cargo metadata runs");
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).expect("metadata");
    let names: std::collections::BTreeMap<&str, &str> = metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .map(|package| {
            (
                package["id"].as_str().expect("id"),
                package["name"].as_str().expect("name"),
            )
        })
        .collect();
    let nodes: std::collections::BTreeMap<&str, &serde_json::Value> = metadata["resolve"]["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| (node["id"].as_str().expect("id"), node))
        .collect();
    let root_id = metadata["resolve"]["root"].as_str().expect("root");
    let seen: Vec<&str> = nodes[root_id]["deps"]
        .as_array()
        .expect("deps")
        .iter()
        .filter(|dep| {
            dep["dep_kinds"]
                .as_array()
                .expect("kinds")
                .iter()
                .any(|kind| kind["kind"].is_null())
        })
        .map(|dep| dep["pkg"].as_str().expect("pkg"))
        .collect();
    assert!(seen.len() > 5, "the normal dependencies were read");
    for id in seen {
        let name = names[id];
        for io in [
            "tokio",
            "reqwest",
            "hyper",
            "ureq",
            "tracing",
            "opentelemetry",
            "sqlx",
            "rusqlite",
        ] {
            assert_ne!(name, io, "the normal dependencies include {io}");
        }
    }
}

/// Every path a `use` item of `text` imports, with brace groups expanded and
/// aliases dropped: `use std::{fs::File, io::Write};` yields `std::fs::File`
/// and `std::io::Write`.
fn use_paths(text: &str) -> Vec<String> {
    let code: String = text
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ");
    let mut paths = Vec::new();
    for statement in code.split(';') {
        let statement = statement.trim();
        let statement = statement.strip_prefix("pub ").unwrap_or(statement);
        let statement = statement
            .find("use ")
            .filter(|at| statement[..*at].trim().is_empty() || statement[..*at].ends_with(')'))
            .map(|at| &statement[at + 4..]);
        if let Some(tree) = statement {
            let flat: String = tree.split_whitespace().collect::<Vec<_>>().join(" ");
            paths.extend(expand_use_tree(&flat));
        }
    }
    paths
}

fn expand_use_tree(tree: &str) -> Vec<String> {
    let tree = tree.trim();
    let Some(open) = tree.find('{') else {
        let path = tree.split(" as ").next().unwrap_or(tree);
        return vec![path.replace(' ', "").trim_start_matches("::").to_owned()];
    };
    let prefix = tree[..open].replace(' ', "");
    let close = tree.rfind('}').expect("balanced braces");
    let (mut depth, mut start, mut items) = (0usize, open + 1, Vec::new());
    for (at, ch) in tree[open + 1..close].char_indices() {
        let at = at + open + 1;
        match ch {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                items.push(&tree[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    items.push(&tree[start..close]);
    items
        .into_iter()
        .filter(|item| !item.trim().is_empty())
        .flat_map(|item| {
            let prefix = prefix.clone();
            expand_use_tree(item).into_iter().map(move |rest| {
                let joined = if rest == "self" {
                    prefix.trim_end_matches("::").to_owned()
                } else {
                    format!("{prefix}{rest}")
                };
                joined.trim_start_matches("::").to_owned()
            })
        })
        .collect()
}

/// FR-045-AC-7: with the per-binder charge log of AC-8 in place, each
/// assessment reports attribution as supported: the log is FSL's, carried
/// whole on a completed and on an unavailable result, the unavailable binder
/// has its own entry, and a binder the slot does not read has none (no zero
/// entry is substituted for it).
#[test]
#[trace("FR-045-AC-7", "TC-151")]
fn attribution_is_supported_and_no_zero_entry_stands_in_for_a_missing_binder() {
    let admitted = handoff();
    let bare = bare(&admitted);
    let node = compensation_key(&admitted, "Full");
    let selected = |member: &str| {
        bare.select_protocol_slot(&ProtocolSlotDescriptor::new(node.clone(), member, None))
            .expect("selects")
    };
    let retry_slot = selected("retry.relation");
    let guard_slot = selected("trigger.guard");
    let binder = |slot: &fsl_package::BareSlot, name: &str| {
        slot.sources()
            .iter()
            .find(|source| source.name() == name)
            .expect("binder")
            .binder()
            .clone()
    };

    // One unavailable binder among four.
    let values = vec![
        ("targetFull", None),
        ("activatedFull", Some(Value::Boolean(false))),
        ("earlierFull", Some(attempt(&bare, &node, "Full", 0))),
        ("laterFull", Some(attempt(&bare, &node, "Full", 1))),
    ];
    let (slot, supply) = call(&bare, &node, "retry.relation", values);
    let mut meter = Meter::new(UNLIMITED);
    let result = evaluate(&bare, &slot, &supply, &mut meter).expect("call");
    assert!(matches!(result.outcome, SlotOutcome::Unavailable { .. }));
    let unavailable = binder(&retry_slot, "targetFull");
    assert!(
        result.charges.binder(&unavailable).is_some(),
        "the unavailable binder has its own entry (its work is not charged: it is never read)"
    );
    assert_eq!(
        result.charges.accounting_version(),
        fsl_eval::value::BINDER_ACCOUNTING_VERSION
    );
    for kind in LimitKind::ALL {
        assert_eq!(
            result.charges.total().amount(kind),
            meter.consumed(kind),
            "{kind:?}"
        );
    }
    // A binder of another slot is not read here and has no entry.
    let foreign = binder(&guard_slot, "triggerFull");
    assert!(result.charges.binder(&foreign).is_none());

    // The same log is carried whole on a completed result.
    let values = vec![
        ("targetFull", Some(total(&bare, &node, "Full", 5))),
        ("activatedFull", Some(Value::Boolean(false))),
        ("earlierFull", Some(attempt(&bare, &node, "Full", 0))),
        ("laterFull", Some(attempt(&bare, &node, "Full", 1))),
    ];
    let (slot, supply) = call(&bare, &node, "retry.relation", values);
    let mut meter = Meter::new(UNLIMITED);
    let done = evaluate(&bare, &slot, &supply, &mut meter).expect("call");
    assert!(holds(&done));
    for name in ["targetFull", "activatedFull", "earlierFull", "laterFull"] {
        assert!(
            done.charges.binder(&binder(&retry_slot, name)).is_some(),
            "{name} has an entry on a completed result"
        );
    }
    for kind in LimitKind::ALL {
        assert_eq!(done.charges.total().amount(kind), meter.consumed(kind));
    }
    assert!(done.charges.binder(&foreign).is_none());
}

/// The import scan expands brace groups, so a grouped import cannot hide an
/// I/O path from the FR-045-AC-5 scan.
#[test]
#[trace("FR-045-AC-5", "TC-151")]
fn the_import_scan_expands_brace_groups() {
    let paths = use_paths("use std::{fs::File, io::Write};\nuse std::{self, env as e};\n");
    assert_eq!(
        paths,
        ["std::fs::File", "std::io::Write", "std", "std::env"]
    );
}

// ---------------------------------------------------------------------------
// The recovery slot (FR-045-AC-1, AC-2, AC-3)
// ---------------------------------------------------------------------------

/// The unit of the quantity binder `binder` of the slot `member`.
fn unit_of(
    bare: &BarePackage<'_>,
    node: &NodeKey,
    member: &str,
    binder: &str,
) -> filament_exact::UnitId {
    let selected = bare
        .select_protocol_slot(&ProtocolSlotDescriptor::new(node.clone(), member, None))
        .expect("slot selects");
    let source = selected
        .sources()
        .iter()
        .find(|source| source.name() == binder)
        .expect("binder");
    let ValueType::Quantity(unit) = source.value_type() else {
        panic!("{binder} is a quantity");
    };
    unit.clone()
}

fn quantity(unit: &filament_exact::UnitId, amount: i64) -> Value {
    Value::Quantity(Quantity::new(
        Rational::from_integer(Integer::from(amount)),
        unit.clone(),
    ))
}

/// A `Recovery` record whose `receipts` are the quantities `amounts`.
fn recovery(bare: &BarePackage<'_>, node: &NodeKey, name: &str, amounts: &[i64]) -> Value {
    let member = "recovery.predicate";
    let unit = unit_of(bare, node, member, &format!("target{name}"));
    let key = record_key(bare, node, member, &format!("recovered{name}"));
    let receipts = from_admitted(
        CollectionType::new(
            CollectionKind::Sequence,
            ValueType::Quantity(unit.clone()),
            Some(CardinalityBound::at_most(5)),
        ),
        amounts
            .iter()
            .map(|amount| quantity(&unit, *amount))
            .collect(),
    );
    bare.world()
        .expect("world")
        .types()
        .record(key, vec![("receipts", FieldValue::Present(receipts))])
        .expect("Recovery")
}

/// The recovery relation of `name` over a target, an activation capture and the
/// achieved receipts; `None` supplies the binder as unavailable.
fn recovers(
    admitted: &AdmittedHandoff,
    name: &str,
    target: Option<i64>,
    activated: Option<bool>,
    receipts: Option<&[i64]>,
) -> SlotEvaluation {
    let bare = bare(admitted);
    let node = compensation_key(admitted, name);
    let unit = unit_of(&bare, &node, "recovery.predicate", &format!("target{name}"));
    let values = [
        (
            format!("target{name}"),
            target.map(|amount| quantity(&unit, amount)),
        ),
        (format!("activated{name}"), activated.map(Value::Boolean)),
        (
            format!("recovered{name}"),
            receipts.map(|amounts| recovery(&bare, &node, name, amounts)),
        ),
    ];
    let values = values
        .iter()
        .map(|(n, v)| (n.as_str(), v.clone()))
        .collect();
    let (slot, supply) = call(&bare, &node, "recovery.predicate", values);
    evaluate(&bare, &slot, &supply, &mut Meter::new(UNLIMITED)).expect("recovery evaluates")
}

/// FR-045-AC-1, the recovery-predicate rows: `Full` holds for receipts that sum
/// to the captured target and not otherwise; `Partial` holds for a positive sum
/// below it. The slot is the one FSL selects (`recovery.predicate`, the `sum`
/// query bound inside it), evaluated through the same call as every other row.
#[test]
#[trace("FR-045-AC-1", "TC-151")]
fn the_recovery_predicate_rows_have_their_successor_outcomes() {
    let admitted = handoff();
    let full = |receipts: &[i64], activated| {
        holds(&recovers(
            &admitted,
            "Full",
            Some(5),
            Some(activated),
            Some(receipts),
        ))
    };
    assert!(full(&[2, 3], false), "receipts summing to the target");
    assert!(!full(&[2, 2], false), "receipts short of the target");
    assert!(!full(&[2, 3, 1], false), "receipts above the target");
    assert!(!full(&[2, 3], true), "an activated capture withdraws it");
    let partial = |receipts: &[i64]| {
        holds(&recovers(
            &admitted,
            "Partial",
            Some(5),
            Some(false),
            Some(receipts),
        ))
    };
    assert!(partial(&[2]), "a positive sum below the target");
    assert!(partial(&[1, 3]), "the sum, not a receipt, is compared");
    assert!(!partial(&[]), "no receipts is not positive");
    assert!(!partial(&[5]), "a sum equal to the target is not below it");
}

/// FR-045-AC-2, the recovery assertions of `tc_003` (FR-024-AC-1 to AC-6)
/// restated on the outcome map.
#[test]
#[trace("FR-045-AC-2", "TC-151")]
fn the_recovery_behaviors_of_tc_003_hold_on_the_outcome_map() {
    let admitted = handoff();
    let outcome = |name: &str, target, receipts: &[i64]| {
        holds(&recovers(
            &admitted,
            name,
            Some(target),
            Some(false),
            Some(receipts),
        ))
    };
    // FR-024-AC-1: one achieved state satisfies the partial relation and fails
    // the full one over the same captured target, and neither ranks above.
    assert!(outcome("Partial", 5, &[2]));
    assert!(!outcome("Full", 5, &[2]));
    assert!(outcome("Full", 5, &[2, 3]));
    assert!(!outcome("Partial", 5, &[2, 3]));
    // FR-024-AC-2: the exact relation, target and achieved state; varying any
    // one withdraws the success.
    assert!(outcome("Full", 5, &[2, 3]));
    assert!(!outcome("Full", 6, &[2, 3]), "another target");
    assert!(!outcome("Full", 5, &[2, 2]), "another achieved state");
    assert!(!outcome("Partial", 5, &[2, 3]), "another relation");
    // FR-024-AC-3: the aggregate is exact arithmetic over the whole population.
    assert!(outcome("Full", 12, &[5, 4, 3]));
    assert!(!outcome("Full", 12, &[5, 4, 2]));
    // FR-024-AC-4: a missing target or achieved state is Unavailable, never a
    // recovery success.
    let missing_target = recovers(&admitted, "Full", None, Some(false), Some(&[2, 3]));
    let missing_achieved = recovers(&admitted, "Full", Some(5), Some(false), None);
    for found in [&missing_target, &missing_achieved] {
        assert!(
            matches!(found.outcome, SlotOutcome::Unavailable { .. }),
            "{:?}",
            found.outcome
        );
    }
    // FR-024-AC-5 and AC-6: the slot reads its three binders and no attempt
    // outcome or callback, and nothing converts an outcome to a Boolean.
    let bare = bare(&admitted);
    let node = compensation_key(&admitted, "Full");
    let selected = bare
        .select_protocol_slot(&ProtocolSlotDescriptor::new(
            node,
            "recovery.predicate",
            None,
        ))
        .expect("slot selects");
    let mut names: Vec<_> = selected
        .sources()
        .iter()
        .map(|s| s.name().to_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["activatedFull", "recoveredFull", "targetFull"]);
}

/// FR-045-AC-3, the recovery supplies: an incomplete population, a missing
/// target state, a missing achieved state or an unavailable binder never
/// yields a true recovery.
#[test]
#[trace("FR-045-AC-3", "TC-151")]
fn a_recovery_supply_that_is_not_complete_never_yields_a_true_recovery() {
    let admitted = handoff();
    for (target, activated, receipts) in [
        (None, Some(false), Some(&[2_i64, 3][..])),
        (Some(5), None, Some(&[2, 3][..])),
        (Some(5), Some(false), None),
    ] {
        let found = recovers(&admitted, "Full", target, activated, receipts);
        assert!(
            matches!(found.outcome, SlotOutcome::Unavailable { .. }),
            "{target:?} {activated:?} {receipts:?}: {:?}",
            found.outcome
        );
    }
    // An incomplete population: a `Recovery` without its required `receipts`
    // cannot be built, so no such supply reaches the slot.
    let bare = bare(&admitted);
    let node = compensation_key(&admitted, "Full");
    let key = record_key(&bare, &node, "recovery.predicate", "recoveredFull");
    let absent = bare
        .world()
        .expect("world")
        .types()
        .record(key, vec![("receipts", FieldValue::Absent)]);
    assert!(absent.is_err());
}
