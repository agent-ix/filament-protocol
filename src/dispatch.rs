// SPDX-License-Identifier: AGPL-3.0-or-later
//! FR-150, FR-151, FR-152, FR-153, FR-303: dispatch of `local-projection`,
//! `realizability` and `finite-replay` claims to the backend FSL's registry
//! selects.
//!
//! FSL decides what a backend serves. The registry is FSL's own
//! ([`fsl_route::Registry`], FR-290's candidate computation), and a backend
//! joins it at the provider boundary ([`ProviderBoundary::register`](crate::dispatch::ProviderBoundary::register)) with the
//! descriptor FSL's registry accepts. This library holds no capability table of
//! its own and the closed built-in table of FR-005 gains no entry.
//!
//! The dispatcher computes no projection, no realizability verdict, no
//! strategy and no witness. It binds the claim to its premises (FR-151), asks
//! the one backend FR-290 names, and carries what comes back unchanged
//! (FR-150, FR-152). While no backend serves the kind it settles the claim
//! `unsupported` (FR-153).

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use filament_contract_model::CheckedNodeId;
use filament_semantic_value::catalog::CatalogCode;
use fsl_foundation::BackendId;
use fsl_package::protocol_view::Delivery;
use fsl_replay::spine::ProtocolAnalysisRecord;
use fsl_replay::{
    InconclusiveCause, SeparatingWitnessRecord, SettlementBasis, TerminalRecord, TerminalValue,
    Truth,
};
use fsl_route::{BackendDescriptor, Candidate, CandidateOutcome, RegistrationRefusal, Registry};
use fsl_semantics::check::Capability;

use crate::assume_guarantee::AssumeGuaranteeRequest;
use crate::composition::{decide, CompositionOutcome, Premise};
use crate::declarations::{ChannelDeclaration, Declarations, Ordering, RoleDeclaration};
use crate::disposition::{
    settle_claim, ClaimSettlement, Disposition, FslBoundary, UnsupportedCause,
    UnsupportedDisposition, Warning,
};
use crate::refinement::{decide as decide_refinement, RefinementOutcome, RefinementRequest};
use crate::refusal::ProtocolCauseKind;

/// The claim kinds this module dispatches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolClaimKind {
    /// `local-projection`.
    LocalProjection,
    /// `realizability`.
    Realizability,
    /// `global-conformance` (FR-205): FSL's explicit-state check of every
    /// behaviour of the protocol (FSL FR-215, FR-636). FSL routes that check as
    /// a `temporal-satisfaction` item, because no engine advertises the
    /// `global-conformance` kind itself, so the capability FR-290 selects on is
    /// FSL's `temporal-satisfaction`.
    GlobalConformance,
    /// `finite-replay` (FR-303): FSL's replay of a supplied finite trace of
    /// the protocol (FSL FR-636).
    FiniteReplay,
}

impl ProtocolClaimKind {
    /// FSL's capability kind for the claim kind.
    #[must_use]
    pub const fn capability(self) -> Capability {
        match self {
            Self::LocalProjection => Capability::LocalProjection,
            Self::Realizability => Capability::Realizability,
            Self::GlobalConformance => Capability::TemporalSatisfaction,
            Self::FiniteReplay => Capability::FiniteReplay,
        }
    }
}

/// The nodes of the claim's admitted fragment whose premises the claim
/// depends on.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Fragment {
    /// The channels of the fragment.
    pub channels: Vec<CheckedNodeId>,
    /// The roles of the fragment.
    pub roles: Vec<CheckedNodeId>,
}

/// One channel's premise as the requester states it (FR-151). The ordering
/// and delivery are the components FSL's read view publishes, so they are
/// typed and must equal the checked package's. The sender, receiver and
/// message type are the linked declaration's (FSL publishes them), so the
/// requester states none of them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChannelPremise {
    /// The ordering policy.
    pub ordering: Ordering,
    /// The delivery policy.
    pub delivery: Delivery,
}

/// A role's internality: a binding fact, never read from the checked package
/// (FSpec FR-217, FR-050-AC-10). The vocabulary is exactly `internal` and
/// `external`; `external` is not assumed fully observable.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub enum Internality {
    /// A bound role inside the assessed protocol.
    Internal,
    /// A bound role outside the assessed protocol.
    External,
}

/// The per-claim premises the requester supplied, keyed by the channel or role
/// they are the premise of (FR-151). Each role's internality is the closed
/// [`Internality`] vocabulary, stated by the binding (FSpec FR-217-AC-4).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Premises {
    /// The premise of each channel.
    pub channels: BTreeMap<CheckedNodeId, ChannelPremise>,
    /// The internality of each role.
    pub roles: BTreeMap<CheckedNodeId, Internality>,
}

/// One requested `local-projection`, `realizability` or `global-conformance`
/// claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolRequest<A> {
    /// The claim kind.
    pub kind: ProtocolClaimKind,
    /// The admitted fragment.
    pub fragment: Fragment,
    /// The backend the request names, when it names one.
    pub backend: Option<BackendId>,
    /// The assumptions, as the requester states them.
    pub assumptions: A,
    /// The per-claim premises.
    pub premises: Premises,
    /// The source identities the disposition retains (FR-030).
    pub sources: Vec<CheckedNodeId>,
}

/// What a backend is sent and what a disposition retains (FR-030, FR-151):
/// the request exactly as made, with the channel and role declarations the
/// checked package authors for the fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Binding<A> {
    /// The request, unchanged.
    pub request: ProtocolRequest<A>,
    /// The backend the claim was selected against, once FR-290 selects one.
    pub selected: Option<BackendId>,
    /// The checked package's declaration of each channel of the fragment, in
    /// fragment order.
    pub channels: Vec<ChannelDeclaration>,
    /// The checked package's declaration of each role of the fragment, in
    /// fragment order.
    pub roles: Vec<RoleDeclaration>,
}

/// Why one disposition is refused under FR-030, scoped to that disposition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispositionRefusal {
    /// The fragment names a channel the checked package does not declare.
    UndeclaredChannel(CheckedNodeId),
    /// The fragment names a role the checked package does not declare.
    UndeclaredRole(CheckedNodeId),
    /// The requester's premises omit the premise of a channel of the fragment.
    AbsentChannelPremise(CheckedNodeId),
    /// The requester's premises omit the internality of a role of the
    /// fragment.
    AbsentRoleInternality(CheckedNodeId),
    /// A decisive result was returned for a request that retains no source
    /// identity (FR-152).
    NoSourceIdentity,
    /// The requester's premise for a channel disagrees with the checked
    /// package's declaration of it.
    SubstitutedChannelPremise(CheckedNodeId),
    /// A decisive `realizability` result carries no trace-position witness
    /// (FR-302).
    DecisiveWithoutWitness,
    /// A result whose basis is not decisive carries a witness (FR-302).
    WitnessWithoutDecisiveBasis,
    /// The witness a result carries is not the one its `native-run-result/2`
    /// protocol-analysis record holds, byte for byte (FR-302).
    WitnessNotTheRecords,
    /// The record a result carries states another truth or basis than the
    /// result does (FR-302).
    RecordNotTheResults,
    /// A `finite-replay` result carries no FSL terminal record, or one that is
    /// not the `proved` record its truth and basis state (FR-303).
    ReplayWithoutItsRecord,
    /// FSL's registry selected a backend that has no provider.
    NoProvider(BackendId),
    /// A `composition` or `refinement` request names no component subject
    /// (FR-200).
    NoSubjects,
    /// A `composition` or `refinement` request names no admitted fragment
    /// (FR-200).
    NoFragment,
    /// A component subject is not a node of the checked package's protocol.
    UndeclaredSubject(CheckedNodeId),
    /// An assumption identity is not a node of the checked package's
    /// protocol.
    UndeclaredAssumption(CheckedNodeId),
    /// A source identity is not a node of the checked package's protocol.
    UndeclaredSource(CheckedNodeId),
    /// A guarantee on a dependency cycle (computed from the declared proof edges) is offered as an assumption, in the
    /// request or in the result's retained assumptions (FR-203).
    CycleGuaranteeAsAssumption(CheckedNodeId),
    /// A guarantee, or a node a guarantee's proof used, is not a node of the
    /// checked package's protocol (FR-200).
    UndeclaredGuarantee(CheckedNodeId),
    /// A premise names a discharger that is not a node of the checked
    /// package's protocol (FR-201).
    UndeclaredPremise(CheckedNodeId),
    /// A guarantee is declared twice (FR-200).
    DuplicateGuarantee(CheckedNodeId),
    /// An observable label of a refinement system is not a node of the
    /// checked package's protocol (FR-301).
    UndeclaredLabel(CheckedNodeId),
}

impl DispositionRefusal {
    /// The FR-008 cause this refusal is raised under, where the closed
    /// vocabulary names one: the two exact identities that must agree
    /// differ.
    #[must_use]
    pub const fn cause(&self) -> Option<ProtocolCauseKind> {
        match self {
            Self::SubstitutedChannelPremise(_) => Some(ProtocolCauseKind::IdentityMismatch),
            _ => None,
        }
    }
}

/// What the dispatcher reads of a backend's result. The result itself is
/// FSL's and is carried unchanged.
pub trait BackendResult {
    /// The result's truth.
    fn truth(&self) -> Truth;
    /// The result's settlement basis.
    fn basis(&self) -> SettlementBasis;
    /// The separating-witness record the result carries, when it carries
    /// one.
    fn witness(&self) -> Option<&SeparatingWitnessRecord>;
    /// The `native-run-result/2` protocol-analysis record the result's
    /// witness is imported from (FSpec FR-352), or `None` when the backend
    /// wrote none (FR-302).
    fn run_record(&self) -> Option<&ProtocolAnalysisRecord>;
    /// The FR-331 terminal record FSL's `finite-replay` item settled the
    /// result with (FR-303), or `None` for every other kind and for a backend
    /// that wrote none.
    fn terminal_record(&self) -> Option<&TerminalRecord> {
        None
    }
    /// FSL's `InconclusiveCause` when the result is its `Inconclusive`
    /// verdict.
    fn inconclusive(&self) -> Option<&InconclusiveCause>;
}

/// The settlement of one requested claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolOutcome<R, B> {
    /// The selected backend answered, or the claim settled `unsupported`.
    Settled(ClaimSettlement<R, B>),
    /// FR-290 settled the request itself `invalid-request`
    /// (`ambiguous-backend` or `unknown-backend`) and no backend was called.
    InvalidRequest {
        /// FSL's catalog code, as FR-290 spells it.
        code: CatalogCode,
        /// Every candidate, in candidate order; empty for `unknown-backend`.
        candidates: Vec<Candidate>,
        /// The binding the claim was requested against.
        binding: B,
    },
    /// The disposition is refused under FR-030.
    Refused(DispositionRefusal),
}

impl<R: BackendResult, B> ProtocolOutcome<R, B> {
    /// The truth of the disposition. A refusal, an `unsupported` cause, a
    /// requires-bound outcome, a failure and an `Inconclusive` verdict hold
    /// neither `satisfied` nor `violated`.
    #[must_use]
    pub fn truth(&self) -> Truth {
        match self {
            Self::Settled(ClaimSettlement::Result { result, .. }) => {
                match result.truth() {
                    // A decided truth cannot accompany an `Inconclusive`
                    // verdict; the backend's `pending` and `unavailable` are
                    // carried as it states them.
                    Truth::Satisfied | Truth::Violated if result.inconclusive().is_some() => {
                        Truth::Unavailable
                    }
                    truth => truth,
                }
            }
            Self::Settled(ClaimSettlement::Disposition(_))
            | Self::InvalidRequest { .. }
            | Self::Refused(_) => Truth::Unavailable,
        }
    }
}

/// A requested claim: finite global conformance, whose result the caller
/// computed (FR-004) and this dispatcher carries untouched, or a protocol
/// claim.
// One request per claim, held briefly; boxing would only hide the claim the
// caller matches on.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimRequest<G, A, T> {
    /// `global-conformance`, with its result.
    GlobalConformance(G),
    /// `local-projection` or `realizability`.
    Protocol(ProtocolRequest<A>),
    /// `composition`, decided here (FR-200).
    Composition(AssumeGuaranteeRequest<T>),
    /// `refinement`, decided here (FR-301).
    Refinement(RefinementRequest),
}

/// The disposition of one requested claim, in request order (FR-029).
// One disposition per requested claim, held briefly; boxing would only hide the
// settlement the caller matches on.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimDisposition<G, A, T, P> {
    /// The global conformance result, unchanged.
    GlobalConformance(G),
    /// The protocol claim's settlement.
    Protocol(ProtocolOutcome<P, Binding<A>>),
    /// The composition claim's decision, made by this library (FR-200).
    Composition(CompositionOutcome<T>),
    /// The refinement claim's decision, made by this library (FR-301).
    Refinement(RefinementOutcome),
}

/// Why a provider was not registered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistrationError {
    /// FSL's registry refused the descriptor.
    Registry(RegistrationRefusal),
    /// The identity already has a provider.
    ProviderHeld(BackendId),
}

/// The call into one registered backend.
type Provider<'p, K, R> = Box<dyn FslBoundary<K, R> + 'p>;

/// The provider boundary: FSL's registry, and the call into each backend
/// registered in it. A caller supplies no callback per request. `K` is the
/// claim a provider is asked about: a [`Binding`].
pub struct ProviderBoundary<'p, K, R> {
    registry: Registry,
    providers: BTreeMap<BackendId, Provider<'p, K, R>>,
}

impl<K, R> Default for ProviderBoundary<'_, K, R> {
    fn default() -> Self {
        Self {
            registry: Registry::new(),
            providers: BTreeMap::new(),
        }
    }
}

impl<'p, K, R> ProviderBoundary<'p, K, R> {
    /// An empty boundary: no backend serves any kind.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// FSL's registry snapshot.
    #[must_use]
    pub const fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Register `provider` under `descriptor`, as FSL's registry admits it.
    ///
    /// # Errors
    ///
    /// [`RegistrationError::Registry`] with FSL's `duplicate-backend` refusal
    /// when the identity conflicts; the identity is then served by no
    /// provider. [`RegistrationError::ProviderHeld`] when the identity
    /// already has a provider; the held provider is kept and `provider` is
    /// dropped.
    pub fn register(
        &mut self,
        descriptor: BackendDescriptor,
        provider: impl FslBoundary<K, R> + 'p,
    ) -> Result<(), RegistrationError> {
        let id = descriptor.id().clone();
        match self.registry.register(descriptor) {
            Ok(()) => match self.providers.entry(id) {
                Entry::Occupied(held) => Err(RegistrationError::ProviderHeld(held.key().clone())),
                Entry::Vacant(slot) => {
                    slot.insert(Box::new(provider));
                    Ok(())
                }
            },
            Err(refusal) => {
                self.providers.remove(&id);
                Err(RegistrationError::Registry(refusal))
            }
        }
    }
}

/// FR-290's selection of the one backend that serves `capability`, or the
/// outcome that settles the request without calling a backend: `unsupported`
/// while none serves it, `invalid-request` when the named backend is unknown
/// or the candidates are ambiguous. `binding` is what that outcome retains.
pub(crate) fn select<R, B>(
    registry: &Registry,
    capability: Capability,
    requested: Option<&BackendId>,
    binding: B,
) -> Result<(BackendId, B), Box<ProtocolOutcome<R, B>>> {
    let set = match registry.candidates(capability, requested) {
        CandidateOutcome::UnknownBackend(_) => {
            return Err(Box::new(ProtocolOutcome::InvalidRequest {
                // IR-1513: the owner (filament-contract-codegen) is to publish this code.
                code: CatalogCode::new("invalid_capability", "unknown-backend"),
                candidates: Vec::new(),
                binding,
            }));
        }
        CandidateOutcome::Candidates(set) => set,
    };
    let [candidate] = set.candidates() else {
        return Err(Box::new(if set.is_empty() {
            ProtocolOutcome::Settled(ClaimSettlement::Disposition(Disposition::Unsupported(
                UnsupportedDisposition {
                    cause: UnsupportedCause::requested_capability(),
                    construct: None,
                    premises: binding,
                    warnings: vec![Warning::UnservedCapability {
                        kind: set.kind(),
                        backend: set.requested_backend().cloned(),
                    }],
                },
            )))
        } else {
            ProtocolOutcome::InvalidRequest {
                // IR-1513: the owner (filament-contract-codegen) is to publish this code.
                code: CatalogCode::new("invalid_capability", "ambiguous-backend"),
                candidates: set.candidates().to_vec(),
                binding,
            }
        }));
    };
    Ok((candidate.id().clone(), binding))
}

/// Bind `request` to the checked package's declarations and to the
/// requester's premises (FR-151).
///
/// # Errors
///
/// [`DispositionRefusal`] naming the absent or undeclared component; no
/// default, inferred or sampled premise is supplied.
pub fn bind<A: Clone>(
    declarations: &Declarations,
    request: &ProtocolRequest<A>,
) -> Result<Binding<A>, DispositionRefusal> {
    let mut channels = Vec::with_capacity(request.fragment.channels.len());
    for node in &request.fragment.channels {
        let declared = declarations
            .channel(node)
            .ok_or_else(|| DispositionRefusal::UndeclaredChannel(node.clone()))?;
        let stated = request
            .premises
            .channels
            .get(node)
            .ok_or_else(|| DispositionRefusal::AbsentChannelPremise(node.clone()))?;
        if stated.ordering != declared.ordering || stated.delivery != declared.delivery {
            return Err(DispositionRefusal::SubstitutedChannelPremise(node.clone()));
        }
        channels.push(declared.clone());
    }
    let mut roles = Vec::with_capacity(request.fragment.roles.len());
    for node in &request.fragment.roles {
        let declared = declarations
            .role(node)
            .ok_or_else(|| DispositionRefusal::UndeclaredRole(node.clone()))?;
        if !request.premises.roles.contains_key(node) {
            return Err(DispositionRefusal::AbsentRoleInternality(node.clone()));
        }
        roles.push(declared.clone());
    }
    Ok(Binding {
        request: request.clone(),
        selected: None,
        channels,
        roles,
    })
}

/// The contract version a result that carries a `native-run-result/2`
/// record is produced under (FR-302-AC-5). The record's schema fixes it; it is
/// never read from the witness.
pub const WITNESS_RESULT_CONTRACT: &str = "filament.protocol.result/v1-draft.2";

/// The contract version `result` is produced under: the witness-bearing
/// version when it has a `native-run-result/2` record, and none otherwise.
#[must_use]
pub fn result_contract<R: BackendResult>(result: &R) -> Option<&'static str> {
    result.run_record().map(|_| WITNESS_RESULT_CONTRACT)
}

/// Why a carried result is not settled as a result.
pub(crate) enum Rejected {
    /// The disposition is refused under FR-030.
    Refused(DispositionRefusal),
    /// The backend returned no `native-run-result/2` record for a decisive
    /// result (FR-302-AC-4): the claim settles `unsupported`.
    NoRunRecord,
}

impl From<DispositionRefusal> for Rejected {
    fn from(refusal: DispositionRefusal) -> Self {
        Self::Refused(refusal)
    }
}

/// A boundary that is never asked a kind has no result of that kind.
impl BackendResult for std::convert::Infallible {
    fn truth(&self) -> Truth {
        match *self {}
    }
    fn basis(&self) -> SettlementBasis {
        match *self {}
    }
    fn witness(&self) -> Option<&SeparatingWitnessRecord> {
        match *self {}
    }
    fn run_record(&self) -> Option<&ProtocolAnalysisRecord> {
        match *self {}
    }
    fn inconclusive(&self) -> Option<&InconclusiveCause> {
        match *self {}
    }
}

/// The steps every claim kind shares once its premises are bound: take
/// FR-290's candidate set from FSL's registry, send the claim to the one
/// candidate, carry what returns, and refuse a result that `refuse` rejects.
pub(crate) fn settle_bound<K, R, B: Clone>(
    boundary: &mut ProviderBoundary<'_, K, R>,
    binding: B,
    (capability, requested): (Capability, Option<&BackendId>),
    claim_of: impl FnOnce(&B) -> K,
    mark_selected: impl FnOnce(&mut B, BackendId),
    refuse: impl FnOnce(&R, &B) -> Option<Rejected>,
) -> ProtocolOutcome<R, B> {
    let (selected, mut binding) = match select(&boundary.registry, capability, requested, binding) {
        Ok(selected) => selected,
        Err(outcome) => return *outcome,
    };
    mark_selected(&mut binding, selected.clone());
    let Some(provider) = boundary.providers.get_mut(&selected) else {
        return ProtocolOutcome::Refused(DispositionRefusal::NoProvider(selected));
    };
    let claim = claim_of(&binding);
    match settle_claim(provider.as_mut(), &claim, binding) {
        ClaimSettlement::Result { result, premises } => match refuse(&result, &premises) {
            Some(Rejected::Refused(refusal)) => ProtocolOutcome::Refused(refusal),
            Some(Rejected::NoRunRecord) => ProtocolOutcome::Settled(ClaimSettlement::Disposition(
                Disposition::Unsupported(UnsupportedDisposition {
                    cause: UnsupportedCause::requested_capability(),
                    construct: None,
                    premises,
                    warnings: vec![Warning::NoRunRecord {
                        kind: capability,
                        backend: selected,
                    }],
                }),
            )),
            None => ProtocolOutcome::Settled(ClaimSettlement::Result { result, premises }),
        },
        ClaimSettlement::Disposition(disposition) => {
            ProtocolOutcome::Settled(ClaimSettlement::Disposition(disposition))
        }
    }
}

/// Settle one claim: bind its premises, take FR-290's candidate set from
/// FSL's registry, send the claim to the one candidate, and carry what
/// returns.
pub fn dispatch<A, P>(
    boundary: &mut ProviderBoundary<'_, Binding<A>, P>,
    declarations: &Declarations,
    request: &ProtocolRequest<A>,
) -> ProtocolOutcome<P, Binding<A>>
where
    A: Clone,
    P: BackendResult,
{
    let binding = match bind(declarations, request) {
        Ok(binding) => binding,
        Err(refusal) => return ProtocolOutcome::Refused(refusal),
    };
    settle_bound(
        boundary,
        binding,
        (request.kind.capability(), request.backend.as_ref()),
        Clone::clone,
        |binding, selected| binding.selected = Some(selected),
        |result, binding| carry_refusal(request.kind, result, &binding.request.sources),
    )
}

/// FR-152, FR-302: a decisive result is carried beside the source identities
/// the disposition retains, and a decisive `realizability` result with the
/// trace-position witness its `native-run-result/2` record holds; without them
/// it is refused, and without the record it settles `unsupported`. A result
/// whose basis is not decisive carries no witness. The witness is compared with
/// the record's, never read, normalised or reordered.
fn carry_refusal<R: BackendResult>(
    kind: ProtocolClaimKind,
    result: &R,
    sources: &[CheckedNodeId],
) -> Option<Rejected> {
    if kind == ProtocolClaimKind::FiniteReplay {
        return carry_replay_refusal(result, sources);
    }
    if !result.basis().is_decisive() {
        return result
            .witness()
            .map(|_| DispositionRefusal::WitnessWithoutDecisiveBasis.into());
    }
    if sources.is_empty() {
        return Some(DispositionRefusal::NoSourceIdentity.into());
    }
    if kind != ProtocolClaimKind::Realizability {
        return None;
    }
    let Some(record) = result.run_record() else {
        return Some(Rejected::NoRunRecord);
    };
    let Some(witness) = result
        .witness()
        .filter(|witness| witness.trace_position.is_some())
    else {
        return Some(DispositionRefusal::DecisiveWithoutWitness.into());
    };
    if record.truth() != result.truth() || record.basis() != result.basis() {
        return Some(DispositionRefusal::RecordNotTheResults.into());
    }
    (!record.witness_is(witness)).then(|| DispositionRefusal::WitnessNotTheRecords.into())
}

/// FR-303: a `finite-replay` result is carried beside the source identities the
/// disposition retains, with the terminal record FSL's `finite-replay` item
/// settled it with. A replay that settles a truth is `proved`: truth `satisfied`
/// on basis `closed-scope`, and the record must be FSL's `proved` one. A
/// result with no record settles `unsupported`; a result whose record is not
/// the proved one, or whose truth or basis differ from it, is refused. A replay
/// has no separating witness, so one is refused.
fn carry_replay_refusal<R: BackendResult>(
    result: &R,
    sources: &[CheckedNodeId],
) -> Option<Rejected> {
    if result.witness().is_some() {
        return Some(DispositionRefusal::WitnessWithoutDecisiveBasis.into());
    }
    if sources.is_empty() {
        return Some(DispositionRefusal::NoSourceIdentity.into());
    }
    let Some(record) = result.terminal_record() else {
        return Some(Rejected::NoRunRecord);
    };
    let proved = matches!(record.value(), TerminalValue::Proved { .. });
    let stated =
        result.truth() == Truth::Satisfied && result.basis() == SettlementBasis::ClosedScope;
    (!(proved && stated)).then(|| DispositionRefusal::ReplayWithoutItsRecord.into())
}

/// Settle every requested claim, in request order and independently of one
/// another (FR-029). Global conformance results pass through untouched.
pub fn dispatch_claims<G, A, T, P>(
    boundary: &mut ProviderBoundary<'_, Binding<A>, P>,
    declarations: &Declarations,
    components: &[&Declarations],
    requests: &[ClaimRequest<G, A, T>],
) -> Vec<ClaimDisposition<G, A, T, P>>
where
    G: Clone,
    A: Clone,
    T: Clone + PartialEq + Premise,
    P: BackendResult,
{
    requests
        .iter()
        .map(|request| match request {
            ClaimRequest::GlobalConformance(result) => {
                ClaimDisposition::GlobalConformance(result.clone())
            }
            ClaimRequest::Protocol(request) => {
                ClaimDisposition::Protocol(dispatch(boundary, declarations, request))
            }
            ClaimRequest::Composition(request) => {
                ClaimDisposition::Composition(decide(components, request))
            }
            ClaimRequest::Refinement(request) => {
                ClaimDisposition::Refinement(decide_refinement(components, request))
            }
        })
        .collect()
}
