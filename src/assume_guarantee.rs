// SPDX-License-Identifier: AGPL-3.0-or-later
//! FR-200, FR-201, FR-202, FR-203, FR-204: the `composition` and `refinement`
//! claim requests, their premise lists and their binding.
//!
//! A `composition` claim is decided by this library
//! ([`composition`](crate::composition), ADR-010 Decision 6). A `refinement`
//! claim is dispatched to the backend FSL's registry selects through the same
//! registry and provider boundary as FR-150
//! ([`ProviderBoundary`](crate::dispatch::ProviderBoundary)) and the result is
//! carried unchanged. For a refinement the only decisions made here are
//! inclusion tests on identities: a result may not name a subject the request
//! did not name, may not retain premise lists that differ from the request's,
//! and may not retain an assumption identity the request did not state.

use std::collections::BTreeSet;

use filament_contract_model::CheckedNodeId;
use fsl_foundation::BackendId;
use fsl_replay::{ItemLabel, SettlementBasis, Truth};
use fsl_semantics::check::Capability;

use crate::closure::AssessmentExecution;
use crate::composition::{cycle_members, Guarantee};
use crate::declarations::Declarations;
use crate::dispatch::{
    settle_bound, Answer, DispositionRefusal, Fragment, ProtocolOutcome, ProviderBoundary,
};
use crate::disposition::ClaimSettlement;

/// The two claim kinds this module dispatches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssumeGuaranteeKind {
    /// `composition`.
    Composition,
    /// `refinement`.
    Refinement,
}

impl AssumeGuaranteeKind {
    /// FSL's capability kind for the claim kind; `None` for a composition,
    /// which no FSL backend decides.
    #[must_use]
    pub const fn capability(self) -> Option<Capability> {
        match self {
            Self::Composition => None,
            Self::Refinement => Some(Capability::Refinement),
        }
    }
}

/// One of the four premise lists of `filament-specification` FR-489.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PremiseList {
    /// `environment`.
    Environment,
    /// `relies`.
    Relies,
    /// `liveness_dependencies`.
    LivenessDependencies,
    /// `fairness`.
    Fairness,
}

/// The four premise lists, kept separate (FR-201). `T` is one entry as the
/// requester states it; entries are compared and forwarded as stated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PremiseLists<T> {
    /// The environment premises.
    pub environment: Vec<T>,
    /// The rely premises.
    pub relies: Vec<T>,
    /// The liveness-dependency premises.
    pub liveness_dependencies: Vec<T>,
    /// The fairness premises.
    pub fairness: Vec<T>,
}

impl<T> Default for PremiseLists<T> {
    fn default() -> Self {
        Self {
            environment: Vec::new(),
            relies: Vec::new(),
            liveness_dependencies: Vec::new(),
            fairness: Vec::new(),
        }
    }
}

impl<T: PartialEq> PremiseLists<T> {
    /// The first list of `self` that differs from the same list of `other`.
    #[must_use]
    pub fn first_difference(&self, other: &Self) -> Option<PremiseList> {
        [
            (
                PremiseList::Environment,
                &self.environment,
                &other.environment,
            ),
            (PremiseList::Relies, &self.relies, &other.relies),
            (
                PremiseList::LivenessDependencies,
                &self.liveness_dependencies,
                &other.liveness_dependencies,
            ),
            (PremiseList::Fairness, &self.fairness, &other.fairness),
        ]
        .into_iter()
        .find_map(|(list, left, right)| (left != right).then_some(list))
    }
}

/// One requested `composition` or `refinement` claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssumeGuaranteeRequest<T> {
    /// The claim kind.
    pub kind: AssumeGuaranteeKind,
    /// The admitted fragment the claim ranges over.
    pub fragment: Fragment,
    /// The component subjects: the component protocols of a composition, the
    /// implementation and specification protocols of a refinement.
    pub subjects: Vec<CheckedNodeId>,
    /// The backend the request names, when it names one.
    pub backend: Option<BackendId>,
    /// The identities of the assumptions the claim is requested under.
    pub assumptions: Vec<CheckedNodeId>,
    /// The four premise lists; empty for a refinement.
    pub premises: PremiseLists<T>,
    /// The component guarantees of a composition and what each proof used
    /// (FR-176); empty for a refinement. The dependency cycles are computed
    /// from these (FR-203), never declared.
    pub guarantees: Vec<Guarantee>,
    /// The source identities the disposition retains (FR-030).
    pub sources: Vec<CheckedNodeId>,
}

/// What a backend is sent and what a disposition retains: the request exactly
/// as made, and the backend FR-290 selected for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssumeGuaranteeBinding<T> {
    /// The request, unchanged.
    pub request: AssumeGuaranteeRequest<T>,
    /// The backend the claim was selected against, once FR-290 selects one.
    pub selected: Option<BackendId>,
}

/// What the dispatcher reads of a backend's refinement result.
/// The result itself is FSL's and is carried unchanged.
pub trait AssumeGuaranteeResult<T> {
    /// The result label, as FSL's backend states it.
    fn label(&self) -> ItemLabel;
    /// The component subjects the result names.
    fn subjects(&self) -> &[CheckedNodeId];
    /// The premise lists the result retains, when it retains them.
    fn retained_premises(&self) -> Option<&PremiseLists<T>>;
    /// The assumption identities the result retains; `None` when the result
    /// omits the field.
    fn retained_assumptions(&self) -> Option<&[CheckedNodeId]>;
}

/// A boundary that is never asked a kind has no result of that kind.
impl<T> AssumeGuaranteeResult<T> for std::convert::Infallible {
    fn label(&self) -> ItemLabel {
        match *self {}
    }
    fn subjects(&self) -> &[CheckedNodeId] {
        match *self {}
    }
    fn retained_premises(&self) -> Option<&PremiseLists<T>> {
        match *self {}
    }
    fn retained_assumptions(&self) -> Option<&[CheckedNodeId]> {
        match *self {}
    }
}

/// The values a refinement result's label reads into (FR-202). A value is
/// `None` where no owner table derives it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LabelReading {
    /// The label, as the backend returned it.
    pub label: ItemLabel,
    /// The FR-241 assessment-execution value.
    pub execution: Option<AssessmentExecution>,
    /// The FR-242 truth.
    pub truth: Option<Truth>,
    /// The FR-243 settlement basis.
    pub basis: Option<SettlementBasis>,
}

/// A refinement label has no owner table into the axes (FR-202): it is
/// carried and reads into nothing.
#[must_use]
pub const fn read_refinement_label(label: ItemLabel) -> LabelReading {
    LabelReading {
        label,
        execution: None,
        truth: None,
        basis: None,
    }
}

/// The declarations of the component package that owns `node`.
pub(crate) fn owner<'a>(
    components: &[&'a Declarations],
    node: &CheckedNodeId,
) -> Option<&'a Declarations> {
    components.iter().copied().find(|declarations| {
        declarations.package() == &*node.package && declarations.declares(node)
    })
}

/// Bind `request` to its component packages (FR-030, FR-200). The components
/// live in separate checked packages (FR-043, FR-044), so `components` holds
/// the declarations of each, and every fragment node, subject, assumption
/// identity and source identity is checked against the package that owns it.
/// No guarantee on a dependency cycle may be offered as an assumption (FR-203).
///
/// # Errors
///
/// [`DispositionRefusal`] naming the absent, undeclared or offending node.
pub fn bind<T: Clone>(
    components: &[&Declarations],
    request: &AssumeGuaranteeRequest<T>,
) -> Result<AssumeGuaranteeBinding<T>, DispositionRefusal> {
    if request.fragment.channels.is_empty() && request.fragment.roles.is_empty() {
        return Err(DispositionRefusal::NoFragment);
    }
    for node in &request.fragment.channels {
        if owner(components, node).is_none_or(|owner| owner.channel(node).is_none()) {
            return Err(DispositionRefusal::UndeclaredChannel(node.clone()));
        }
    }
    for node in &request.fragment.roles {
        if owner(components, node).is_none_or(|owner| owner.role(node).is_none()) {
            return Err(DispositionRefusal::UndeclaredRole(node.clone()));
        }
    }
    if request.subjects.is_empty() {
        return Err(DispositionRefusal::NoSubjects);
    }
    if let Some(node) = request
        .subjects
        .iter()
        .find(|node| owner(components, node).is_none())
    {
        return Err(DispositionRefusal::UndeclaredSubject(node.clone()));
    }
    if let Some(node) = request
        .assumptions
        .iter()
        .find(|node| owner(components, node).is_none())
    {
        return Err(DispositionRefusal::UndeclaredAssumption(node.clone()));
    }
    let cycles = cycle_members(&request.guarantees);
    if let Some(node) = cycle_member(&cycles, &request.assumptions) {
        return Err(DispositionRefusal::CycleGuaranteeAsAssumption(node));
    }
    if let Some(node) = request
        .sources
        .iter()
        .find(|node| owner(components, node).is_none())
    {
        return Err(DispositionRefusal::UndeclaredSource(node.clone()));
    }
    Ok(AssumeGuaranteeBinding {
        request: request.clone(),
        selected: None,
    })
}

/// The first of `offered` that is a guarantee of a dependency cycle.
fn cycle_member(
    cycles: &BTreeSet<CheckedNodeId>,
    offered: &[CheckedNodeId],
) -> Option<CheckedNodeId> {
    offered.iter().find(|node| cycles.contains(*node)).cloned()
}

/// Settle one `refinement` claim: bind it to its component packages, then take
/// the steps every backend-decided claim shares (`settle_bound`). A
/// `composition` claim is decided by [`composition::decide`](crate::composition::decide),
/// never by a backend, and is refused here.
pub fn dispatch<K, P, Q, T>(
    boundary: &mut ProviderBoundary<'_, K, Answer<P, Q>>,
    components: &[&Declarations],
    request: &AssumeGuaranteeRequest<T>,
) -> ProtocolOutcome<Q, AssumeGuaranteeBinding<T>>
where
    K: From<AssumeGuaranteeBinding<T>>,
    T: Clone + PartialEq,
    Q: AssumeGuaranteeResult<T>,
{
    let Some(capability) = request.kind.capability() else {
        return ProtocolOutcome::Refused(DispositionRefusal::CompositionIsDecidedHere);
    };
    if !request.guarantees.is_empty() || request.premises != PremiseLists::default() {
        return ProtocolOutcome::Refused(DispositionRefusal::RefinementCarriesCompositionInputs);
    }
    let binding = match bind(components, request) {
        Ok(binding) => binding,
        Err(refusal) => return ProtocolOutcome::Refused(refusal),
    };
    settle_bound(
        boundary,
        binding,
        (capability, request.backend.as_ref()),
        |binding| K::from(binding.clone()),
        |binding, selected| binding.selected = Some(selected),
        |answer| match answer {
            Answer::AssumeGuarantee(result) => Some(result),
            Answer::Protocol(_) => None,
        },
        |result, binding| carry_refusal(&binding.request, result).map(Into::into),
    )
}

impl<T, R: AssumeGuaranteeResult<T>> ProtocolOutcome<R, AssumeGuaranteeBinding<T>> {
    /// The reading of a carried refinement result's label (FR-202): into
    /// nothing, because no owner table maps it.
    #[must_use]
    pub fn label_reading(&self) -> Option<LabelReading> {
        let Self::Settled(ClaimSettlement::Result { result, .. }) = self else {
            return None;
        };
        Some(read_refinement_label(result.label()))
    }
}

/// The labels on which a verdict rests: a decisive label needs a source
/// identity to be attributed to (FR-030).
const fn is_decisive(label: ItemLabel) -> bool {
    matches!(label, ItemLabel::Proved | ItemLabel::Refuted)
}

/// The identities of `held` that `allowed` does not contain.
fn extra(held: &[CheckedNodeId], allowed: &[CheckedNodeId]) -> Vec<CheckedNodeId> {
    held.iter()
        .filter(|identity| !allowed.contains(identity))
        .cloned()
        .collect()
}

/// FR-201, FR-202, FR-203, FR-204, FR-030: a refinement result is carried only when it
/// was established for the request it is attributed to. Identities are
/// compared by inclusion and the lists by equality; nothing is entailed,
/// ordered or discharged.
fn carry_refusal<T: PartialEq, R: AssumeGuaranteeResult<T>>(
    request: &AssumeGuaranteeRequest<T>,
    result: &R,
) -> Option<DispositionRefusal> {
    if is_decisive(result.label()) && request.sources.is_empty() {
        return Some(DispositionRefusal::NoSourceIdentity);
    }
    let foreign = extra(result.subjects(), &request.subjects);
    if !foreign.is_empty() {
        return Some(DispositionRefusal::IdentityMismatch(
            IdentityMismatch::Subjects(foreign),
        ));
    }
    if let Some(list) = result
        .retained_premises()
        .and_then(|retained| retained.first_difference(&request.premises))
    {
        return Some(DispositionRefusal::RetainedPremiseList(list));
    }
    let retained = result.retained_assumptions();
    if let Some(retained) = retained {
        if let Some(node) = cycle_member(&cycle_members(&request.guarantees), retained) {
            return Some(DispositionRefusal::CycleGuaranteeAsAssumption(node));
        }
        let stronger = extra(retained, &request.assumptions);
        if !stronger.is_empty() {
            return Some(DispositionRefusal::IdentityMismatch(
                IdentityMismatch::Assumptions(stronger),
            ));
        }
    } else {
        return Some(DispositionRefusal::IdentityMismatch(
            IdentityMismatch::AbsentAssumptions,
        ));
    }
    None
}

/// What an `identity-mismatch` refusal names (FR-008's cause of that name).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityMismatch {
    /// The result names component subjects the request did not; each is
    /// listed.
    Subjects(Vec<CheckedNodeId>),
    /// The result retains assumption identities the request did not state: it
    /// was established under stronger assumptions (FR-204); each is listed.
    Assumptions(Vec<CheckedNodeId>),
    /// The result omits its retained assumption identities, which compare
    /// only to absence (FR-030-AC-4).
    AbsentAssumptions,
}
