// SPDX-License-Identifier: AGPL-3.0-or-later
//! A realizability fixture whose every package comes from a handoff directory
//! written by FSL's writer and compiled back by `compile` (FSL FR-517, FR-571):
//! a three-role protocol over the recipe model, the admitted package the
//! request names, the snapshot built from the handoff's domain documents, and
//! the subject FSL's `protocol_subject` builds. No front end is driven here.

// Each test binary uses a different part of these helpers.
#![allow(dead_code)]

use std::collections::BTreeSet;

use filament_checked::key::DeclarationKey;
use filament_contract_model::{
    CheckedNodeId, CheckedPackageDispatchResult, CheckedPackageReadLimits, CheckedPackageV2,
};
use filament_exact::ScalarLimits;
use filament_protocol::engine::FiniteReplayRequest;
use filament_spec_language::checked_handoff::{compile, CompiledHandoff, Corpus};
use fsl_analyze::analyze::{
    AnalyzeLimits, AnalyzeRequest, ClaimItem, ItemOptions, NegotiatedItem, SourceProvision,
    SubjectBinding,
};
use fsl_analyze::protocol_analysis::{
    protocol_subject, RealizabilityRequest, SubjectBindings, SubjectPackage,
};
use fsl_eval::model_subject::{ModelInstance, ProtocolSubject, RoleBinding};
use fsl_eval::simulation::model::ModelSystem;
use fsl_eval::simulation::model::{ExpansionLimits, PopulationUniverse};
use fsl_eval::simulation::protocol::{
    send_record_json, transition_json, ProtocolSystem, ProtocolTransition, Sc,
};
use fsl_eval::simulation::{SubjectSystem, TransitionSystem};
use fsl_foundation::source::provenance::OccurrenceKey;
use fsl_foundation::RequestIndex;
use fsl_replay::protocol_analysis::{BoundsDraft, RunBudget};
use fsl_route::request::ExtentClassification;
use fsl_route::routing::Disposition;
use fsl_semantics::check::Capability;
use fsl_semantics::model::accounting::ModelNormalizationLimits;
use fsl_semantics::model::observation::{
    document_reference, DocumentProvision, DocumentRef, ObservationLimits,
};
use serde_json::{json, Value as Json};

use filament_protocol::link::{link, LinkedProtocol};

use super::handoff::{request, written_request, Handoff};
use super::projection::{assessed_root, MEMBERS, RECORDS};

const PACKAGE: &str = "agent-ix/checked-handoff";

const UNLIMITED: ScalarLimits = ScalarLimits {
    integer_bits: 1 << 20,
    decimal_digits: 1 << 20,
    scale_expansion: 1 << 20,
    text_input_bytes: 1 << 20,
    text_scalars: 1 << 20,
    normalized_scalars: 1 << 20,
    unit_edges: 1 << 20,
    value_occurrences: 1 << 20,
    work_units: 1 << 30,
    result_units: 1 << 30,
};

fn ident(suffix: &str) -> String {
    format!("ix://{PACKAGE}/{suffix}")
}

fn key(suffix: &str) -> DeclarationKey {
    DeclarationKey {
        package: PACKAGE.to_owned(),
        node: ident(suffix),
    }
}

/// The recipe model with the closed populations the subject simulates: one
/// per role's object type.
fn model(bounded: bool) -> Vec<u8> {
    let mut model: Json = serde_json::from_slice(&super::dynamic::model()).expect("model JSON");
    // A deliberate fixture-side substitute for the bounded-domain tests
    // only, not a workaround for a defect. A quantity is not a finite value
    // domain (FSL FR-120 lists the finite domains: Boolean, bounded integers,
    // Option, Reference, bounded sequences, enums, model-variant unions,
    // records, tuples, sets and bags), so FSL's EN-1 INTENDEDLY refuses an
    // unbounded quantity root as requiring a bound. The tests that need EN-1
    // to decide read `Amount` and `Total` here as the bounded integers their
    // own `min`/`max` constraints state. This rewrites FSL's model in memory;
    // no file of FSL's is copied. The unrewritten model is
    // [`Realizability::unbounded`]: TC-205's FR-205-AC-5 dispatches it to the
    // real checker with no rewrite and asserts the requires-bound refusal is
    // carried unchanged. That test, not this rewrite, is V1-PROT-014's
    // evidence.
    for declared in model["types"].as_array_mut().expect("types") {
        let is_unit = declared["target"]
            .as_str()
            .is_some_and(|target| target.starts_with("ix://filament/unit/"));
        if is_unit && bounded {
            let object = declared.as_object_mut().expect("a type");
            object.remove("target");
            object.insert(
                "kind".to_owned(),
                json!({"module": PACKAGE, "name": "value_type"}),
            );
            object.insert("scalar".to_owned(), json!("integer"));
            for constraint in object["constraints"].as_array_mut().expect("constraints") {
                constraint["appliesTo"] = json!("ix://filament/native/Integer");
            }
        }
    }
    let populations = model["populations"].as_array_mut().expect("populations");
    for (name, member) in [("workflows", "Workflow"), ("depots", "Depot")] {
        populations.push(json!({
            "identity": ident(name), "displayName": ident(name),
            "kind": {"module": PACKAGE, "name": "population"},
            "members": [ident(member)], "extent": "closed",
            "origin": {"generated": {"generatorIdentity": ident(name),
                "generatorVersion": "1.0.0", "inputIdentities": [ident(name)]}},
        }));
    }
    serde_json::to_vec(&model).expect("model bytes")
}

/// The value of `field` in an initial object keyed `object`, read from the
/// model package `document`: the type's own lower bound, so it is admissible
/// whatever bounds the model declares.
fn initial_value(document: &Json, field: &Json, object: &str) -> Json {
    let type_ref = field["typeRef"].as_str().expect("a type reference");
    let scalar = document["types"]
        .as_array()
        .expect("types")
        .iter()
        .find(|found| found["identity"] == type_ref);
    let unit = scalar
        .and_then(|found| found["target"].as_str())
        .and_then(|target| target.strip_prefix("ix://filament/unit/"));
    let one = match scalar.and_then(|found| found["scalar"].as_str()) {
        // A quantity, at its own lower bound: the unit and an exact magnitude.
        None if unit.is_some() => {
            let minimum = scalar.expect("a type")["constraints"]
                .as_array()
                .and_then(|all| all.iter().find(|found| found["keyword"] == "min"))
                .map_or("0", |found| {
                    found["operands"]["value"].as_str().expect("a bound")
                });
            json!({"kind": "quantity", "unit": unit,
                "magnitude": {"kind": "rational", "numerator": minimum, "denominator": "1"}})
        }
        None => json!({"boolean": false}),
        Some("integer") => {
            let minimum = scalar.expect("a type")["constraints"]
                .as_array()
                .and_then(|all| all.iter().find(|found| found["keyword"] == "min"))
                .map_or("0", |found| {
                    found["operands"]["value"].as_str().expect("a bound")
                });
            json!({"integer": minimum})
        }
        Some("string") => json!({"kind": "text", "text": object}),
        Some(other) => panic!("no initial value for scalar {other}"),
    };
    if field["multiplicity"]["upper"].as_u64() == Some(1) {
        one
    } else {
        json!({"sequence": []})
    }
}

/// The initialization snapshot of the model package `document`: one object
/// keyed `w` in `workflows`, and every other population empty.
fn snapshot(document: &Json) -> Vec<u8> {
    let populations: Vec<Json> = document["populations"]
        .as_array()
        .expect("populations")
        .iter()
        .map(|population| {
            let objects: Vec<Json> = if population["identity"] == ident("workflows") {
                let member = population["members"][0].as_str().expect("one member type");
                let object_type = document["types"]
                    .as_array()
                    .expect("types")
                    .iter()
                    .find(|found| found["identity"] == member)
                    .expect("the member type");
                let fields: serde_json::Map<String, Json> = object_type["fields"]
                    .as_array()
                    .expect("fields")
                    .iter()
                    .map(|field| {
                        (
                            field["name"].as_str().expect("a field name").to_owned(),
                            initial_value(document, field, "w"),
                        )
                    })
                    .collect();
                vec![json!({"key": "w", "type": member, "fields": fields})]
            } else {
                Vec::new()
            };
            json!({"population": population["identity"], "complete": true, "objects": objects})
        })
        .collect();
    json!({
        "format": "filament.state.snapshot/v1",
        "identity": {"authority": "test", "identity": "s0"},
        "observation": "current", "model": {"identity": PACKAGE},
        "anchor": {"kind": "initialization", "name": "start"},
        "populations": populations,
    })
    .to_string()
    .into_bytes()
}

pub fn channel(name: &str, from: &str, to: &str, carries: &str) -> String {
    format!(
        "channel {name} from {from} to {to} carries {carries} ordering unordered delivery \
         exactly-once-premise capacity 4 overflow reject;\n"
    )
}

/// One fixture: the handoff FSL's writer wrote, compiled back in process, and
/// the snapshot built from its domain package documents. The subject comes
/// from nothing else.
pub struct Realizability {
    handoff: Handoff,
    compiled: CompiledHandoff,
    snapshots: DocumentProvision,
    initial: Vec<DocumentRef>,
    clause: CheckedNodeId,
    linked: LinkedProtocol,
    /// The protocol's declared name.
    protocol: String,
    /// The realizability request's depth bound.
    depth: u64,
    /// The text of the unit the handoff was written from, which a
    /// `finite-replay` request provisions under the package's source label.
    unit: String,
}

impl Realizability {
    /// `Buyer` places an order; `Seller` reads it and owns `ship | cancel`.
    pub fn worked(ship: &str, cancel: &str) -> Self {
        Self::worked_with("", ship, cancel)
    }

    /// `worked`, with `extra` member declarations (channels) added.
    pub fn worked_with(extra: &str, ship: &str, cancel: &str) -> Self {
        Self::worked_as("Flow", extra, ship, cancel)
    }

    /// `worked_with`, over a protocol declared as `protocol`, so that its
    /// clause node differs from another component's.
    pub fn worked_as(protocol: &str, extra: &str, ship: &str, cancel: &str) -> Self {
        Self::delivering_as(protocol, "exactly-once-premise", "", extra, ship, cancel)
    }

    /// The same fixture with the realizability depth bound set to `depth`.
    pub fn with_depth(mut self, depth: u64) -> Self {
        self.depth = depth;
        self
    }

    /// `worked`, over channels of the `delivery` mode, with the `Warehouse`
    /// declaration `warehouse` in place of its static one when it is not empty.
    pub fn delivering(
        delivery: &str,
        warehouse: &str,
        extra: &str,
        ship: &str,
        cancel: &str,
    ) -> Self {
        Self::delivering_as("Flow", delivery, warehouse, extra, ship, cancel)
    }

    /// `delivering`, over a protocol declared as `protocol`.
    pub fn delivering_as(
        protocol: &str,
        delivery: &str,
        warehouse: &str,
        extra: &str,
        ship: &str,
        cancel: &str,
    ) -> Self {
        Self::build(protocol, delivery, warehouse, extra, ship, cancel, "", true)
    }

    /// `worked`, over the recipe model as FSL ships it: `Amount` and `Total`
    /// stay the unbounded quantities FSL FR-120 says they are, with no
    /// in-memory bounding. FSL's explicit-state checker refuses such a root as
    /// requiring a bound (V1-PROT-014, FR-205-AC-5).
    pub fn unbounded() -> Self {
        Self::build(
            "Flow",
            "exactly-once-premise",
            "",
            "",
            super::projection::PICK,
            super::projection::CANCELLED,
            "",
            false,
        )
    }

    /// `worked_as`, with the temporal clause `Due` (a required clause of the
    /// infinite-trace profile, with the fairness row it needs) declared before
    /// the protocol and required by it after its channels (FSL FR-383).
    pub fn requiring(protocol: &str, clause: &str) -> Self {
        Self::build(
            protocol,
            "exactly-once-premise",
            "",
            &format!("requires temporal {clause};\n"),
            super::projection::PICK,
            super::projection::CANCELLED,
            clause,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        protocol: &str,
        delivery: &str,
        warehouse: &str,
        extra: &str,
        ship: &str,
        cancel: &str,
        clause: &str,
        bounded: bool,
    ) -> Self {
        let (profile, temporal) = if clause.is_empty() {
            (String::new(), String::new())
        } else {
            (
                "profile tt = \"filament.temporal.infinite-trace/v1\";\n".to_owned(),
                format!(
                    "temporal {clause} using tt over (c: M::Workflow) clock \"model-steps\" \
                     on origin {{\nfair weak whole M::Workflow::apply;\nalways true }}\n"
                ),
            )
        };
        let mut members = MEMBERS.replace("exactly-once-premise", delivery);
        if !warehouse.is_empty() {
            members = members.replace("role Warehouse on M::Depot;\n", warehouse);
        }
        // The unbounded model's quantities are of the unit `U`, which the unit
        // declares (the bounded model has no quantity).
        let money = if bounded {
            ""
        } else {
            super::handoff::MONEY_UNIT
        };
        let text = format!(
            "language \"ix:native\" edition \"1-draft\";\n\
             profile pp = \"filament.protocol.finite-global/v1\";\n\
             {profile}model M = \"{PACKAGE}\";\n{money}\
             {temporal}{RECORDS}protocol {protocol} using pp over (k: M::Workflow) on origin {{\n{members}{extra}\n\
             run sequence Main {{\n\
             send Placed via Order as (placed: Ack) {{ true }};\n\
             receive Got via Order of Placed as (got: Ack) {{ true }};\n\
             choice Decide by Seller visible (got.ok) {{\n\
               case no when {{ not got.ok }} sequence Cancel {{ {cancel} }}\n\
               case yes when {{ got.ok }} sequence Ship {{ {ship} }}\n\
             }} }}\n\
             finish Done as (zz: Boolean) {{ true }};\n}}\n"
        );
        let inputs = tempfile::tempdir().expect("temporary directory");
        let model_path = inputs.path().join("model.semantic-ir.json");
        std::fs::write(&model_path, model(bounded)).expect("model file");
        let source_path = inputs.path().join("unit.native");
        std::fs::write(&source_path, &text).expect("source file");
        let mut asked = request("event_position", Corpus::None);
        asked.emit.source = source_path;
        asked.emit.models = vec![model_path];
        asked.emit.temporal_selections = Vec::new();
        let handoff = written_request(asked);
        let compiled = compile(&handoff.root).expect("the handoff compiles back");
        let (_, document) = compiled
            .packages()
            .iter()
            .find(|(identity, _)| identity.as_str() == PACKAGE)
            .expect("the model package");
        let snapshot = snapshot(&serde_json::from_slice(document).expect("model JSON"));
        let mut snapshots = DocumentProvision::new();
        snapshots.insert(
            document_reference(&snapshot).expect("a document names itself"),
            snapshot,
        );
        let clause = compiled
            .admitted()
            .resolved_graph()
            .nodes
            .iter()
            .find(|node| &*node.semantic_form == "protocol_clause")
            .expect("one protocol clause")
            .node_id
            .clone();
        let linked = link(compiled.admitted(), &clause).expect("the protocol links");
        Self {
            handoff,
            compiled,
            snapshots,
            initial: vec![DocumentRef {
                authority: "test".to_owned(),
                identity: "s0".to_owned(),
            }],
            clause,
            linked,
            protocol: protocol.to_owned(),
            depth: 64,
            unit: text,
        }
    }

    /// `checked-package.json` of the handoff with `mutate` applied to its
    /// wire JSON, read by Contract IR's reader with the writer's evidence.
    pub fn read_mutated(&self, mutate: impl FnOnce(&mut Json)) -> CheckedPackageDispatchResult {
        let mut wire: Json = serde_json::from_slice(&self.handoff.files["checked-package.json"])
            .expect("the package is JSON");
        mutate(&mut wire);
        filament_contract_model::read_checked_package(
            &serde_json::to_vec(&wire).expect("JSON bytes"),
            CheckedPackageReadLimits::bounded(),
            &self.handoff.evidence,
        )
    }

    /// The admitted package, the request's own form.
    pub fn package(&self) -> &CheckedPackageV2 {
        self.compiled.admitted()
    }

    /// FSL's checked package of the compiled handoff.
    pub fn checked(&self) -> &fsl_package::CheckedPackage {
        self.compiled.checked()
    }

    /// The node of form `form` whose declared name ends in `last`.
    pub fn named(&self, form: &str, last: &str) -> CheckedNodeId {
        self.package()
            .resolved_graph()
            .nodes
            .iter()
            .find(|node| {
                &*node.semantic_form == form
                    && node.declaration.as_ref().is_some_and(|declaration| {
                        declaration.qualified_name.last().map(|segment| &**segment) == Some(last)
                    })
            })
            .unwrap_or_else(|| panic!("no {form} named {last}"))
            .node_id
            .clone()
    }

    /// The protocol clause.
    pub fn clause(&self) -> &CheckedNodeId {
        &self.clause
    }

    /// The linked protocol.
    pub fn linked(&self) -> &LinkedProtocol {
        &self.linked
    }

    /// The protocol subject FSL builds from the compiled handoff alone.
    pub fn subject(&self) -> ProtocolSubject<'_> {
        protocol_subject(
            SubjectPackage {
                package: self.compiled.checked(),
                packages: self.compiled.packages(),
                snapshots: &self.snapshots,
            },
            SubjectBindings {
                protocol: self.protocol.clone(),
                over: ModelInstance {
                    population: key("workflows"),
                    key: "w".to_owned(),
                },
                roles: ["Buyer", "Seller", "Warehouse"]
                    .into_iter()
                    .map(|role| RoleBinding {
                        role: role.to_owned(),
                        model: PACKAGE.to_owned(),
                    })
                    .collect(),
                memory: Vec::new(),
                initial: self.initial.clone(),
                universes: [
                    ("workflows", vec!["w".to_owned()]),
                    ("workers", Vec::new()),
                    ("depots", Vec::new()),
                ]
                .into_iter()
                .map(|(name, keys)| PopulationUniverse {
                    population: key(name),
                    keys,
                })
                .collect(),
                normalization: ModelNormalizationLimits::default(),
                observation: ObservationLimits::default(),
                expansion: ExpansionLimits {
                    max_candidates: 1_000_000,
                },
                meter: UNLIMITED,
                emit: self.compiled.emit_limits(),
            },
        )
        .expect("the protocol subject is built")
    }

    /// The realizability request over the protocol, bounded at its depth (64 unless set).
    pub fn request(&self) -> RealizabilityRequest<'_> {
        RealizabilityRequest {
            protocol: self.clause.clone(),
            subject: self.subject(),
            definition_key: Some(format!("ix://test/{}", self.protocol)),
            strategy_profile: Some("bounded-static".to_owned()),
            role_knowledge: Some(Some(BTreeSet::new())),
            environment: Some(Some(BTreeSet::new())),
            bounds: Some(BoundsDraft {
                max_depth: Some(self.depth),
                max_history: Some(16),
            }),
            assessed_root: Some(assessed_root()),
            budget: RunBudget::default(),
        }
    }
}

impl Realizability {
    /// The first `count` steps of a run of the protocol, taken from FSL's own
    /// protocol system and spelled as FSpec FR-433 spells a counterexample's
    /// steps: each step's transition identity, the key of its post-state and,
    /// for a `send`, its send record. The first successor of each state is
    /// taken, so the run is the same every time.
    pub fn trace_steps(&self, count: usize) -> Vec<Json> {
        use base64::Engine as _;
        let subject = self.subject();
        let model = ModelSystem::new(subject.model.simulation.clone()).expect("the subject admits");
        let system = ProtocolSystem::new(model, &subject, Sc).expect("the protocol system builds");
        let mut state = system.initial().remove(0);
        let mut steps = Vec::new();
        for _ in 0..count {
            let (transition, post): (ProtocolTransition, _) = system
                .successors(&state)
                .expect("the state expands")
                .successors
                .into_iter()
                .next()
                .expect("the run has a next step");
            let key = system.canonical_state_key(&post).expect("the key encodes");
            let mut step = json!({
                "transition": transition_json(&transition).expect("the identity is spelled"),
                "post_state": {"key": base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(key.as_bytes())},
            });
            if let Some(record) = transition.send_record() {
                step["send_record"] = send_record_json(record).expect("the record is spelled");
            }
            steps.push(step);
            state = post;
        }
        steps
    }

    /// The snapshots of this fixture with the trace document `steps` spell,
    /// provisioned under the reference the document names itself by.
    pub fn with_trace(&self, identity: &str, steps: &[Json]) -> (DocumentProvision, DocumentRef) {
        let document = json!({
            "format": "filament.protocol-trace/v1",
            "identity": {"authority": "test", "identity": identity},
            "prefix": steps,
        })
        .to_string()
        .into_bytes();
        let reference = document_reference(&document).expect("a document names itself");
        let mut snapshots = self.snapshots.clone();
        snapshots.insert(reference.clone(), document);
        (snapshots, reference)
    }

    /// FSL's `analyze` request for the `finite-replay` item that replays the
    /// document `trace` (provisioned in `snapshots`) over the protocol, with
    /// the disposition FR-290 settles for it.
    pub fn replay_request<'a>(
        &'a self,
        snapshots: &'a DocumentProvision,
        trace: DocumentRef,
    ) -> FiniteReplayRequest<'a> {
        let package = self.compiled.checked();
        let graph = package.graph();
        let node = graph
            .protocol_node_keys(&self.protocol)
            .and_then(|keys| keys.clause.clone())
            .expect("the protocol has a clause");
        let (node_key, origin, _) = graph
            .occurrences()
            .find(|(candidate, _, _)| *candidate == node)
            .expect("the clause occurs");
        let mut item = ClaimItem::new(
            package,
            RequestIndex::new(0),
            OccurrenceKey::new(node_key, origin),
            Capability::FiniteReplay,
            ExtentClassification::Bounded,
        )
        .expect("a finite-replay claim over a protocol");
        item.options = ItemOptions {
            required: true,
            max_cases: None,
            trace: Some(trace),
        };
        let descriptor =
            filament_protocol::engine::engine_descriptor(fsl_route::ProviderOrigin::Linked)
                .expect("FSL's own descriptor");
        FiniteReplayRequest {
            package,
            request: AnalyzeRequest {
                binding: SubjectBinding {
                    packages: self.compiled.packages(),
                    snapshots,
                    initial: self.initial.clone(),
                    universes: ["workflows", "workers", "depots"]
                        .into_iter()
                        .map(|name| PopulationUniverse {
                            population: key(name),
                            keys: if name == "workflows" {
                                vec!["w".to_owned()]
                            } else {
                                Vec::new()
                            },
                        })
                        .collect(),
                },
                items: vec![NegotiatedItem {
                    item,
                    disposition: Disposition::Supported(descriptor.candidate().clone()),
                    diagnostic: None,
                }],
                provision: {
                    let mut provision = SourceProvision::default();
                    for source in graph.sources() {
                        provision.insert(source, self.unit.clone().into_bytes());
                    }
                    provision
                },
            },
            limits: AnalyzeLimits::unbounded(),
        }
    }
}

/// The `native-run-result/2` record FSL's engine writes for a refuted
/// `realizability` result, and the FR-351 witness it holds: what a decisive
/// stand-in result carries.
pub fn refuted_record() -> (
    fsl_replay::spine::ProtocolAnalysisRecord,
    fsl_replay::SeparatingWitnessRecord,
) {
    use fsl_analyze::protocol_analysis::{realize, ProtocolAnalysisArtifact};
    use fsl_replay::protocol_analysis::AnalysisOutcome;

    let fixture = Realizability::worked(super::projection::PICK, "");
    let result = realize(fixture.package(), fixture.request()).expect("the request is admitted");
    let AnalysisOutcome::Refuted(counterexample) = &result.outcome else {
        panic!("a refutation: {:?}", result.outcome);
    };
    let witness = counterexample.separating_witness().clone();
    let record = ProtocolAnalysisArtifact::Realizability(Box::new(result))
        .record()
        .expect("FSL writes the record");
    (record, witness)
}

/// Dispatch a `realizability` claim over `fixture` to FSL's registered engine
/// and return its result, as TC-166 and TC-148 do.
pub fn settle(fixture: &Realizability) -> filament_protocol::engine::EngineResult {
    use filament_protocol::dispatch::{
        dispatch, Binding, ProtocolClaimKind, ProtocolOutcome, ProviderBoundary,
    };
    use filament_protocol::disposition::ClaimSettlement;
    use filament_protocol::engine::{engine_descriptor, EngineRequest, FslEngine};
    use fsl_analyze::protocol_analysis::manifest::BACKEND_ID;
    use fsl_route::ProviderOrigin;

    use super::projection::{claim_of, Asked};

    let mut boundary: ProviderBoundary<'_, Asked, filament_protocol::engine::EngineResult> =
        ProviderBoundary::new();
    let engine = FslEngine::new(fixture.package(), |_: &Binding<()>| {
        EngineRequest::Realizability(Box::new(fixture.request()))
    })
    .expect("FSL's linked engines register");
    boundary
        .register(
            engine_descriptor(ProviderOrigin::Linked).expect("FSL's own descriptor"),
            engine,
        )
        .expect("the registration is accepted");
    let asked = claim_of(
        ProtocolClaimKind::Realizability,
        fixture.linked(),
        fixture.clause(),
        |form, name| fixture.named(form, name),
    );
    match dispatch(&mut boundary, fixture.linked().declarations(), &asked) {
        ProtocolOutcome::Settled(ClaimSettlement::Result { result, premises }) => {
            assert_eq!(
                premises.selected.as_ref().map(|id| id.as_str()),
                Some(BACKEND_ID)
            );
            result
        }
        other => panic!("the engine decides the claim: {other:?}"),
    }
}
