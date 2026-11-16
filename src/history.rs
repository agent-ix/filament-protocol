// SPDX-License-Identifier: AGPL-3.0-or-later
//! FR-208 to FR-211: finite global conformance of one admitted observation
//! history, decided here over the live assessment axes of [`crate::dynamics`]
//! and the closure algebra of [`crate::closure`].
//!
//! FSL has no engine for these questions (it explores a protocol, ADR-010), so
//! they are this library's, as composition and refinement are. Nothing here
//! steps a protocol, evaluates a temporal clause or repairs a history. A
//! history is judged on the four dynamic axes (roles, channels, joins, loops)
//! by the assessments of [`crate::dynamics`]; this module only reads their
//! results together:
//!
//! - each axis keeps its own verdict and an adverse observation names the axis
//!   (FR-209);
//! - the facts are read in the history's own order, so a presentation order
//!   changes nothing (FR-208);
//! - a history assessed whole and a history assessed prefix by prefix give the
//!   same results, and a divergence is a typed refusal (FR-211);
//! - complete global success needs conformant axes *and* the five closure axes
//!   *and* a completed execution, each independent of the others (FR-210).

use fsl_replay::Truth;

use filament_contract_model::CheckedNodeId;

use crate::closure::{ClosureRefusal, CompleteGlobalConformance, ProtocolAssessment};
use crate::declarations::Declarations;
use crate::disposition::Construct;
use crate::dynamics::{
    assess_channels, assess_joins, assess_loops, assess_roles, ChannelFact, ChannelOutcome,
    HistoryRefusal, JoinFact, JoinOutcome, LoopAssessment, LoopFact, ObservationRef,
    RoleAssessment, RoleFact,
};

/// The facts of one finite observed history, by the axis they are about.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HistoryFacts {
    /// Role lifetime and population facts.
    pub roles: Vec<RoleFact>,
    /// Channel facts.
    pub channels: Vec<ChannelFact>,
    /// Join facts.
    pub joins: Vec<JoinFact>,
    /// Loop facts.
    pub loops: Vec<LoopFact>,
}

impl HistoryFacts {
    /// The observation of every fact, across the four axes.
    fn observations(&self) -> Vec<&ObservationRef> {
        self.roles
            .iter()
            .map(|fact| &fact.at)
            .chain(self.channels.iter().map(|fact| &fact.at))
            .chain(self.joins.iter().map(|fact| &fact.at))
            .chain(self.loops.iter().map(|fact| &fact.at))
            .collect()
    }

    /// Append every fact of `later`, after the facts already held.
    pub fn extend(&mut self, later: Self) {
        self.roles.extend(later.roles);
        self.channels.extend(later.channels);
        self.joins.extend(later.joins);
        self.loops.extend(later.loops);
    }
}

/// The dynamic axes a history is judged on.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DynamicAxis {
    /// Role lifetime and population (FR-053).
    Roles,
    /// Queue capacity, overflow, order and delivery premises (FR-054).
    Channels,
    /// Join settlement (FR-055).
    Joins,
    /// Loop iteration bound (FR-056).
    Loops,
}

impl DynamicAxis {
    /// Every axis, in declared order.
    pub const ALL: [Self; 4] = [Self::Roles, Self::Channels, Self::Joins, Self::Loops];
}

/// One axis's verdict on a history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AxisVerdict {
    /// Assessed, and no violation was observed.
    Conformant,
    /// Assessed, and a violation of the declaration was observed.
    NonConformant,
    /// FSL would report an `Unsupported` cause for a declaration of the axis
    /// and no violation was observed on the rest: neither conformant nor
    /// violating (FR-057).
    Unsupported,
    /// An observation breaches what the protocol declares: it names a subject
    /// or a join branch the protocol does not declare, or a lifecycle event of
    /// a static role. It is reported with the refusal that names it, never
    /// repaired or discarded.
    Unexpected(HistoryRefusal),
    /// A fact cannot be read as an occurrence of the declared protocol (a
    /// workflow identity that is not text, or an occurrence FSL's key refuses).
    /// That is no breach of a declaration, so the axis is neither conformant
    /// nor violated: it is unavailable.
    Unreadable(HistoryRefusal),
}

/// Whether `refusal` is an observation that breaches the declarations.
const fn breaches_declarations(refusal: &HistoryRefusal) -> bool {
    match refusal {
        HistoryRefusal::UndeclaredSubject { .. }
        | HistoryRefusal::StaticRoleLifecycle { .. }
        | HistoryRefusal::UndeclaredBranch { .. } => true,
        HistoryRefusal::WorkflowNotText { .. } | HistoryRefusal::Occurrence(_) => false,
    }
}

impl AxisVerdict {
    /// Whether the verdict is `Conformant`.
    #[must_use]
    pub const fn is_conformant(&self) -> bool {
        matches!(self, Self::Conformant)
    }
}

/// The four assessments of one history, each exactly as [`crate::dynamics`]
/// returned it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryAssessment {
    roles: Result<Vec<RoleAssessment>, HistoryRefusal>,
    channels: Result<Vec<ChannelOutcome>, HistoryRefusal>,
    joins: Result<Vec<JoinOutcome>, HistoryRefusal>,
    loops: Result<Vec<LoopAssessment>, HistoryRefusal>,
}

impl HistoryAssessment {
    /// The role assessments, or the refusal.
    pub const fn roles(&self) -> &Result<Vec<RoleAssessment>, HistoryRefusal> {
        &self.roles
    }

    /// The channel outcomes, or the refusal.
    pub const fn channels(&self) -> &Result<Vec<ChannelOutcome>, HistoryRefusal> {
        &self.channels
    }

    /// The join outcomes, or the refusal.
    pub const fn joins(&self) -> &Result<Vec<JoinOutcome>, HistoryRefusal> {
        &self.joins
    }

    /// The loop assessments, or the refusal.
    pub const fn loops(&self) -> &Result<Vec<LoopAssessment>, HistoryRefusal> {
        &self.loops
    }

    /// The verdict on one axis.
    #[must_use]
    pub fn verdict(&self, axis: DynamicAxis) -> AxisVerdict {
        match axis {
            DynamicAxis::Roles => judge(
                &self.roles,
                |role| !role.lifetime_violations.is_empty() || role.population_violation.is_some(),
                |_| false,
            ),
            DynamicAxis::Channels => judge(
                &self.channels,
                |outcome| match outcome {
                    ChannelOutcome::Assessed(channel) => !channel.violations.is_empty(),
                    ChannelOutcome::Unsupported { .. } => false,
                },
                |outcome| matches!(outcome, ChannelOutcome::Unsupported { .. }),
            ),
            DynamicAxis::Joins => judge(
                &self.joins,
                |outcome| match outcome {
                    JoinOutcome::Assessed(join) => !join.violations.is_empty(),
                    JoinOutcome::Unsupported { .. } => false,
                },
                |outcome| matches!(outcome, JoinOutcome::Unsupported { .. }),
            ),
            DynamicAxis::Loops => {
                judge(&self.loops, |looped| looped.violation.is_some(), |_| false)
            }
        }
    }

    /// Every axis with its verdict, in declared order.
    #[must_use]
    pub fn verdicts(&self) -> Vec<(DynamicAxis, AxisVerdict)> {
        DynamicAxis::ALL
            .into_iter()
            .map(|axis| (axis, self.verdict(axis)))
            .collect()
    }

    /// The axes that are not conformant, in declared order.
    #[must_use]
    pub fn withheld(&self) -> Vec<DynamicAxis> {
        DynamicAxis::ALL
            .into_iter()
            .filter(|axis| !self.verdict(*axis).is_conformant())
            .collect()
    }

    /// The truth of the history: `violated` when any axis is non-conformant or
    /// holds an unexpected observation, `unavailable` when none is but an axis
    /// is unsupported or unreadable, otherwise `satisfied`. A violation
    /// outranks an unsupported axis.
    #[must_use]
    pub fn truth(&self) -> Truth {
        let verdicts = self.verdicts();
        if verdicts.iter().any(|(_, verdict)| {
            matches!(
                verdict,
                AxisVerdict::NonConformant | AxisVerdict::Unexpected(_)
            )
        }) {
            Truth::Violated
        } else if verdicts.iter().any(|(_, verdict)| {
            matches!(
                verdict,
                AxisVerdict::Unsupported | AxisVerdict::Unreadable(_)
            )
        }) {
            Truth::Unavailable
        } else {
            Truth::Satisfied
        }
    }
}

fn judge<T>(
    assessed: &Result<Vec<T>, HistoryRefusal>,
    violates: impl Fn(&T) -> bool,
    unsupported: impl Fn(&T) -> bool,
) -> AxisVerdict {
    match assessed {
        Err(refusal) if breaches_declarations(refusal) => AxisVerdict::Unexpected(refusal.clone()),
        Err(refusal) => AxisVerdict::Unreadable(refusal.clone()),
        Ok(items) if items.iter().any(violates) => AxisVerdict::NonConformant,
        Ok(items) if items.iter().any(unsupported) => AxisVerdict::Unsupported,
        Ok(_) => AxisVerdict::Conformant,
    }
}

/// Assess `facts` on every dynamic axis against `declarations`. The facts are
/// read in the history's own order (each assessment sorts by
/// [`ObservationRef`](crate::dynamics::ObservationRef)), so the order the slices
/// are presented in changes nothing, and no timestamp, arrival or key order is
/// read as a causal edge. `construct` resolves a checked node to its
/// [`Construct`] for an unsupported channel or join.
pub fn assess(
    declarations: &Declarations,
    construct: impl Fn(&CheckedNodeId) -> Construct,
    facts: &HistoryFacts,
) -> HistoryAssessment {
    HistoryAssessment {
        roles: assess_roles(declarations, &facts.roles),
        channels: assess_channels(declarations, &facts.channels, &construct),
        joins: assess_joins(declarations, &facts.joins, &construct),
        loops: assess_loops(declarations, &facts.loops),
    }
}

/// Why complete global success is withheld. Every reason is reported; none
/// masks another.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Withheld {
    /// The dynamic axes that are not conformant.
    pub axes: Vec<DynamicAxis>,
    /// The closure axes and execution states that do not admit it.
    pub closure: Vec<ClosureRefusal>,
}

/// The settlement of one history's global conformance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalSettlement {
    /// The history's own truth, unchanged by any closure state.
    pub truth: Truth,
    /// Complete global success, or every reason it is withheld.
    pub complete: Result<CompleteGlobalConformance, Withheld>,
}

/// Settle global conformance: the history's truth is read from the assessment
/// alone, and complete global success needs every dynamic axis conformant, the
/// five closure axes established and the execution completed. A closed
/// decision scope, a closed surrounding execution, a completed execution or a
/// settled truth is not a substitute for any other.
#[must_use]
pub fn settle_global(history: &HistoryAssessment, closure: ProtocolAssessment) -> GlobalSettlement {
    let axes = history.withheld();
    let complete = match closure.closure_established() {
        Ok(proof) if axes.is_empty() => Ok(CompleteGlobalConformance::of(proof)),
        Ok(_) => Err(Withheld {
            axes,
            closure: Vec::new(),
        }),
        Err(closure) => Err(Withheld { axes, closure }),
    };
    GlobalSettlement {
        truth: history.truth(),
        complete,
    }
}

/// The two paths disagree: the axes on which batch and incremental results
/// differ.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathDivergence {
    /// The axes whose results differ, in declared order.
    pub axes: Vec<DynamicAxis>,
}

/// Compare a batch result with an incremental one, axis by axis.
///
/// # Errors
///
/// [`PathDivergence`] naming each axis on which they differ; agreement is never
/// reported over a difference.
pub fn agree(
    batch: &HistoryAssessment,
    incremental: &HistoryAssessment,
) -> Result<(), PathDivergence> {
    let differs = |axis: DynamicAxis| match axis {
        DynamicAxis::Roles => batch.roles != incremental.roles,
        DynamicAxis::Channels => batch.channels != incremental.channels,
        DynamicAxis::Joins => batch.joins != incremental.joins,
        DynamicAxis::Loops => batch.loops != incremental.loops,
    };
    let axes: Vec<_> = DynamicAxis::ALL
        .into_iter()
        .filter(|axis| differs(*axis))
        .collect();
    if axes.is_empty() {
        Ok(())
    } else {
        Err(PathDivergence { axes })
    }
}

/// Why a prefix is not admitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrefixRefusal {
    /// A fact of the offered prefix is not after every fact already admitted,
    /// so the offered facts are not the next part of the history.
    NotExtending {
        /// The latest observation already admitted.
        admitted: ObservationRef,
        /// The earliest offered observation, which is not after it.
        offered: ObservationRef,
    },
}

/// The incremental path: facts arrive in admitted prefixes, and each prefix is
/// assessed as it arrives, so a value the prefix decides is settled before the
/// rest of the history is known. A prefix must extend the history: every fact
/// of it is after every fact already admitted, in the history's own order
/// ([`ObservationRef`]); one that is not is refused and admits nothing.
///
/// Each advance assesses everything admitted so far, so the cost of a history
/// of `n` facts in `k` prefixes is `k` assessments of up to `n` facts.
pub struct IncrementalHistory<'a, F> {
    declarations: &'a Declarations,
    construct: F,
    admitted: HistoryFacts,
    latest: Option<ObservationRef>,
}

impl<'a, F: Fn(&CheckedNodeId) -> Construct> IncrementalHistory<'a, F> {
    /// An incremental path that has admitted nothing.
    #[must_use]
    pub fn new(declarations: &'a Declarations, construct: F) -> Self {
        Self {
            declarations,
            construct,
            admitted: HistoryFacts::default(),
            latest: None,
        }
    }

    /// Admit the next prefix and assess everything admitted so far.
    ///
    /// # Errors
    ///
    /// [`PrefixRefusal::NotExtending`] when a fact of `next` is not after the
    /// latest fact already admitted; nothing is admitted.
    pub fn advance(&mut self, next: HistoryFacts) -> Result<HistoryAssessment, PrefixRefusal> {
        let offered = next.observations();
        if let (Some(admitted), Some(first)) = (&self.latest, offered.iter().min()) {
            if *first <= admitted {
                return Err(PrefixRefusal::NotExtending {
                    admitted: admitted.clone(),
                    offered: (*first).clone(),
                });
            }
        }
        if let Some(last) = offered.into_iter().max() {
            self.latest = Some(last.clone());
        }
        self.admitted.extend(next);
        Ok(assess(self.declarations, &self.construct, &self.admitted))
    }
}
