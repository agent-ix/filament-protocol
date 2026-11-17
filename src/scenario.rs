// SPDX-License-Identifier: AGPL-3.0-or-later
//! FR-206: the scenario record Quoin's intake takes, composed from the live
//! dispositions of a scenario's claims.
//!
//! The record is `quoin_evidence::scenario::ScenarioEvidence`, imported. This
//! module declares no member, enumeration or wire string of its shape; it
//! fills the owner's value from values this library already holds: the
//! version-lock manifest (FR-012), the budget granted and consumed per
//! dimension (FR-016), and the dispositions of the claims (FR-029) with their
//! execution, truth and settlement-basis axes. Nothing is stored or read back
//! here: the value is handed to Quoin's `write_scenario_evidence`.

use filament_semantic_value::catalog::CatalogCode;
use fsl_replay::{SettlementBasis, Truth};
use fsl_semantics::check::Capability;
use quoin_evidence::scenario::{
    BudgetFigure, CausingEntry, ClaimOutcome, Integration, PinnedEntry, ResolutionState,
    ScenarioEvidence, TraceBinding, TypedOutcome, ValueOwner, INTAKE_CONTRACT,
};

use crate::bounds::inventory::{Budget, BudgetDimension, Usage};
use crate::closure::{AssessmentExecution, TruthDisposition};
use crate::composition::CompositionOutcome;
use crate::dispatch::{
    decided_truth, BackendResult, ClaimDisposition, ClaimRequest, ProtocolOutcome,
};
use crate::disposition::{ClaimSettlement, Disposition};
use crate::outcome::OperationOutcome;
use crate::version_lock::{EntryKey, EntryStatus, ExactBytes, ManifestEntry, VersionLockManifest};

use crate::refinement::RefinementOutcome;

/// What one scenario ran against and is bound to.
#[derive(Clone, Copy, Debug)]
pub struct Scenario<'a> {
    /// The integration the scenario belongs to.
    pub integration: Integration,
    /// The scenario id, such as `IT-002-SC-07a`.
    pub scenario: &'a str,
    /// The version-lock manifest the scenario resolved against (FR-012).
    pub manifest: &'a VersionLockManifest,
    /// Each consumed producer value and the entry it resolved through.
    pub value_owners: &'a [(&'a str, &'a EntryKey)],
    /// The budget the request was granted (FR-016).
    pub budget: &'a Budget,
    /// The work the request consumed.
    pub usage: Usage,
    /// The acceptance criterion the scenario is bound to.
    pub criterion: &'a str,
}

/// Why a scenario cannot be reported as executed (FR-206-AC-3). It is never
/// a passing record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Unexecuted {
    /// The manifest holds no entry to attribute the scenario to.
    NoResolution,
    /// An entry component is not text, so it cannot be copied into the record.
    NotText(&'static str),
    /// A pinned entry carries no revision.
    NoRevision,
    /// A consumed value names an entry that is absent from the reached
    /// manifest or not pinned.
    ValueOwnerUnresolved(String),
    /// A budget figure does not fit the record's figure.
    FigureUnrepresentable(&'static str),
    /// The dispositions are not one per requested claim (FR-029).
    DispositionsNotPerRequest {
        /// How many claims were requested.
        requested: usize,
        /// How many dispositions came back.
        returned: usize,
    },
}

impl core::fmt::Display for Unexecuted {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoResolution => formatter.write_str("the manifest holds no entry"),
            Self::NotText(component) => {
                write!(
                    formatter,
                    "the {component} is not text and cannot be recorded"
                )
            }
            Self::NoRevision => formatter.write_str("a pinned entry carries no revision"),
            Self::ValueOwnerUnresolved(value) => write!(
                formatter,
                "the value {value:?} resolves through no pinned entry of the reached manifest"
            ),
            Self::FigureUnrepresentable(dimension) => write!(
                formatter,
                "the {dimension} budget figure does not fit the record"
            ),
            Self::DispositionsNotPerRequest {
                requested,
                returned,
            } => write!(
                formatter,
                "{returned} dispositions were returned for {requested} requested claims"
            ),
        }
    }
}

impl std::error::Error for Unexecuted {}

fn text(bytes: &ExactBytes, component: &'static str) -> Result<String, Unexecuted> {
    String::from_utf8(bytes.as_bytes().to_vec()).map_err(|_| Unexecuted::NotText(component))
}

fn pinned(entry: &ManifestEntry) -> Result<PinnedEntry, Unexecuted> {
    Ok(PinnedEntry {
        owner: text(entry.key().owner(), "owner")?,
        contract_version: text(entry.key().contract_version(), "contract version")?,
        revision: text(entry.revision().ok_or(Unexecuted::NoRevision)?, "revision")?,
    })
}

/// The entries up to the first unresolved one are reached; that one is the
/// causing entry. With no unresolved entry the whole manifest is the state.
fn resolution(
    manifest: &VersionLockManifest,
) -> Result<(ResolutionState, Vec<&ManifestEntry>), Unexecuted> {
    if manifest.entries().is_empty() {
        return Err(Unexecuted::NoResolution);
    }
    let mut reached: Vec<&ManifestEntry> = Vec::new();
    for entry in manifest.entries() {
        if entry.status() == EntryStatus::Unresolved {
            let causing_entry = CausingEntry {
                owner: text(entry.key().owner(), "owner")?,
                contract_version: text(entry.key().contract_version(), "contract version")?,
                reason: entry
                    .unresolved_reason()
                    .map_or_else(|| Ok(String::new()), |r| text(r, "reason"))?,
            };
            let held = reached
                .iter()
                .map(|e| pinned(e))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok((
                ResolutionState::Partial {
                    reached: held,
                    causing_entry,
                },
                reached,
            ));
        }
        reached.push(entry);
    }
    let entries = reached
        .iter()
        .map(|e| pinned(e))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((ResolutionState::Full { entries }, reached))
}

fn truth_label(truth: Truth) -> &'static str {
    match truth {
        Truth::Satisfied => TruthDisposition::Satisfied,
        Truth::Violated => TruthDisposition::Violated,
        Truth::Pending => TruthDisposition::Pending,
        Truth::Unavailable => TruthDisposition::Unavailable,
    }
    .label()
}

/// The claim an identity is composed for: its position in the request list
/// (FR-029 returns dispositions in request order) and the FR-290 kind label of
/// the request.
#[derive(Clone, Copy)]
struct Claim {
    position: usize,
    kind: &'static str,
}

/// One row: the claim's position and kind, and its three independent axes.
fn row(
    claim: Claim,
    execution: AssessmentExecution,
    truth: Truth,
    basis: SettlementBasis,
) -> ClaimOutcome {
    ClaimOutcome::Row {
        identity: format!(
            "{}/{}:{}:{}:{}",
            claim.position,
            claim.kind,
            execution.label(),
            truth_label(truth),
            basis.as_str()
        ),
    }
}

fn refusal(claim: Claim, cause: &str) -> ClaimOutcome {
    ClaimOutcome::Refusal {
        identity: format!(
            "{}/{}:{}:{cause}",
            claim.position,
            claim.kind,
            AssessmentExecution::Refused.label()
        ),
    }
}

fn catalog(code: &CatalogCode) -> String {
    format!("{}.{}", code.code(), code.cause())
}

fn protocol_claim<R: BackendResult, B>(
    claim: Claim,
    outcome: &ProtocolOutcome<R, B>,
) -> ClaimOutcome {
    let unrun = |execution| {
        row(
            claim,
            execution,
            Truth::Unavailable,
            SettlementBasis::Unavailable,
        )
    };
    match outcome {
        ProtocolOutcome::Settled(ClaimSettlement::Result { result, .. }) => row(
            claim,
            AssessmentExecution::Completed,
            decided_truth(result),
            result.basis(),
        ),
        ProtocolOutcome::Settled(ClaimSettlement::Disposition(disposition)) => match disposition {
            Disposition::Unsupported(_) => unrun(AssessmentExecution::Unsupported),
            Disposition::Failed(_) => unrun(AssessmentExecution::Failed),
            Disposition::Refused(d) => refusal(claim, &catalog(&d.code)),
            Disposition::RequiresBound(_) => refusal(claim, "requires-bound"),
        },
        ProtocolOutcome::InvalidRequest { code, .. } => refusal(claim, &catalog(code)),
        ProtocolOutcome::Refused(refused) => refusal(claim, refused.code()),
    }
}

/// The FR-290 label of the claim a request asks for.
fn kind_of<G, A, T>(request: &ClaimRequest<G, A, T>) -> &'static str {
    match request {
        ClaimRequest::GlobalConformance(_) => Capability::GlobalConformance,
        ClaimRequest::Protocol(request) => request.kind.capability(),
        ClaimRequest::Composition(_) => Capability::Composition,
        ClaimRequest::Refinement(_) => Capability::Refinement,
    }
    .to_wire()
}

fn claim<G, A, T, P>(claim: Claim, disposition: &ClaimDisposition<G, A, T, P>) -> ClaimOutcome
where
    G: BackendResult,
    P: BackendResult,
{
    match disposition {
        ClaimDisposition::GlobalConformance(result) => row(
            claim,
            AssessmentExecution::Completed,
            decided_truth(result),
            result.basis(),
        ),
        ClaimDisposition::Protocol(outcome) => protocol_claim(claim, outcome),
        ClaimDisposition::Composition(CompositionOutcome::Decided(c)) => {
            row(claim, c.execution(), c.truth(), c.basis())
        }
        ClaimDisposition::Composition(CompositionOutcome::Refused(r)) => refusal(claim, r.code()),
        ClaimDisposition::Refinement(RefinementOutcome::Decided(r)) => {
            row(claim, r.execution(), r.truth(), r.basis())
        }
        ClaimDisposition::Refinement(RefinementOutcome::Refused(r)) => refusal(claim, r.code()),
    }
}

fn outcome<G, A, T, P>(
    requests: &[ClaimRequest<G, A, T>],
    operation: &OperationOutcome<Vec<ClaimDisposition<G, A, T, P>>>,
) -> Result<TypedOutcome, Unexecuted>
where
    G: BackendResult,
    P: BackendResult,
{
    Ok(match operation {
        OperationOutcome::Completed(dispositions) => {
            if dispositions.len() != requests.len() {
                return Err(Unexecuted::DispositionsNotPerRequest {
                    requested: requests.len(),
                    returned: dispositions.len(),
                });
            }
            TypedOutcome {
                operation: AssessmentExecution::Completed.label().to_owned(),
                claims: requests
                    .iter()
                    .zip(dispositions)
                    .enumerate()
                    .map(|(position, (request, disposition))| {
                        claim(
                            Claim {
                                position,
                                kind: kind_of(request),
                            },
                            disposition,
                        )
                    })
                    .collect(),
            }
        }
        OperationOutcome::Refused(refusal) => TypedOutcome {
            operation: String::from_utf8(refusal.axis().as_bytes().to_vec())
                .map_err(|_| Unexecuted::NotText("refusal axis"))?,
            claims: Vec::new(),
        },
        OperationOutcome::ResourceIncomplete(incomplete) => TypedOutcome {
            operation: format!(
                "{}:{}:{}",
                incomplete.code().as_str(),
                incomplete.dimension().as_str(),
                incomplete.site().as_str()
            ),
            claims: Vec::new(),
        },
    })
}

fn budget(budget: &Budget, usage: Usage) -> Result<Vec<BudgetFigure>, Unexecuted> {
    BudgetDimension::ALL
        .iter()
        .map(|&dimension| {
            let figure = |n: usize| {
                u64::try_from(n).map_err(|_| Unexecuted::FigureUnrepresentable(dimension.as_str()))
            };
            Ok(BudgetFigure {
                dimension: dimension.as_str().to_owned(),
                granted: budget.grant_for(dimension).map(figure).transpose()?,
                consumed: figure(usage.consumed(dimension))?,
            })
        })
        .collect()
}

/// Compose the record of `scenario` from the live `operation` outcome and the
/// dispositions of the claims it returned, one per entry of `requests` in
/// request order (FR-029, FR-206).
///
/// A claim is identified in the record by its position in `requests`, the
/// FR-290 label of its kind and its axes. That composition is this library's
/// interim one: no owner publishes a canonical claim-row identity (ADR-010 G12).
///
/// # Errors
///
/// [`Unexecuted`] when a component cannot be retained: the scenario is then
/// reported unexecuted, never as passing.
pub fn compose<G, A, T, P>(
    scenario: &Scenario<'_>,
    requests: &[ClaimRequest<G, A, T>],
    operation: &OperationOutcome<Vec<ClaimDisposition<G, A, T, P>>>,
) -> Result<ScenarioEvidence, Unexecuted>
where
    G: BackendResult,
    P: BackendResult,
{
    let (resolution, reached) = resolution(scenario.manifest)?;
    let value_owners = scenario
        .value_owners
        .iter()
        .map(|(value, key)| {
            let entry = reached
                .iter()
                .find(|entry| entry.key() == *key && entry.status() == EntryStatus::Pinned)
                .ok_or_else(|| Unexecuted::ValueOwnerUnresolved((*value).to_owned()))?;
            let PinnedEntry {
                owner,
                contract_version,
                revision,
            } = pinned(entry)?;
            Ok(ValueOwner {
                value: (*value).to_owned(),
                owner,
                contract_version,
                revision,
            })
        })
        .collect::<Result<_, Unexecuted>>()?;
    Ok(ScenarioEvidence {
        contract: INTAKE_CONTRACT.to_owned(),
        integration: scenario.integration,
        scenario: scenario.scenario.to_owned(),
        resolution,
        value_owners,
        outcome: outcome(requests, operation)?,
        budget: budget(scenario.budget, scenario.usage)?,
        trace_binding: TraceBinding {
            criterion: scenario.criterion.to_owned(),
        },
    })
}
