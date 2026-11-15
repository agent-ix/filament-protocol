// SPDX-License-Identifier: AGPL-3.0-or-later
//! FR-200, FR-201, FR-202, FR-203, FR-204: the assume-guarantee composition
//! decision procedure (ADR-010 Decision 6).
//!
//! This library decides a `composition` claim at the protocol level. It reads
//! the guarantees the requester declares, what each guarantee's proof used and
//! the four premise lists of `filament-specification` FR-489, and decides:
//!
//! - **Cycles.** A guarantee on a dependency cycle (a strongly connected
//!   component of the declared guarantees, or a guarantee that uses itself) is
//!   never available. It discharges no premise, its own cycle's included, and
//!   so does no guarantee whose proof used one. No fixed point is computed
//!   (FSL FR-176 accepts a cycle only under a profile that declares one, and
//!   none is declared here).
//! - **Discharge.** A `rely` premise is discharged only by a declared,
//!   proved, available safety guarantee, a liveness dependency only by one of
//!   liveness form, and an `environment` or `fairness` premise only by an
//!   offered assumption. A guarantee of one form never discharges a premise of
//!   the other.
//! - **Stronger assumptions.** When the proof of a relied-on guarantee
//!   reaches an assumption the request did not offer, the composition rests on
//!   more than was offered and the result is a paired, replayable
//!   [`Counterexample`].
//!
//! The atomic and temporal obligations behind a guarantee are FSL's: the
//! requester carries FSL's label for each guarantee ([`Guarantee::label`]) and
//! only `proved` makes a guarantee available. This module evaluates no
//! temporal formula and no model-level refinement.

use std::collections::{BTreeMap, BTreeSet};

use filament_contract_model::CheckedNodeId;
use fsl_replay::planner::ClauseForm;
use fsl_replay::{ItemLabel, SettlementBasis, Truth};

use crate::assume_guarantee::{
    bind as bind_claim, owner, AssumeGuaranteeBinding, AssumeGuaranteeRequest, PremiseList,
};
use crate::closure::AssessmentExecution;
use crate::declarations::Declarations;
use crate::dispatch::DispositionRefusal;

/// One component guarantee and what its proof used (FSL FR-176: the nodes are
/// qualified guarantees and the edges name the assumptions used in their
/// proofs).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Guarantee {
    /// The guarantee's checked node.
    pub id: CheckedNodeId,
    /// Whether it is a safety or a liveness clause (FSL FR-363).
    pub form: ClauseForm,
    /// FSL's label for the guarantee's own obligation. Only `proved` makes the
    /// guarantee available; every other label, `unsupported` included, leaves
    /// it unavailable.
    pub label: ItemLabel,
    /// The nodes its proof used: sibling guarantees or assumptions.
    pub uses: Vec<CheckedNodeId>,
}

/// What a premise entry names: the node that must discharge it. For an
/// `environment` or `fairness` premise this is an offered assumption; for a
/// `relies` or `liveness_dependencies` premise it is the peer guarantee.
pub trait Premise {
    /// The node that discharges the premise.
    fn discharger(&self) -> &CheckedNodeId;
}

impl Premise for CheckedNodeId {
    fn discharger(&self) -> &CheckedNodeId {
        self
    }
}

/// One premise: its list and its position in that list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PremiseRef {
    /// The list.
    pub list: PremiseList,
    /// The entry's index in the request's list.
    pub index: usize,
}

/// What discharged a premise.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DischargedBy {
    /// An assumption the request offered.
    Assumption(CheckedNodeId),
    /// A proved guarantee outside every cycle whose proof used only offered
    /// assumptions and available guarantees.
    Guarantee(CheckedNodeId),
}

/// One entry of the premise discharge record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Discharge {
    /// The premise.
    pub premise: PremiseRef,
    /// What discharged it.
    pub by: DischargedBy,
}

/// Why a premise stays undischarged (FSL FR-489 cause `premise-unproved`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Unavailable {
    /// An `environment` or `fairness` premise names no offered assumption.
    NotOffered,
    /// A `relies` or `liveness_dependencies` premise names no declared
    /// guarantee.
    NotDeclared,
    /// The guarantee is of the other form.
    WrongForm(ClauseForm),
    /// FSL's label for the guarantee is not `proved`.
    NotProved(ItemLabel),
    /// The guarantee is on a dependency cycle, so it is never available.
    InCycle,
    /// The guarantee's proof used a guarantee that is unavailable.
    DependsOn(CheckedNodeId),
}

/// A premise left undischarged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Undischarged {
    /// The premise.
    pub premise: PremiseRef,
    /// Why.
    pub why: Unavailable,
}

/// One edge of a proof: `guarantee`'s proof used `uses`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyStep {
    /// The guarantee whose proof it is.
    pub guarantee: CheckedNodeId,
    /// A node that proof used.
    pub uses: CheckedNodeId,
}

/// The counterexample to a composition that rests on more than was offered: the
/// two sides of the pair are the assumptions the request offered and the proof
/// path from the relied-on guarantee to an assumption it did not.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Counterexample {
    /// The premise that relies on the guarantee.
    pub premise: PremiseRef,
    /// The offered side: the request's assumptions.
    pub offered: Vec<CheckedNodeId>,
    /// The relied side: the replay path, from the relied-on guarantee to the
    /// first node its proof used that was not offered.
    pub path: Vec<DependencyStep>,
}

impl Counterexample {
    /// The assumption the path ends at, which the request did not offer.
    #[must_use]
    pub fn unoffered(&self) -> Option<&CheckedNodeId> {
        self.path.last().map(|step| &step.uses)
    }

    /// Replay the counterexample against `request`: the premise relies on the
    /// path's first guarantee, every step is an edge the request declares, the
    /// steps chain, the offered side is the request's, and the path's end is
    /// neither a declared guarantee nor an offered assumption.
    #[must_use]
    pub fn replay<T: Premise>(&self, request: &AssumeGuaranteeRequest<T>) -> bool {
        let Some(first) = self.path.first() else {
            return false;
        };
        let relied = request
            .premises_list(self.premise.list)
            .get(self.premise.index);
        let declared = |id: &CheckedNodeId| request.guarantees.iter().find(|g| &g.id == id);
        let edges_exist = self.path.iter().all(|step| {
            declared(&step.guarantee).is_some_and(|guarantee| guarantee.uses.contains(&step.uses))
        });
        let chained = self
            .path
            .windows(2)
            .all(|pair| pair[0].uses == pair[1].guarantee);
        let ends_unoffered = self
            .unoffered()
            .is_some_and(|end| declared(end).is_none() && !request.assumptions.contains(end));
        relied.is_some_and(|entry| entry.discharger() == &first.guarantee)
            && self.offered == request.assumptions
            && edges_exist
            && chained
            && ends_unoffered
    }
}

impl<T> AssumeGuaranteeRequest<T> {
    /// The request's premise list `list`.
    fn premises_list(&self, list: PremiseList) -> &[T] {
        match list {
            PremiseList::Environment => &self.premises.environment,
            PremiseList::Relies => &self.premises.relies,
            PremiseList::LivenessDependencies => &self.premises.liveness_dependencies,
            PremiseList::Fairness => &self.premises.fairness,
        }
    }
}

/// What a decided composition is, in the order of what it must name first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// Every premise is discharged, no proof rests on more than was offered
    /// and no guarantee is on a cycle.
    Proved,
    /// A proof rests on an assumption the request did not offer.
    Incompatible,
    /// The declared guarantees depend on one another in a cycle. The cycle is
    /// refused as circular reasoning (FSL FR-176) and discharges nothing.
    Circular,
    /// A premise stays undischarged.
    Unproved,
}

/// A decided composition: the verdict's every finding, bound to the request it
/// was decided for (FR-030).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Composition<T> {
    /// The request, unchanged; its four premise lists stay separate.
    pub binding: AssumeGuaranteeBinding<T>,
    /// The premise discharge record, in premise order.
    pub discharges: Vec<Discharge>,
    /// The premises left undischarged.
    pub undischarged: Vec<Undischarged>,
    /// Every dependency cycle, as its guarantees in node order.
    pub cycles: Vec<Vec<CheckedNodeId>>,
    /// The counterexamples to stronger assumptions.
    pub counterexamples: Vec<Counterexample>,
}

impl<T> Composition<T> {
    /// The verdict. It is [`Verdict::Proved`] only when there is no finding of
    /// any other kind.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        if !self.counterexamples.is_empty() {
            Verdict::Incompatible
        } else if !self.cycles.is_empty() {
            Verdict::Circular
        } else if !self.undischarged.is_empty() {
            Verdict::Unproved
        } else {
            Verdict::Proved
        }
    }

    /// The FR-241 assessment-execution value.
    #[must_use]
    pub fn execution(&self) -> AssessmentExecution {
        match self.verdict() {
            Verdict::Proved | Verdict::Incompatible | Verdict::Unproved => {
                AssessmentExecution::Completed
            }
            Verdict::Circular => AssessmentExecution::Refused,
        }
    }

    /// The FR-242 truth: `satisfied` only for [`Verdict::Proved`].
    #[must_use]
    pub fn truth(&self) -> Truth {
        match self.verdict() {
            Verdict::Proved => Truth::Satisfied,
            Verdict::Incompatible => Truth::Violated,
            Verdict::Unproved => Truth::Pending,
            Verdict::Circular => Truth::Unavailable,
        }
    }

    /// The FR-243 settlement basis.
    #[must_use]
    pub fn basis(&self) -> SettlementBasis {
        match self.verdict() {
            Verdict::Proved | Verdict::Incompatible => SettlementBasis::DecisiveWitness,
            Verdict::Unproved => SettlementBasis::Unsettled,
            Verdict::Circular => SettlementBasis::Unavailable,
        }
    }
}

/// The settlement of one composition claim.
// One outcome per requested claim, held briefly; boxing would only hide the
// decision the caller matches on.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompositionOutcome<T> {
    /// The procedure decided the claim.
    Decided(Composition<T>),
    /// The claim is refused under FR-030; it holds no success.
    Refused(DispositionRefusal),
}

/// The guarantees of `guarantees` that lie on a dependency cycle: a guarantee
/// that can reach itself through the edges between declared guarantees.
#[must_use]
pub fn cycle_members(guarantees: &[Guarantee]) -> BTreeSet<CheckedNodeId> {
    reach(guarantees)
        .into_iter()
        .filter(|(id, reached)| reached.contains(id))
        .map(|(id, _)| id)
        .collect()
}

/// For each declared guarantee, every declared guarantee reachable from it by
/// one or more edges.
fn reach(guarantees: &[Guarantee]) -> BTreeMap<CheckedNodeId, BTreeSet<CheckedNodeId>> {
    let edges: BTreeMap<&CheckedNodeId, Vec<&CheckedNodeId>> = guarantees
        .iter()
        .map(|g| {
            let used = g
                .uses
                .iter()
                .filter(|used| guarantees.iter().any(|other| &other.id == *used))
                .collect();
            (&g.id, used)
        })
        .collect();
    guarantees
        .iter()
        .map(|g| {
            let mut seen = BTreeSet::new();
            let mut stack: Vec<&CheckedNodeId> = edges[&g.id].clone();
            while let Some(next) = stack.pop() {
                if seen.insert(next.clone()) {
                    stack.extend(edges[next].iter().copied());
                }
            }
            (g.id.clone(), seen)
        })
        .collect()
}

/// Every dependency cycle as a strongly connected component: the guarantees on
/// it, in node order, one entry per component.
fn cycles(guarantees: &[Guarantee]) -> Vec<Vec<CheckedNodeId>> {
    let reached = reach(guarantees);
    let mut components: BTreeSet<Vec<CheckedNodeId>> = BTreeSet::new();
    for (id, from) in &reached {
        if !from.contains(id) {
            continue;
        }
        let component = from
            .iter()
            .filter(|other| reached[*other].contains(id))
            .cloned()
            .collect();
        components.insert(component);
    }
    components.into_iter().collect()
}

/// Why a guarantee is not available.
enum Blocked {
    /// It, or something it used, is unavailable for this reason.
    Unavailable(Unavailable),
    /// Its proof reaches an assumption that was not offered, by this path.
    Stronger(Vec<DependencyStep>),
}

/// Resolve each declared guarantee: available, or blocked and why. A cycle
/// member is blocked at once; every other guarantee is resolved after the
/// guarantees its proof used, which exist because what remains is acyclic.
fn availability(
    request_guarantees: &[Guarantee],
    offered: &[CheckedNodeId],
) -> BTreeMap<CheckedNodeId, Result<(), Blocked>> {
    let on_cycle = cycle_members(request_guarantees);
    let mut resolved: BTreeMap<CheckedNodeId, Result<(), Blocked>> = request_guarantees
        .iter()
        .filter(|g| on_cycle.contains(&g.id))
        .map(|g| {
            (
                g.id.clone(),
                Err(Blocked::Unavailable(Unavailable::InCycle)),
            )
        })
        .collect();
    let declared = |id: &CheckedNodeId| request_guarantees.iter().any(|g| &g.id == id);
    while resolved.len() < request_guarantees.len() {
        let before = resolved.len();
        for guarantee in request_guarantees {
            if resolved.contains_key(&guarantee.id) {
                continue;
            }
            let ready = guarantee
                .uses
                .iter()
                .all(|used| !declared(used) || resolved.contains_key(used));
            if ready {
                let outcome = resolve(guarantee, &resolved, &declared, offered);
                resolved.insert(guarantee.id.clone(), outcome);
            }
        }
        // What is left would be a cycle, which was resolved first.
        if resolved.len() == before {
            break;
        }
    }
    resolved
}

fn resolve(
    guarantee: &Guarantee,
    resolved: &BTreeMap<CheckedNodeId, Result<(), Blocked>>,
    declared: &dyn Fn(&CheckedNodeId) -> bool,
    offered: &[CheckedNodeId],
) -> Result<(), Blocked> {
    if guarantee.label != ItemLabel::Proved {
        return Err(Blocked::Unavailable(Unavailable::NotProved(
            guarantee.label,
        )));
    }
    for used in &guarantee.uses {
        let step = || DependencyStep {
            guarantee: guarantee.id.clone(),
            uses: used.clone(),
        };
        if declared(used) {
            match &resolved[used] {
                Ok(()) => {}
                Err(Blocked::Unavailable(_)) => {
                    return Err(Blocked::Unavailable(Unavailable::DependsOn(used.clone())));
                }
                Err(Blocked::Stronger(path)) => {
                    let mut through = vec![step()];
                    through.extend(path.iter().cloned());
                    return Err(Blocked::Stronger(through));
                }
            }
        } else if !offered.contains(used) {
            return Err(Blocked::Stronger(vec![step()]));
        }
    }
    Ok(())
}

/// Bind a composition request (FR-030, FR-200): the claim binding of
/// [`assume_guarantee::bind`](crate::assume_guarantee::bind), then the
/// guarantees and premises it declares, each checked against the component
/// package that owns the node. No backend is named, because none decides it.
///
/// # Errors
///
/// [`DispositionRefusal`] naming the offending node or backend.
pub fn bind<T: Clone + Premise>(
    components: &[&Declarations],
    request: &AssumeGuaranteeRequest<T>,
) -> Result<AssumeGuaranteeBinding<T>, DispositionRefusal> {
    if let Some(backend) = &request.backend {
        return Err(DispositionRefusal::BackendNamedForComposition(
            backend.clone(),
        ));
    }
    let binding = bind_claim(components, request)?;
    let declared = |node: &CheckedNodeId| owner(components, node).is_some();
    let mut seen = BTreeSet::new();
    for guarantee in &request.guarantees {
        if !seen.insert(&guarantee.id) {
            return Err(DispositionRefusal::DuplicateGuarantee(guarantee.id.clone()));
        }
        if let Some(node) = std::iter::once(&guarantee.id)
            .chain(&guarantee.uses)
            .find(|node| !declared(node))
        {
            return Err(DispositionRefusal::UndeclaredGuarantee(node.clone()));
        }
    }
    for list in LISTS {
        if let Some(entry) = request
            .premises_list(list)
            .iter()
            .find(|entry| !declared(entry.discharger()))
        {
            return Err(DispositionRefusal::UndeclaredPremise(
                entry.discharger().clone(),
            ));
        }
    }
    Ok(binding)
}

const LISTS: [PremiseList; 4] = [
    PremiseList::Environment,
    PremiseList::Relies,
    PremiseList::LivenessDependencies,
    PremiseList::Fairness,
];

/// Decide one `composition` claim (FR-200): bind it, then run the procedure.
/// A decisive verdict, `proved` or incompatible, needs a source identity to be
/// attributed to (FR-030).
pub fn decide<T: Clone + Premise>(
    components: &[&Declarations],
    request: &AssumeGuaranteeRequest<T>,
) -> CompositionOutcome<T> {
    let binding = match bind(components, request) {
        Ok(binding) => binding,
        Err(refusal) => return CompositionOutcome::Refused(refusal),
    };
    let composition = run(binding);
    let decisive = matches!(
        composition.verdict(),
        Verdict::Proved | Verdict::Incompatible
    );
    if decisive && composition.binding.request.sources.is_empty() {
        return CompositionOutcome::Refused(DispositionRefusal::NoSourceIdentity);
    }
    CompositionOutcome::Decided(composition)
}

/// The procedure over a bound request.
fn run<T: Premise>(binding: AssumeGuaranteeBinding<T>) -> Composition<T> {
    let request = &binding.request;
    let resolved = availability(&request.guarantees, &request.assumptions);
    let guarantee = |id: &CheckedNodeId| request.guarantees.iter().find(|g| &g.id == id);
    let mut discharges = Vec::new();
    let mut undischarged = Vec::new();
    let mut counterexamples = Vec::new();
    for list in LISTS {
        for (index, entry) in request.premises_list(list).iter().enumerate() {
            let premise = PremiseRef { list, index };
            let named = entry.discharger();
            let outcome = match list {
                PremiseList::Environment | PremiseList::Fairness => request
                    .assumptions
                    .contains(named)
                    .then(|| DischargedBy::Assumption(named.clone()))
                    .ok_or(Blocked::Unavailable(Unavailable::NotOffered)),
                PremiseList::Relies | PremiseList::LivenessDependencies => {
                    relied(guarantee(named), list, &resolved)
                        .map(|()| DischargedBy::Guarantee(named.clone()))
                }
            };
            match outcome {
                Ok(by) => discharges.push(Discharge { premise, by }),
                Err(Blocked::Unavailable(why)) => undischarged.push(Undischarged { premise, why }),
                Err(Blocked::Stronger(path)) => counterexamples.push(Counterexample {
                    premise,
                    offered: request.assumptions.clone(),
                    path,
                }),
            }
        }
    }
    Composition {
        cycles: cycles(&request.guarantees),
        binding,
        discharges,
        undischarged,
        counterexamples,
    }
}

/// Whether the declared `guarantee` can discharge a premise of `list`.
fn relied(
    guarantee: Option<&Guarantee>,
    list: PremiseList,
    resolved: &BTreeMap<CheckedNodeId, Result<(), Blocked>>,
) -> Result<(), Blocked> {
    let Some(guarantee) = guarantee else {
        return Err(Blocked::Unavailable(Unavailable::NotDeclared));
    };
    let needed = match list {
        PremiseList::LivenessDependencies => ClauseForm::Liveness,
        _ => ClauseForm::Safety,
    };
    if guarantee.form != needed {
        return Err(Blocked::Unavailable(Unavailable::WrongForm(guarantee.form)));
    }
    match &resolved[&guarantee.id] {
        Ok(()) => Ok(()),
        Err(Blocked::Unavailable(why)) => Err(Blocked::Unavailable(why.clone())),
        Err(Blocked::Stronger(path)) => Err(Blocked::Stronger(path.clone())),
    }
}
