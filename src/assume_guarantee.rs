// SPDX-License-Identifier: AGPL-3.0-or-later
//! FR-200, FR-201, FR-203: the `composition` claim request, its premise lists
//! and its binding.
//!
//! A `composition` claim is decided by this library
//! ([`composition`](crate::composition), ADR-010 Decision 6). This module
//! holds the request and the binding checks every claim shares: each node the
//! request names is checked against the declarations of the component package
//! that owns it.

use std::collections::BTreeSet;

use filament_contract_model::CheckedNodeId;

use crate::composition::{cycle_members, Guarantee};
use crate::declarations::Declarations;
use crate::dispatch::{DispositionRefusal, Fragment};

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

/// One requested `composition` claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssumeGuaranteeRequest<T> {
    /// The admitted fragment the claim ranges over.
    pub fragment: Fragment,
    /// The component protocols of the composition.
    pub subjects: Vec<CheckedNodeId>,
    /// The identities of the assumptions the claim is requested under.
    pub assumptions: Vec<CheckedNodeId>,
    /// The four premise lists.
    pub premises: PremiseLists<T>,
    /// The component guarantees of the composition and what each proof used
    /// (FR-176). The dependency cycles are computed from these (FR-203),
    /// never declared.
    pub guarantees: Vec<Guarantee>,
    /// The source identities the disposition retains (FR-030).
    pub sources: Vec<CheckedNodeId>,
}

/// What a disposition retains: the request exactly as made (FR-030).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssumeGuaranteeBinding<T> {
    /// The request, unchanged.
    pub request: AssumeGuaranteeRequest<T>,
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

/// Check that `fragment` names at least one node and that each channel and
/// role it names is declared by the component package that owns it
/// (FR-030).
///
/// # Errors
///
/// [`DispositionRefusal`] naming the absent or undeclared node.
pub(crate) fn bind_fragment(
    components: &[&Declarations],
    fragment: &Fragment,
) -> Result<(), DispositionRefusal> {
    if fragment.channels.is_empty() && fragment.roles.is_empty() {
        return Err(DispositionRefusal::NoFragment);
    }
    for node in &fragment.channels {
        if owner(components, node).is_none_or(|owner| owner.channel(node).is_none()) {
            return Err(DispositionRefusal::UndeclaredChannel(node.clone()));
        }
    }
    for node in &fragment.roles {
        if owner(components, node).is_none_or(|owner| owner.role(node).is_none()) {
            return Err(DispositionRefusal::UndeclaredRole(node.clone()));
        }
    }
    Ok(())
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
    bind_fragment(components, &request.fragment)?;
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
    })
}

/// The first of `offered` that is a guarantee of a dependency cycle.
fn cycle_member(
    cycles: &BTreeSet<CheckedNodeId>,
    offered: &[CheckedNodeId],
) -> Option<CheckedNodeId> {
    offered.iter().find(|node| cycles.contains(*node)).cloned()
}
