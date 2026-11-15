// SPDX-License-Identifier: AGPL-3.0-or-later
//! FR-150, FR-152, FR-153, FR-302: the provider that runs FSL's own
//! `protocol-analysis` engine behind the dispatch boundary.
//!
//! The engine, its descriptor and its result are FSL's (ADR-036). This module
//! adds no decision procedure: it builds the engine's `analyze` item from the
//! request the caller supplies, runs it on FSL's linked engines, and returns
//! the engine's record unchanged, so the dispatcher carries the witness as the
//! engine wrote it. A report FSL gives no result for stays the typed
//! `unsupported` path of FR-153.

use filament_contract_model::CheckedPackageV2;
use filament_exact::Cancel;
use fsl_analyze::analyze::{
    analyze_items, AnalyzeRecord, Disposed, DispositionKind, Engines, Settlement,
};
use fsl_analyze::protocol_analysis::manifest::descriptor;
use fsl_analyze::protocol_analysis::{
    projection_item, realizability_item, ProjectionRequest, ProtocolAnalysisArtifact,
    RealizabilityRequest,
};
use fsl_foundation::RequestIndex;
use fsl_replay::protocol_analysis::{
    AnalysisOutcome, ProtocolAnalysisSettlement, RealizabilityCounterexample,
};
use fsl_replay::spine::ProtocolAnalysisRecord;
use fsl_replay::{
    InconclusiveCause, SeparatingWitnessRecord, SettlementBasis, TerminalValue, Truth,
};
use fsl_route::{BackendDescriptor, ProviderOrigin, RegistrationRefusal};

use crate::dispatch::{Answer, BackendResult, Binding, Claim, ProtocolClaimKind};
use crate::disposition::{FslBoundary, FslReport, UnsupportedCause};

/// The request the caller states for one claim, in the engine's own request
/// type. The key members and the run budget are the caller's to state; FSL
/// refuses an absent member (FR-572).
pub enum EngineRequest<'p> {
    /// A `local-projection` request.
    Projection(Box<ProjectionRequest>),
    /// A `realizability` request.
    Realizability(Box<RealizabilityRequest<'p>>),
}

/// FSL's engine descriptor, to register at the
/// [`ProviderBoundary`](crate::dispatch::ProviderBoundary) beside
/// [`FslEngine`].
///
/// # Errors
///
/// FSL's registry refusal of the engine's own manifest.
pub fn engine_descriptor(origin: ProviderOrigin) -> Result<BackendDescriptor, RegistrationRefusal> {
    descriptor(origin)
}

/// The result of a run of FSL's protocol analysis engine: FSL's `analyze`
/// record, unchanged, and the FR-573 settlement read from its artifact.
#[derive(Debug)]
pub struct EngineResult {
    record: Box<AnalyzeRecord>,
    settlement: ProtocolAnalysisSettlement,
    run_record: ProtocolAnalysisRecord,
}

impl EngineResult {
    fn of(record: AnalyzeRecord) -> Option<Self> {
        let artifact = record.protocol_analysis.as_deref()?;
        let settlement = artifact.settlement();
        // FSL writes the record for every result it settles; an engine that
        // cannot write it has failed, which is not an absent record.
        let run_record = artifact.record().ok()?;
        Some(Self {
            record: Box::new(record),
            settlement,
            run_record,
        })
    }

    /// FSL's record, as the engine returned it.
    #[must_use]
    pub fn record(&self) -> &AnalyzeRecord {
        &self.record
    }

    /// FSL's result artifact: the key and the outcome.
    #[must_use]
    pub fn artifact(&self) -> Option<&ProtocolAnalysisArtifact> {
        self.record.protocol_analysis.as_deref()
    }

    /// FSL's `native-run-result/2` protocol-analysis record of the result
    /// (FSpec FR-352), as the engine wrote it.
    #[must_use]
    pub const fn run_record(&self) -> &ProtocolAnalysisRecord {
        &self.run_record
    }

    /// FSL's FR-573 settlement of the result.
    #[must_use]
    pub const fn settlement(&self) -> &ProtocolAnalysisSettlement {
        &self.settlement
    }
}

impl BackendResult for EngineResult {
    fn truth(&self) -> Truth {
        self.settlement.truth
    }

    fn basis(&self) -> SettlementBasis {
        self.settlement.basis
    }

    fn witness(&self) -> Option<&SeparatingWitnessRecord> {
        match self.artifact()? {
            ProtocolAnalysisArtifact::Projection(result) => match &result.outcome {
                AnalysisOutcome::Refuted(found) => Some(&found.separating_witness),
                _ => None,
            },
            ProtocolAnalysisArtifact::Realizability(result) => match &result.outcome {
                AnalysisOutcome::Refuted(
                    RealizabilityCounterexample::IndistinguishableHistories {
                        separating_witness,
                        ..
                    }
                    | RealizabilityCounterexample::EnvironmentDefeat {
                        separating_witness, ..
                    },
                ) => Some(separating_witness),
                _ => None,
            },
        }
    }

    fn run_record(&self) -> Option<&ProtocolAnalysisRecord> {
        Some(&self.run_record)
    }

    fn inconclusive(&self) -> Option<&InconclusiveCause> {
        match &self.settlement.value {
            TerminalValue::Inconclusive(cause) => Some(cause),
            _ => None,
        }
    }
}

/// FSL's protocol analysis engine as a provider. `supply` states the engine's
/// request for each bound claim; the engine decides it.
pub struct FslEngine<'p, S> {
    engines: Engines,
    cancel: Cancel,
    package: &'p CheckedPackageV2,
    supply: S,
}

impl<'p, S> FslEngine<'p, S> {
    /// The provider over `package`, running FSL's linked engines.
    ///
    /// # Errors
    ///
    /// FSL's registry refusal of an engine's own manifest.
    pub fn new(package: &'p CheckedPackageV2, supply: S) -> Result<Self, RegistrationRefusal> {
        Ok(Self {
            engines: Engines::fsl()?,
            cancel: Cancel::new(),
            package,
            supply,
        })
    }
}

/// A report FSL gives no result for, with FSL's own diagnostic carried.
fn without_result<R>(record: &AnalyzeRecord) -> FslReport<R> {
    match &record.settlement {
        Settlement::Disposed(Disposed {
            kind: DispositionKind::Unsupported,
            diagnostic,
            ..
        }) => FslReport::Unsupported {
            cause: diagnostic
                .as_ref()
                .map_or_else(UnsupportedCause::requested_capability, |found| {
                    UnsupportedCause::RequestedCapability(found.code)
                }),
            construct: None,
        },
        // The request is refused for what the caller stated (an absent key
        // member, FR-572), not a failure of the engine.
        Settlement::Disposed(Disposed {
            kind: DispositionKind::InvalidRequest,
            diagnostic: Some(found),
            ..
        }) => FslReport::Refusal(found.code),
        // A `Disposed` record holds no domain, so a requires-bound outcome
        // cannot be rebuilt from it: it is reported as FSL's own diagnostic
        // on a failure, never as a result.
        Settlement::Disposed(Disposed { diagnostic, .. }) => {
            FslReport::Failed(diagnostic.as_ref().map(|found| found.code))
        }
        Settlement::Terminal(terminal) => match terminal.value() {
            TerminalValue::Declined { .. } => terminal
                .diagnostic()
                .map_or(FslReport::Failed(None), FslReport::Refusal),
            _ => FslReport::Failed(terminal.diagnostic()),
        },
    }
}

impl<'p, A, I, T, Q, S> FslBoundary<Claim<A, I, T>, Answer<EngineResult, Q>> for FslEngine<'p, S>
where
    S: FnMut(&Binding<A, I>) -> EngineRequest<'p>,
{
    fn report(&mut self, claim: &Claim<A, I, T>) -> FslReport<Answer<EngineResult, Q>> {
        let Claim::Protocol(binding) = claim else {
            // Composition and refinement are not this engine's kinds.
            return FslReport::Unsupported {
                cause: UnsupportedCause::requested_capability(),
                construct: None,
            };
        };
        let index = RequestIndex::new(0);
        // The request the caller supplies must be the claim's own kind.
        let item = match ((self.supply)(binding), binding.request.kind) {
            (EngineRequest::Projection(request), ProtocolClaimKind::LocalProjection) => {
                projection_item(index, self.package, *request)
            }
            (EngineRequest::Realizability(request), ProtocolClaimKind::Realizability) => {
                realizability_item(index, self.package, *request)
            }
            _ => return FslReport::Failed(None),
        };
        let mut records = analyze_items(&self.engines, vec![item], &self.cancel);
        let Some(record) = records.pop() else {
            return FslReport::Failed(None);
        };
        if record.protocol_analysis.is_none() {
            return without_result(&record);
        }
        EngineResult::of(record).map_or(FslReport::Failed(None), |result| {
            FslReport::Result(Answer::Protocol(result))
        })
    }
}
