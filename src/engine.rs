// SPDX-License-Identifier: AGPL-3.0-or-later
//! FR-150, FR-152, FR-153, FR-302, FR-303: the provider that runs FSL's own
//! `protocol-analysis` engine behind the dispatch boundary.
//!
//! The engine, its descriptor and its result are FSL's (ADR-036). This module
//! adds no decision procedure: it builds the engine's `analyze` item from the
//! request the caller supplies, runs it on FSL's linked engines, and returns
//! the engine's record unchanged, so the dispatcher carries the witness as the
//! engine wrote it. A report FSL gives no result for stays the typed
//! `unsupported` path of FR-153. A supplied finite trace (FR-303) is replayed
//! by FSL's `finite-replay` item through FSL's `analyze` entry; this module
//! replays no step.

use filament_contract_model::CheckedPackageV2;
use filament_exact::Cancel;
use filament_exact::NodeKey;
use filament_semantic_value::catalog::CatalogCode;
use fsl_analyze::analyze::{
    analyze, analyze_items, AnalyzeLimits, AnalyzeRecord, AnalyzeRequest, Disposed,
    DispositionKind, Engines, Settlement,
};
use fsl_analyze::protocol_analysis::manifest::descriptor;
use fsl_analyze::protocol_analysis::{
    projection_item, realizability_item, ProjectionRequest, ProtocolAnalysisArtifact,
    RealizabilityRequest,
};
use fsl_foundation::RequestIndex;
use fsl_package::CheckedPackage;
use fsl_replay::protocol_analysis::{
    AnalysisOutcome, ProtocolAnalysisSettlement, RealizabilityCounterexample,
};
use fsl_replay::spine::ProtocolAnalysisRecord;
use fsl_replay::{
    InconclusiveCause, SeparatingWitnessRecord, SettlementBasis, TerminalRecord, TerminalValue,
    Truth,
};
use fsl_route::{BackendDescriptor, ProviderOrigin, RegistrationRefusal};
use fsl_semantics::check::wire_id::WireGraph;
use fsl_semantics::check::Capability;

use crate::dispatch::{replay_settlement, BackendResult, Binding, ProtocolClaimKind};
use crate::disposition::{FailureCause, FslBoundary, FslReport, UnsupportedCause};

/// The request the caller states for one claim, in the engine's own request
/// type. The key members and the run budget are the caller's to state; FSL
/// refuses an absent member (FR-572).
pub enum EngineRequest<'p> {
    /// A `local-projection` request.
    Projection(Box<ProjectionRequest>),
    /// A `realizability` request.
    Realizability(Box<RealizabilityRequest<'p>>),
    /// A `finite-replay` request carrying a supplied finite trace (FR-303).
    FiniteReplay(Box<FiniteReplayRequest<'p>>),
}

/// The request for a `finite-replay` claim, in FSL's own types: the arguments
/// of FSL's `analyze` entry (FSL FR-621, FR-636). The one item of `request` is
/// a `finite-replay` item whose `trace` names the supplied trace document, and
/// that document's bytes are in `request.binding.snapshots` under that
/// reference. The caller states the item and its disposition; FSL reads the
/// trace and replays it.
pub struct FiniteReplayRequest<'p> {
    /// The checked package the item's claim is in.
    pub package: &'p CheckedPackage,
    /// FSL's analyze request: the subject binding and the one item.
    pub request: AnalyzeRequest<'p>,
    /// FSL's run limits.
    pub limits: AnalyzeLimits,
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

/// The result of a run of FSL's protocol analysis engine: for
/// `local-projection` and `realizability`, FSL's `analyze` record, unchanged,
/// and the FR-573 settlement read from its artifact; for `finite-replay`,
/// FSL's FR-331 terminal record of the replay, unchanged (FR-303).
#[derive(Debug)]
pub struct EngineResult {
    body: Body,
}

#[derive(Debug)]
enum Body {
    Analysis(Box<Analysis>),
    Replay(Box<TerminalRecord>),
}

#[derive(Debug)]
struct Analysis {
    record: AnalyzeRecord,
    settlement: ProtocolAnalysisSettlement,
    run_record: ProtocolAnalysisRecord,
}

impl EngineResult {
    fn of(record: AnalyzeRecord) -> Result<Self, FailureCause> {
        let artifact = record
            .protocol_analysis
            .as_deref()
            .ok_or(FailureCause::NoArtifact)?;
        let settlement = artifact.settlement();
        // FSL writes the record for every result it settles; an engine that
        // cannot write it has failed, which is not an absent record.
        let run_record = artifact
            .record()
            .map_err(|error| FailureCause::RunRecordRefused(error.to_string()))?;
        Ok(Self {
            body: Body::Analysis(Box::new(Analysis {
                record,
                settlement,
                run_record,
            })),
        })
    }

    /// FSL's `analyze` record of a `local-projection` or `realizability`
    /// result, as the engine returned it; absent for a replay.
    #[must_use]
    pub fn record(&self) -> Option<&AnalyzeRecord> {
        match &self.body {
            Body::Analysis(analysis) => Some(&analysis.record),
            Body::Replay(_) => None,
        }
    }

    /// FSL's result artifact: the key and the outcome; absent for a replay.
    #[must_use]
    pub fn artifact(&self) -> Option<&ProtocolAnalysisArtifact> {
        self.record()?.protocol_analysis.as_deref()
    }

    /// FSL's FR-573 settlement of a `local-projection` or `realizability`
    /// result; absent for a replay.
    #[must_use]
    pub fn settlement(&self) -> Option<&ProtocolAnalysisSettlement> {
        match &self.body {
            Body::Analysis(analysis) => Some(&analysis.settlement),
            Body::Replay(_) => None,
        }
    }

    /// FSL's FR-331 terminal record of a `finite-replay` result, as the engine
    /// returned it (FR-303); absent for the other kinds.
    #[must_use]
    pub fn replay_record(&self) -> Option<&TerminalRecord> {
        match &self.body {
            Body::Replay(record) => Some(record),
            Body::Analysis(_) => None,
        }
    }
}

impl BackendResult for EngineResult {
    /// A replay's truth and basis are read from FSL's outcome axes of its
    /// terminal record (FR-303).
    fn truth(&self) -> Truth {
        match &self.body {
            Body::Analysis(analysis) => analysis.settlement.truth,
            Body::Replay(record) => replay_settlement(record.value()).0,
        }
    }

    fn basis(&self) -> SettlementBasis {
        match &self.body {
            Body::Analysis(analysis) => analysis.settlement.basis,
            Body::Replay(record) => replay_settlement(record.value()).1,
        }
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
        match &self.body {
            Body::Analysis(analysis) => Some(&analysis.run_record),
            Body::Replay(_) => None,
        }
    }

    fn terminal_record(&self) -> Option<&TerminalRecord> {
        self.replay_record()
    }

    fn inconclusive(&self) -> Option<&InconclusiveCause> {
        match &self.settlement()?.value {
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

/// The cause of a failure FSL disposed of with `kind`: its diagnostic code when
/// it wrote one, otherwise the disposition itself.
fn disposed_failure(kind: DispositionKind, code: Option<CatalogCode>) -> FailureCause {
    code.map_or(
        FailureCause::DisposedWithoutDiagnostic(kind),
        FailureCause::Diagnostic,
    )
}

/// The cause of a failure FSL settled with `value`: its diagnostic code when it
/// wrote one, otherwise the settled value itself.
fn settled_failure(value: &TerminalValue, code: Option<CatalogCode>) -> FailureCause {
    code.map_or_else(
        || FailureCause::SettledWithoutDiagnostic(Box::new(value.clone())),
        FailureCause::Diagnostic,
    )
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
        Settlement::Disposed(Disposed {
            kind, diagnostic, ..
        }) => FslReport::Failed(disposed_failure(
            *kind,
            diagnostic.as_ref().map(|found| found.code),
        )),
        Settlement::Terminal(terminal) => {
            let code = terminal.diagnostic();
            match terminal.value() {
                TerminalValue::Declined { .. } => code.map_or_else(
                    || FslReport::Failed(settled_failure(terminal.value(), None)),
                    FslReport::Refusal,
                ),
                value => FslReport::Failed(settled_failure(value, code)),
            }
        }
    }
}

/// FSL's refusal of an item whose kind does not apply (FSL FR-507-AC-7). The
/// code is spelled as FSL's spec spells it; its owner is to publish it
/// (IR-1513).
const INAPPLICABLE_KIND: CatalogCode = CatalogCode::new("invalid_capability", "inapplicable-kind");

impl<S> FslEngine<'_, S> {
    /// Whether the replay item's claim is one of the dispatched claim's source
    /// identities, read in the package this engine is bound to.
    fn is_the_bound_claim<A>(&self, binding: &Binding<A>, claim: &NodeKey) -> bool {
        let Ok(graph) = WireGraph::new(
            self.package
                .resolved_graph()
                .nodes
                .iter()
                .map(|n| &n.node_id),
        ) else {
            return false;
        };
        binding
            .request
            .sources
            .iter()
            .any(|source| graph.key(source).is_ok_and(|key| key == *claim))
    }

    /// Run FSL's `finite-replay` item (FSL FR-636) and read what it returned:
    /// a `proved` (or `refuted`) record is the result, a `declined` one FSL's
    /// refusal with its record, an `unsupported` one the typed unsupported
    /// path, and any other value a failure carrying FSL's diagnostic.
    fn replay<A>(
        &self,
        binding: &Binding<A>,
        request: &FiniteReplayRequest<'_>,
    ) -> FslReport<EngineResult> {
        let [negotiated] = request.request.items.as_slice() else {
            return FslReport::Refusal(INAPPLICABLE_KIND);
        };
        if negotiated.item.kind != Capability::FiniteReplay {
            return FslReport::Refusal(INAPPLICABLE_KIND);
        }
        if !self.is_the_bound_claim(binding, negotiated.item.claim.node()) {
            return FslReport::Failed(FailureCause::ReplayNotTheBoundClaim);
        }
        let outcome = match analyze(
            request.package,
            &request.request,
            &self.engines,
            &request.limits,
            &self.cancel,
        ) {
            Ok(staged) => staged.into_value(),
            Err(failure) => {
                return FslReport::Failed(match failure.into_refused() {
                    Ok(refusal) => FailureCause::AnalyzeRefused(refusal.to_string()),
                    Err(stopped) => FailureCause::AnalyzeStopped(stopped),
                })
            }
        };
        let Some(record) = outcome.records.into_iter().next() else {
            return FslReport::Failed(FailureCause::NoRecord);
        };
        match record.value().clone() {
            TerminalValue::Proved { .. } | TerminalValue::Refuted => {
                FslReport::Result(EngineResult {
                    body: Body::Replay(Box::new(record)),
                })
            }
            TerminalValue::Declined { .. } => FslReport::Declined(Box::new(record)),
            // A requires-bound disposition reaches here too: a terminal
            // record holds no domain, so none can be rebuilt from it.
            TerminalValue::Unsupported(_) => FslReport::Unsupported {
                cause: record.diagnostic().map_or_else(
                    UnsupportedCause::requested_capability,
                    UnsupportedCause::RequestedCapability,
                ),
                construct: None,
            },
            value => FslReport::Failed(settled_failure(&value, record.diagnostic())),
        }
    }
}

impl<'p, A, S> FslBoundary<Binding<A>, EngineResult> for FslEngine<'p, S>
where
    S: FnMut(&Binding<A>) -> EngineRequest<'p>,
{
    fn report(&mut self, binding: &Binding<A>) -> FslReport<EngineResult> {
        let index = RequestIndex::new(0);
        // The request the caller supplies must be the claim's own kind.
        let item = match ((self.supply)(binding), binding.request.kind) {
            (EngineRequest::FiniteReplay(request), ProtocolClaimKind::FiniteReplay) => {
                return self.replay(binding, &request);
            }
            (EngineRequest::Projection(request), ProtocolClaimKind::LocalProjection) => {
                projection_item(index, self.package, *request)
            }
            (EngineRequest::Realizability(request), ProtocolClaimKind::Realizability) => {
                realizability_item(index, self.package, *request)
            }
            // The supplied request is another kind than the claim's.
            _ => return FslReport::Refusal(INAPPLICABLE_KIND),
        };
        let mut records = analyze_items(&self.engines, vec![item], &self.cancel);
        let Some(record) = records.pop() else {
            return FslReport::Failed(FailureCause::NoRecord);
        };
        if record.protocol_analysis.is_none() {
            return without_result(&record);
        }
        EngineResult::of(record).map_or_else(FslReport::Failed, FslReport::Result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ix_trace_rs::trace;

    const CODE: CatalogCode = CatalogCode::new("invalid_runtime_input", "invalid-value");

    #[trace("TC-164", "FR-150-AC-20")]
    #[test]
    fn a_disposition_with_no_diagnostic_fails_with_the_disposition_as_its_cause() {
        assert_eq!(
            disposed_failure(DispositionKind::RequiresBound, None),
            FailureCause::DisposedWithoutDiagnostic(DispositionKind::RequiresBound)
        );
        assert_eq!(
            disposed_failure(DispositionKind::RequiresBound, Some(CODE)),
            FailureCause::Diagnostic(CODE)
        );
    }

    #[trace("TC-164", "FR-150-AC-20")]
    #[test]
    fn a_refuted_record_with_no_diagnostic_fails_with_the_settled_value_as_its_cause() {
        assert_eq!(
            settled_failure(&TerminalValue::Refuted, None),
            FailureCause::SettledWithoutDiagnostic(Box::new(TerminalValue::Refuted))
        );
        assert_eq!(
            settled_failure(&TerminalValue::Refuted, Some(CODE)),
            FailureCause::Diagnostic(CODE)
        );
    }
}
