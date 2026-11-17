// SPDX-License-Identifier: AGPL-3.0-or-later
//! FR-205: finite global conformance, decided by FSL's explicit-state protocol
//! checker (EN-1) behind the dispatch boundary.
//!
//! FSL owns the decision procedure and its settlement (FSL FR-215, FR-217,
//! FR-636 "Agreement"): `check_protocol` explores every behaviour of the
//! protocol subject, and `settle_protocol_check` settles the outcome by FSL's
//! one map, checking a proof's certificate and replaying a counterexample. This
//! module adds no search, no temporal evaluation and no refinement; it makes
//! the two calls, carries FSL's record unchanged, and reads its truth and
//! basis for the dispatcher.
//!
//! The subject, the item, the certificate request and the replay are the
//! caller's to state, as FSL's own [`ProtocolItem`] states them: FSL's replay of
//! a counterexample recompiles from the caller's source, so the caller owns it.
//! A counterexample whose replay the caller cannot supply is settled by FSL as
//! a refused replay, never as `refuted`.

use filament_exact::Cancel;
use filament_semantic_value::catalog::CatalogCode;
use fsl_analyze::analyze::ProtocolItem;
use fsl_analyze::model_check::manifest::descriptor;
use fsl_analyze::model_check::{check_protocol, ModelCheckRefusal};
use fsl_eval::simulation::protocol::ProtocolRefusal;
use fsl_eval::simulation::protocol::ProtocolTransition;
use fsl_foundation::diagnostic::Code;
use fsl_replay::spine::ProtocolAnalysisRecord;
use fsl_replay::{
    settle_protocol_check, InconclusiveCause, ModelCheckRecord, SeparatingWitnessRecord,
    SettlementBasis, TerminalValue, Truth,
};
use fsl_route::{BackendDescriptor, ProviderOrigin, RegistrationRefusal};

use crate::dispatch::{BackendResult, Binding};
use crate::disposition::{FailureCause, FslBoundary, FslReport};

/// FSL's settled record of one protocol check, unchanged.
#[derive(Clone, Debug)]
pub struct ConformanceResult {
    record: ModelCheckRecord<ProtocolTransition>,
}

impl ConformanceResult {
    /// FSL's record: terminal value, label, basis, truth, the limits the run
    /// used, and the counterexample FSL's replay reproduced.
    #[must_use]
    pub const fn record(&self) -> &ModelCheckRecord<ProtocolTransition> {
        &self.record
    }
}

impl BackendResult for ConformanceResult {
    fn truth(&self) -> Truth {
        self.record.truth
    }

    fn basis(&self) -> SettlementBasis {
        self.record.basis
    }

    /// EN-1's counterexample is its own protocol trace (FSpec FR-433), not a
    /// separating witness (FSpec FR-351), so the result carries none; the trace
    /// stays on [`ConformanceResult::record`].
    fn witness(&self) -> Option<&SeparatingWitnessRecord> {
        None
    }

    /// EN-1 writes no `native-run-result/2` protocol-analysis record.
    fn run_record(&self) -> Option<&ProtocolAnalysisRecord> {
        None
    }

    fn inconclusive(&self) -> Option<&InconclusiveCause> {
        match &self.record.value {
            TerminalValue::Inconclusive(cause) => Some(cause),
            _ => None,
        }
    }
}

/// EN-1's descriptor, to register at the
/// [`ProviderBoundary`](crate::dispatch::ProviderBoundary) beside
/// [`FslProtocolCheck`]. It advertises `temporal-satisfaction`; this library
/// adds no capability to it.
///
/// # Errors
///
/// FSL's registry refusal of the engine's own manifest.
pub fn engine_descriptor(origin: ProviderOrigin) -> Result<BackendDescriptor, RegistrationRefusal> {
    descriptor(origin)
}

/// FSL's protocol checker as a provider. `supply` states FSL's item for each
/// bound claim; EN-1 decides it.
pub struct FslProtocolCheck<S> {
    cancel: Cancel,
    supply: S,
}

impl<S> FslProtocolCheck<S> {
    /// The provider that asks `supply` for the item of each claim.
    #[must_use]
    pub fn new(supply: S) -> Self {
        Self {
            cancel: Cancel::new(),
            supply,
        }
    }
}

impl<'p, A, S> FslBoundary<Binding<A>, ConformanceResult> for FslProtocolCheck<S>
where
    S: FnMut(&Binding<A>) -> ProtocolItem<'p>,
{
    fn report(&mut self, binding: &Binding<A>) -> FslReport<ConformanceResult> {
        let ProtocolItem {
            request,
            certificate,
            replay,
        } = (self.supply)(binding);
        let limits = request.limits;
        // FSL's requires-bound refusal is a public typed value (FSL FR-336):
        // it is carried as the typed outcome FR-058 specifies. Every other
        // refusal is classified by FSL's FR-281 table in a crate-private map
        // (ADR-010 G9); this library does not restate it, so it is carried as
        // the checker's own text.
        match check_protocol(request, &self.cancel) {
            Ok(outcome) => FslReport::Result(ConformanceResult {
                record: settle_protocol_check(
                    &outcome,
                    &limits,
                    &certificate,
                    &self.cancel,
                    replay,
                ),
            }),
            Err(ModelCheckRefusal::RequiresBound(outcome)) => FslReport::RequiresBound {
                outcome,
                construct: None,
            },
            // FSpec FR-785: a symbolic capacity the subject's binding leaves
            // unbound is requires-bound, FSL's list carried unchanged.
            Err(ModelCheckRefusal::UnboundCapacity(capacities)) => FslReport::UnboundCapacity {
                capacities,
                construct: None,
            },
            // A binding defect is FSL's `invalid_runtime_input` refusal; FSL
            // publishes its code and cause on the refusal itself.
            Err(ModelCheckRefusal::Protocol(ProtocolRefusal::Capacity(refusal))) => {
                FslReport::Refusal(CatalogCode::new(
                    Code::as_str(refusal.code()),
                    refusal.cause(),
                ))
            }
            Err(refusal) => FslReport::Failed(FailureCause::CheckerRefused(refusal.to_string())),
        }
    }
}
