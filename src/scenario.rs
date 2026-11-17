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

use fsl_replay::{SettlementBasis, Truth};
use quoin_evidence::scenario::{
    BudgetFigure, CausingEntry, ClaimOutcome, Integration, PinnedEntry, ResolutionState,
    ScenarioEvidence, TraceBinding, TypedOutcome, ValueOwner, INTAKE_CONTRACT,
};

use crate::bounds::inventory::{Budget, BudgetDimension, Usage};
use crate::closure::{AssessmentExecution, TruthDisposition};
use crate::composition::CompositionOutcome;
use crate::dispatch::{BackendResult, ClaimDisposition, ProtocolOutcome};
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
    /// A budget grant does not fit the record's figure.
    FigureUnrepresentable(&'static str),
}

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

/// One row: the claim kind and its three independent axes.
fn row(
    kind: &str,
    execution: AssessmentExecution,
    truth: Truth,
    basis: SettlementBasis,
) -> ClaimOutcome {
    ClaimOutcome::Row {
        identity: format!(
            "{kind}:{}:{}:{}",
            execution.label(),
            truth_label(truth),
            basis.as_str()
        ),
    }
}

fn refusal(kind: &str, cause: &str) -> ClaimOutcome {
    ClaimOutcome::Refusal {
        identity: format!("{kind}:{}:{cause}", AssessmentExecution::Refused.label()),
    }
}

/// A refusal's variant name, which carries the cause without its payload.
fn variant(debug: &impl core::fmt::Debug) -> String {
    format!("{debug:?}")
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn protocol_claim<R: BackendResult, B: ClaimKind>(outcome: &ProtocolOutcome<R, B>) -> ClaimOutcome {
    match outcome {
        ProtocolOutcome::Settled(ClaimSettlement::Result { result, premises }) => row(
            premises.kind(),
            AssessmentExecution::Completed,
            outcome.truth(),
            result.basis(),
        ),
        ProtocolOutcome::Settled(ClaimSettlement::Disposition(disposition)) => match disposition {
            Disposition::Unsupported(d) => row(
                d.premises.kind(),
                AssessmentExecution::Unsupported,
                Truth::Unavailable,
                SettlementBasis::Unavailable,
            ),
            Disposition::Failed(d) => row(
                d.premises.kind(),
                AssessmentExecution::Failed,
                Truth::Unavailable,
                SettlementBasis::Unavailable,
            ),
            Disposition::Refused(d) => refusal(
                d.premises.kind(),
                &format!("{}.{}", d.code.code(), d.code.cause()),
            ),
            Disposition::RequiresBound(d) => refusal(d.premises.kind(), "requires-bound"),
        },
        ProtocolOutcome::InvalidRequest { code, binding, .. } => {
            refusal(binding.kind(), &format!("{}.{}", code.code(), code.cause()))
        }
        ProtocolOutcome::Refused(refused) => refusal("protocol", &variant(refused)),
    }
}

/// The FR-290 label of the claim a binding was requested for.
pub trait ClaimKind {
    /// The label.
    fn kind(&self) -> &'static str;
}

impl<A> ClaimKind for crate::dispatch::Binding<A> {
    fn kind(&self) -> &'static str {
        self.request.kind.capability().to_wire()
    }
}

fn claim<G, A, T, P>(
    disposition: &ClaimDisposition<G, A, T, P>,
    global: &impl Fn(&G) -> (Truth, SettlementBasis),
) -> ClaimOutcome
where
    P: BackendResult,
{
    match disposition {
        ClaimDisposition::GlobalConformance(result) => {
            let (truth, basis) = global(result);
            row(
                fsl_semantics::check::Capability::GlobalConformance.to_wire(),
                AssessmentExecution::Completed,
                truth,
                basis,
            )
        }
        ClaimDisposition::Protocol(outcome) => protocol_claim(outcome),
        ClaimDisposition::Composition(CompositionOutcome::Decided(c)) => row(
            fsl_semantics::check::Capability::Composition.to_wire(),
            c.execution(),
            c.truth(),
            c.basis(),
        ),
        ClaimDisposition::Composition(CompositionOutcome::Refused(r)) => refusal(
            fsl_semantics::check::Capability::Composition.to_wire(),
            &variant(r),
        ),
        ClaimDisposition::Refinement(RefinementOutcome::Decided(r)) => row(
            fsl_semantics::check::Capability::Refinement.to_wire(),
            r.execution(),
            r.truth(),
            r.basis(),
        ),
        ClaimDisposition::Refinement(RefinementOutcome::Refused(r)) => refusal(
            fsl_semantics::check::Capability::Refinement.to_wire(),
            &variant(r),
        ),
    }
}

fn outcome<G, A, T, P>(
    operation: &OperationOutcome<Vec<ClaimDisposition<G, A, T, P>>>,
    global: &impl Fn(&G) -> (Truth, SettlementBasis),
) -> Result<TypedOutcome, Unexecuted>
where
    P: BackendResult,
{
    Ok(match operation {
        OperationOutcome::Completed(dispositions) => TypedOutcome {
            operation: AssessmentExecution::Completed.label().to_owned(),
            claims: dispositions.iter().map(|d| claim(d, global)).collect(),
        },
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
/// dispositions of the claims it returned (FR-206).
///
/// # Errors
///
/// [`Unexecuted`] when a component cannot be retained: the scenario is then
/// reported unexecuted, never as passing.
///
/// `global` reads the truth and settlement basis of a passed-through
/// `global-conformance` result, whose type the caller owns.
pub fn compose<G, A, T, P>(
    scenario: &Scenario<'_>,
    operation: &OperationOutcome<Vec<ClaimDisposition<G, A, T, P>>>,
    global: impl Fn(&G) -> (Truth, SettlementBasis),
) -> Result<ScenarioEvidence, Unexecuted>
where
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
        outcome: outcome(operation, &global)?,
        budget: budget(scenario.budget, scenario.usage)?,
        trace_binding: TraceBinding {
            criterion: scenario.criterion.to_owned(),
        },
    })
}
