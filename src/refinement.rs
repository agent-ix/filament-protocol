// SPDX-License-Identifier: AGPL-3.0-or-later
//! FR-301, FR-202, FR-204: the protocol refinement decision procedure
//! (ADR-010 Decision 6).
//!
//! This library decides whether an implementation protocol refines a
//! specification protocol, each given as a finite [`TransitionSystem`]. The
//! decision is over observable traces, hiding the internal steps:
//!
//! - **Extra traces.** The implementation has an observable trace the
//!   specification does not allow.
//! - **Deadlock.** The implementation reaches a non-final state with no step
//!   at all while the specification, after the same observable trace, can
//!   still continue.
//! - **Divergence.** The implementation can take internal steps forever after
//!   an observable trace after which the specification cannot.
//! - **Stronger assumptions.** Either system rests on an assumption the request
//!   did not offer.
//! - **Unsupported dynamics.** A replicated role, a symbolic queue capacity or
//!   a construct the system builder could not step in the fragment is never
//!   decided: the claim never refines (FSL FR-148's rule for what it cannot
//!   decide).
//!
//! The specification side is explored by subset construction over its
//! internal-step closure, so a specification that is nondeterministic or takes
//! internal steps of its own is judged correctly. Every finding is kept beside
//! the verdict, and the findings depend on neither the order of the transitions
//! nor the order of any list.
//!
//! The transition system is this library's own small type, derived from FSL's
//! read-only protocol step view (FSL FR-578) by [`TransitionSystem::from_view`]
//! and [`step_system`]. Which steps are observable is this library's to define
//! (FR-301): a `send` or `receive` on a channel of the claim's fragment is
//! observable, labelled by the channel's checked node; every other step is
//! hidden. Atomic and temporal obligations stay FSL's and are not evaluated
//! here.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use filament_contract_model::{CheckedNodeId, CheckedPackageV2};
use fsl_analyze::protocol_analysis::protocol_step_view;
use fsl_eval::model_subject::ProtocolSubject;
use fsl_eval::simulation::protocol::StepKind;
use fsl_package::RoleReplication;
use fsl_replay::protocol_analysis::AssessedRoot;
use fsl_replay::{
    ProtocolStepView, ProtocolSteps, RefinementWitness, RefinementWitnessError, SettlementBasis,
    StateIndex, Truth, UnsteppedConstruct,
};
use fsl_semantics::check::wire_id::WireGraph;

use crate::assume_guarantee::{bind_fragment, owner};
use crate::closure::AssessmentExecution;
use crate::declarations::{Capacity, Declarations};
use crate::dispatch::{DispositionRefusal, Fragment};

/// A state of a [`TransitionSystem`], by its index in that system.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StateId(pub u32);

/// What a step does.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Label {
    /// An internal step, hidden from the observable trace.
    Internal,
    /// An observable action, by the checked node of the channel or role event
    /// it is.
    Observable(CheckedNodeId),
}

/// One step of a transition system.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Transition {
    /// The state it leaves.
    pub from: StateId,
    /// What it does.
    pub label: Label,
    /// The state it reaches.
    pub to: StateId,
}

/// A finite protocol system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionSystem {
    /// The initial state.
    pub initial: StateId,
    /// The states in which the protocol may legitimately stop.
    pub final_states: Vec<StateId>,
    /// The steps.
    pub transitions: Vec<Transition>,
    /// The assumptions the system's behaviour rests on.
    pub assumptions: Vec<CheckedNodeId>,
    /// The constructs the builder could not step, as FSL names them.
    pub unsupported: Vec<UnsteppedConstruct>,
    /// Why the builder produced no system when FSL's view stopped (a bound, an
    /// undetermined state, a refused read); `None` for a built system. A
    /// system with a reason holds no steps and is never explored.
    pub incomplete: Option<String>,
}

impl TransitionSystem {
    /// The system of FSL's step view `view` of the protocol in `package`, with
    /// the steps observable that send or receive on a channel in `observed`
    /// (FR-301). Every other step is internal.
    ///
    /// A view that holds unsupported constructs gives a system that names
    /// them and has no step. A multi-state initial set gets one extra
    /// internal start state that steps to each. The assumptions are the
    /// directly required temporal clauses; the protocol's own scheduler and
    /// memory fairness are not claim assumptions.
    #[must_use]
    pub fn from_view(
        package: &CheckedPackageV2,
        view: ProtocolStepView,
        observed: &[CheckedNodeId],
    ) -> Self {
        let held = |unsupported, incomplete| Self {
            initial: StateId(0),
            final_states: Vec::new(),
            transitions: Vec::new(),
            assumptions: Vec::new(),
            unsupported,
            incomplete,
        };
        match view {
            ProtocolStepView::Unsupported(found) => held(found, None),
            ProtocolStepView::Steps(steps) => Self::from_steps(package, &steps, observed)
                .unwrap_or_else(|why| held(Vec::new(), Some(why.to_owned()))),
        }
    }

    fn from_steps(
        package: &CheckedPackageV2,
        steps: &ProtocolSteps,
        observed: &[CheckedNodeId],
    ) -> Result<Self, &'static str> {
        let id = |at: StateIndex| {
            u32::try_from(at.0)
                .map(StateId)
                .map_err(|_| "a state index is beyond a state id")
        };
        let mut transitions = Vec::with_capacity(steps.steps.len());
        for step in &steps.steps {
            let communicates = matches!(step.kind, StepKind::Send | StepKind::Receive);
            let label = match &step.channel {
                Some(channel) if communicates && observed.contains(channel) => {
                    Label::Observable(channel.clone())
                }
                _ => Label::Internal,
            };
            transitions.push(Transition {
                from: id(step.from)?,
                label,
                to: id(step.to)?,
            });
        }
        let initial = match steps.initial.as_slice() {
            [only] => id(*only)?,
            many => {
                let origin = id(StateIndex(steps.states))?;
                for at in many {
                    transitions.push(Transition {
                        from: origin,
                        label: Label::Internal,
                        to: id(*at)?,
                    });
                }
                origin
            }
        };
        let mut assumptions = Vec::new();
        let nodes = &package.resolved_graph().nodes;
        for required in steps
            .assumptions
            .iter()
            .filter_map(|held| held.required_temporal.as_ref())
        {
            let graph = WireGraph::new(nodes.iter().map(|node| &node.node_id))
                .map_err(|_| "the package has no wire identities")?;
            let clause = nodes
                .iter()
                .map(|node| &node.node_id)
                .find(|node| graph.key(node).is_ok_and(|key| key == required.clause))
                .ok_or("a required temporal clause names no node")?;
            assumptions.push(clause.clone());
        }
        Ok(Self {
            initial,
            final_states: steps
                .final_states
                .iter()
                .map(|at| id(*at))
                .collect::<Result<_, _>>()?,
            transitions,
            assumptions,
            unsupported: Vec::new(),
            incomplete: None,
        })
    }
}

/// FSL's step view of the protocol clause `protocol` of `package` over
/// `subject`, as a [`TransitionSystem`] observing the channels of `fragment`
/// (FR-301). FSL's refusal, including more than `max_states` states, gives a
/// system that records why and is never explored: the claim settles
/// `incomplete`, never `refines`.
#[must_use]
pub fn step_system(
    package: &CheckedPackageV2,
    protocol: &CheckedNodeId,
    subject: &ProtocolSubject<'_>,
    fragment: &Fragment,
    max_states: usize,
) -> TransitionSystem {
    match protocol_step_view(package, protocol, subject, max_states) {
        Ok(view) => TransitionSystem::from_view(package, view, &fragment.channels),
        Err(refusal) => TransitionSystem {
            initial: StateId(0),
            final_states: Vec::new(),
            transitions: Vec::new(),
            assumptions: Vec::new(),
            unsupported: Vec::new(),
            incomplete: Some(refusal.to_string()),
        },
    }
}

/// One protocol of a refinement: its checked node and its system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Side {
    /// The protocol's checked node.
    pub node: CheckedNodeId,
    /// Its system.
    pub system: TransitionSystem,
}

/// Which protocol of a refinement.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Party {
    /// The implementation.
    Implementation,
    /// The specification.
    Specification,
}

/// One requested `refinement` claim: the implementation refines the
/// specification under the offered assumptions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefinementRequest {
    /// The admitted fragment the claim ranges over.
    pub fragment: Fragment,
    /// The implementation protocol.
    pub implementation: Side,
    /// The specification protocol.
    pub specification: Side,
    /// The assumptions the claim is requested under.
    pub assumptions: Vec<CheckedNodeId>,
    /// The source identities the disposition retains (FR-030).
    pub sources: Vec<CheckedNodeId>,
    /// The most product states the decision may explore.
    pub max_states: usize,
}

/// Why a claim is not decided.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Undecidable {
    /// The fragment holds a replicated role (dynamic population).
    DynamicRole(CheckedNodeId),
    /// The fragment holds a channel with a symbolic capacity (unbounded).
    UnboundedQueue(CheckedNodeId),
    /// A system's builder could not step this construct.
    Construct(Party, UnsteppedConstruct),
}

/// The side of a counterexample that says what the specification can do.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SpecSide {
    /// The observable trace the implementation run shows, up to the offending
    /// action for an extra trace.
    pub trace: Vec<CheckedNodeId>,
    /// The specification states reachable on that trace, internal steps
    /// included.
    pub reached: BTreeSet<StateId>,
}

/// What went wrong in one counterexample.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Fault {
    /// The run's last step is an observable action the specification cannot
    /// take after `SpecSide::trace`.
    ExtraTrace {
        /// The offending action.
        action: CheckedNodeId,
    },
    /// The run ends in a non-final state with no step, where the specification
    /// can continue.
    Deadlock,
    /// After the run the implementation takes internal steps forever, and the
    /// specification cannot.
    Divergence {
        /// Internal steps from the run's end, the last of which returns to a
        /// state already on the lasso.
        lasso: Vec<Transition>,
    },
}

/// The pair of runs that refutes a refinement: the implementation's and the
/// specification's.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Counterexample {
    /// What went wrong.
    pub fault: Fault,
    /// The implementation run from its initial state.
    pub implementation: Vec<Transition>,
    /// What the specification can do on the same observable trace.
    pub specification: SpecSide,
}

/// A system rests on an assumption the request did not offer (FR-204).
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StrongerAssumption {
    /// The system that rests on it.
    pub party: Party,
    /// The assumption.
    pub assumption: CheckedNodeId,
    /// The offered side: the request's assumptions.
    pub offered: Vec<CheckedNodeId>,
}

/// The decided refinement: every finding, bound to its request (FR-030).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refinement {
    /// The request, unchanged.
    pub request: RefinementRequest,
    /// What the procedure could not decide.
    pub undecidable: Vec<Undecidable>,
    /// The extra traces, deadlocks and divergences found.
    pub counterexamples: Vec<Counterexample>,
    /// The assumptions rested on and not offered.
    pub stronger_assumptions: Vec<StrongerAssumption>,
    /// The product states explored.
    pub explored: usize,
    /// Whether the exploration reached every product state.
    pub complete: bool,
}

/// What a decided refinement is. The precedence is `Unsupported`, then
/// `Refuted`, then `Incomplete`, then `Refines`, whatever the order of the
/// transitions and of any list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// Every observable trace is allowed, no deadlock or divergence is added
    /// and no assumption is stronger.
    Refines,
    /// A counterexample or a stronger assumption was found.
    Refuted,
    /// The exploration reached its bound before every product state.
    Incomplete,
    /// Something in the claim cannot be decided.
    Unsupported,
}

impl Refinement {
    /// The verdict. `Refines` only when there is no finding of any other kind.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        if !self.undecidable.is_empty() {
            Verdict::Unsupported
        } else if !self.counterexamples.is_empty() || !self.stronger_assumptions.is_empty() {
            Verdict::Refuted
        } else if !self.complete {
            Verdict::Incomplete
        } else {
            Verdict::Refines
        }
    }

    /// The FR-241 assessment-execution value.
    #[must_use]
    pub fn execution(&self) -> AssessmentExecution {
        match self.verdict() {
            Verdict::Refines | Verdict::Refuted => AssessmentExecution::Completed,
            Verdict::Incomplete => AssessmentExecution::ResourceIncomplete,
            Verdict::Unsupported => AssessmentExecution::Unsupported,
        }
    }

    /// The FR-242 truth: `satisfied` only for [`Verdict::Refines`].
    #[must_use]
    pub fn truth(&self) -> Truth {
        match self.verdict() {
            Verdict::Refines => Truth::Satisfied,
            Verdict::Refuted => Truth::Violated,
            Verdict::Incomplete | Verdict::Unsupported => Truth::Unavailable,
        }
    }

    /// The FR-243 settlement basis.
    #[must_use]
    pub fn basis(&self) -> SettlementBasis {
        match self.verdict() {
            Verdict::Refines => SettlementBasis::ClosedScope,
            Verdict::Refuted => SettlementBasis::DecisiveCounterexample,
            Verdict::Incomplete | Verdict::Unsupported => SettlementBasis::Unavailable,
        }
    }
}

impl Refinement {
    /// The FR-302 trace-position witness of a refuted claim, as FSL's FR-351
    /// record built by [`RefinementWitness::new`] under the claim's assessed
    /// `root`; `None` for any other verdict. Never recomputed from the bytes:
    /// the carried record is FSL's own writer's.
    ///
    /// The deciding element and position come from the shortest
    /// counterexample (ties by the counterexample's own order), so they
    /// depend on neither the order of the transitions nor of any list:
    /// an extra trace is decided by the offending action's channel node at
    /// the offending step's zero-based position in the implementation run; a
    /// deadlock or divergence by the implementation protocol's node at the
    /// run's length. A refutation by a stronger assumption alone is decided
    /// by that assumption at position zero.
    ///
    /// # Errors
    ///
    /// [`RefinementWitnessError`] when FSL cannot write the record.
    pub fn witness(
        &self,
        root: &AssessedRoot,
    ) -> Result<Option<RefinementWitness>, RefinementWitnessError> {
        if self.verdict() != Verdict::Refuted {
            return Ok(None);
        }
        let (deciding, position) = match self
            .counterexamples
            .iter()
            .min_by_key(|found| (found.implementation.len(), *found))
        {
            Some(Counterexample {
                fault: Fault::ExtraTrace { action },
                implementation,
                ..
            }) => (action, implementation.len().saturating_sub(1)),
            Some(Counterexample { implementation, .. }) => {
                (&self.request.implementation.node, implementation.len())
            }
            None => match self.stronger_assumptions.first() {
                Some(stronger) => (&stronger.assumption, 0),
                None => return Ok(None),
            },
        };
        RefinementWitness::new(root, deciding, position).map(Some)
    }
}

/// The settlement of one refinement claim.
// One outcome per requested claim, held briefly; boxing would only hide the
// decision the caller matches on.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RefinementOutcome {
    /// The procedure decided the claim.
    Decided(Refinement),
    /// The claim is refused under FR-030; it holds no success.
    Refused(DispositionRefusal),
}

/// A system as sorted adjacency, with the states from which internal steps can
/// go on forever.
struct Graph {
    initial: StateId,
    finals: BTreeSet<StateId>,
    succ: BTreeMap<StateId, BTreeSet<(Label, StateId)>>,
    divergent: BTreeSet<StateId>,
}

impl Graph {
    fn new(system: &TransitionSystem) -> Self {
        let mut succ: BTreeMap<StateId, BTreeSet<(Label, StateId)>> = BTreeMap::new();
        for step in &system.transitions {
            succ.entry(step.from)
                .or_default()
                .insert((step.label.clone(), step.to));
        }
        let internal = |state: &StateId, among: &BTreeSet<StateId>| {
            succ.get(state).is_some_and(|steps| {
                steps
                    .iter()
                    .any(|(label, to)| *label == Label::Internal && among.contains(to))
            })
        };
        // The greatest set in which every state has an internal step into it.
        let mut divergent: BTreeSet<StateId> = succ
            .keys()
            .copied()
            .filter(|state| internal(state, &succ.keys().copied().collect()))
            .collect();
        loop {
            let keep: BTreeSet<StateId> = divergent
                .iter()
                .copied()
                .filter(|state| internal(state, &divergent))
                .collect();
            if keep.len() == divergent.len() {
                break;
            }
            divergent = keep;
        }
        Self {
            initial: system.initial,
            finals: system.final_states.iter().copied().collect(),
            succ,
            divergent,
        }
    }

    fn steps(&self, state: StateId) -> impl Iterator<Item = &(Label, StateId)> {
        self.succ.get(&state).into_iter().flatten()
    }

    fn has(&self, step: &Transition) -> bool {
        self.succ
            .get(&step.from)
            .is_some_and(|steps| steps.contains(&(step.label.clone(), step.to)))
    }

    /// `set` with every state reachable from it by internal steps.
    fn closure(&self, set: &BTreeSet<StateId>) -> BTreeSet<StateId> {
        let mut seen = set.clone();
        let mut stack: Vec<StateId> = set.iter().copied().collect();
        while let Some(state) = stack.pop() {
            for (label, to) in self.steps(state) {
                if *label == Label::Internal && seen.insert(*to) {
                    stack.push(*to);
                }
            }
        }
        seen
    }

    /// The closure of the states `set` reaches by the observable `action`.
    fn after(&self, set: &BTreeSet<StateId>, action: &CheckedNodeId) -> BTreeSet<StateId> {
        let reached = set
            .iter()
            .flat_map(|state| self.steps(*state))
            .filter(|(label, _)| matches!(label, Label::Observable(node) if node == action))
            .map(|(_, to)| *to)
            .collect();
        self.closure(&reached)
    }

    fn start(&self) -> BTreeSet<StateId> {
        self.closure(&BTreeSet::from([self.initial]))
    }
}

/// The observable actions along `run`, in order.
fn visible(run: &[Transition]) -> Vec<CheckedNodeId> {
    run.iter()
        .filter_map(|step| match &step.label {
            Label::Observable(node) => Some(node.clone()),
            Label::Internal => None,
        })
        .collect()
}

/// A product state: an implementation state and the specification states
/// reachable on the same observable trace.
type Node = (StateId, BTreeSet<StateId>);

/// What the exploration found.
struct Explored {
    findings: BTreeSet<Counterexample>,
    states: usize,
    complete: bool,
}

/// The implementation run that first reached `node`.
fn run_to(parents: &BTreeMap<Node, Option<(Node, Transition)>>, node: &Node) -> Vec<Transition> {
    let mut run = Vec::new();
    let mut at = node;
    while let Some(Some((before, step))) = parents.get(at) {
        run.push(step.clone());
        at = before;
    }
    run.reverse();
    run
}

/// The internal steps from `state`, within `divergent`, until a state repeats.
fn lasso(graph: &Graph, state: StateId) -> Vec<Transition> {
    let mut steps = Vec::new();
    let mut seen = BTreeSet::from([state]);
    let mut at = state;
    while let Some((_, to)) = graph
        .steps(at)
        .find(|(label, to)| *label == Label::Internal && graph.divergent.contains(to))
    {
        steps.push(Transition {
            from: at,
            label: Label::Internal,
            to: *to,
        });
        if !seen.insert(*to) {
            break;
        }
        at = *to;
    }
    steps
}

/// Breadth-first exploration of the product, shortest runs first.
fn explore(implementation: &Graph, specification: &Graph, max_states: usize) -> Explored {
    let start: Node = (implementation.initial, specification.start());
    let mut findings = BTreeSet::new();
    let mut parents: BTreeMap<Node, Option<(Node, Transition)>> = BTreeMap::new();
    let mut complete = true;
    if max_states == 0 {
        return Explored {
            findings,
            states: 0,
            complete: false,
        };
    }
    parents.insert(start.clone(), None);
    let mut queue = VecDeque::from([start]);
    while let Some(node) = queue.pop_front() {
        let (state, spec_set) = &node;
        let counterexample =
            |fault, run: Vec<Transition>, trace_run: &[Transition]| Counterexample {
                fault,
                implementation: run,
                specification: SpecSide {
                    trace: visible(trace_run),
                    reached: spec_set.clone(),
                },
            };
        let run = run_to(&parents, &node);
        let stuck = implementation.steps(*state).next().is_none();
        let spec_continues = spec_set
            .iter()
            .any(|q| specification.steps(*q).next().is_some());
        if stuck && !implementation.finals.contains(state) && spec_continues {
            findings.insert(counterexample(Fault::Deadlock, run.clone(), &run));
        }
        // Reported where the divergence starts, not at every state along it.
        let diverges = |at: &Node| {
            implementation.divergent.contains(&at.0)
                && at.1.iter().all(|q| !specification.divergent.contains(q))
        };
        let starts_here = diverges(&node)
            && !matches!(parents.get(&node), Some(Some((before, _))) if diverges(before));
        if starts_here {
            let lasso = lasso(implementation, *state);
            findings.insert(counterexample(
                Fault::Divergence { lasso },
                run.clone(),
                &run,
            ));
        }
        for (label, to) in implementation.steps(*state) {
            let step = Transition {
                from: *state,
                label: label.clone(),
                to: *to,
            };
            let next_set = match label {
                Label::Internal => spec_set.clone(),
                Label::Observable(action) => {
                    let after = specification.after(spec_set, action);
                    if after.is_empty() {
                        let mut offending = run.clone();
                        offending.push(step);
                        findings.insert(counterexample(
                            Fault::ExtraTrace {
                                action: action.clone(),
                            },
                            offending,
                            &run,
                        ));
                        continue;
                    }
                    after
                }
            };
            let next: Node = (*to, next_set);
            if parents.contains_key(&next) {
                continue;
            }
            if parents.len() >= max_states {
                complete = false;
                continue;
            }
            parents.insert(next.clone(), Some((node.clone(), step)));
            queue.push_back(next);
        }
    }
    Explored {
        findings,
        states: parents.len(),
        complete,
    }
}

impl Counterexample {
    /// Replay the counterexample against `request`: the implementation run
    /// starts at its initial state and every step is one of its transitions;
    /// the specification side is exactly what the specification reaches on
    /// the run's observable trace; and the fault holds there.
    #[must_use]
    pub fn replay(&self, request: &RefinementRequest) -> bool {
        let implementation = Graph::new(&request.implementation.system);
        let specification = Graph::new(&request.specification.system);
        let Some(end) = walk(
            &implementation,
            implementation.initial,
            &self.implementation,
        ) else {
            return false;
        };
        let reached = self
            .specification
            .trace
            .iter()
            .fold(specification.start(), |set, action| {
                specification.after(&set, action)
            });
        if reached.is_empty() || reached != self.specification.reached {
            return false;
        }
        let shown = visible(&self.implementation);
        match &self.fault {
            Fault::ExtraTrace { action } => {
                let mut expected = self.specification.trace.clone();
                expected.push(action.clone());
                self.implementation
                    .last()
                    .is_some_and(|last| last.label == Label::Observable(action.clone()))
                    && shown == expected
                    && specification.after(&reached, action).is_empty()
            }
            Fault::Deadlock => {
                shown == self.specification.trace
                    && implementation.steps(end).next().is_none()
                    && !implementation.finals.contains(&end)
                    && reached
                        .iter()
                        .any(|q| specification.steps(*q).next().is_some())
            }
            Fault::Divergence { lasso } => {
                let Some(back) = walk(&implementation, end, lasso) else {
                    return false;
                };
                // The last step returns to a state the lasso already left.
                let left: BTreeSet<StateId> = std::iter::once(end)
                    .chain(lasso.iter().map(|step| step.from))
                    .collect();
                shown == self.specification.trace
                    && !lasso.is_empty()
                    && lasso.iter().all(|step| step.label == Label::Internal)
                    && left.contains(&back)
                    && reached.iter().all(|q| !specification.divergent.contains(q))
            }
        }
    }
}

/// The state `steps` leads to from `from`, when each is a transition of
/// `graph` that starts where the previous one ended.
fn walk(graph: &Graph, from: StateId, steps: &[Transition]) -> Option<StateId> {
    steps.iter().try_fold(from, |at, step| {
        (step.from == at && graph.has(step)).then_some(step.to)
    })
}

impl StrongerAssumption {
    /// Replay against `request`: the party's system rests on the assumption,
    /// the request does not offer it, and the offered side is the request's.
    #[must_use]
    pub fn replay(&self, request: &RefinementRequest) -> bool {
        let side = match self.party {
            Party::Implementation => &request.implementation,
            Party::Specification => &request.specification,
        };
        side.system.assumptions.contains(&self.assumption)
            && !request.assumptions.contains(&self.assumption)
            && self.offered == request.assumptions
    }
}

/// Bind a refinement request (FR-030, FR-301): the fragment, both protocols,
/// the assumptions, the observable labels and the sources, each checked
/// against the component package that owns the node.
///
/// # Errors
///
/// [`DispositionRefusal`] naming the absent or undeclared node.
pub fn bind(
    components: &[&Declarations],
    request: &RefinementRequest,
) -> Result<(), DispositionRefusal> {
    bind_fragment(components, &request.fragment)?;
    let declared = |node: &CheckedNodeId| owner(components, node).is_some();
    for side in [&request.implementation, &request.specification] {
        if !declared(&side.node) {
            return Err(DispositionRefusal::UndeclaredSubject(side.node.clone()));
        }
        if let Some(node) = side.system.assumptions.iter().find(|node| !declared(node)) {
            return Err(DispositionRefusal::UndeclaredAssumption(node.clone()));
        }
        let observable = side
            .system
            .transitions
            .iter()
            .filter_map(|step| match &step.label {
                Label::Observable(node) => Some(node),
                Label::Internal => None,
            });
        if let Some(node) = observable.into_iter().find(|node| !declared(node)) {
            return Err(DispositionRefusal::UndeclaredLabel(node.clone()));
        }
    }
    if let Some(node) = request.assumptions.iter().find(|node| !declared(node)) {
        return Err(DispositionRefusal::UndeclaredAssumption(node.clone()));
    }
    if let Some(node) = request.sources.iter().find(|node| !declared(node)) {
        return Err(DispositionRefusal::UndeclaredSource(node.clone()));
    }
    Ok(())
}

/// The order findings about unstepped constructs are kept in, whatever the
/// order the view listed them.
#[derive(Eq, Ord, PartialEq, PartialOrd)]
enum ConstructRank {
    Activation(CheckedNodeId),
    Role(CheckedNodeId),
    Construct(&'static str),
    Refused(String),
}

fn construct_rank(construct: &UnsteppedConstruct) -> ConstructRank {
    match construct {
        UnsteppedConstruct::ActivationOnEach(node) => ConstructRank::Activation(node.clone()),
        UnsteppedConstruct::ReplicatedRole(node) => ConstructRank::Role(node.clone()),
        UnsteppedConstruct::Construct(construct) => ConstructRank::Construct(construct.construct()),
        UnsteppedConstruct::Refused(refusal) => ConstructRank::Refused(refusal.to_string()),
    }
}

/// What the fragment's declarations and the two systems say the procedure
/// cannot decide.
fn undecidable(components: &[&Declarations], request: &RefinementRequest) -> Vec<Undecidable> {
    let mut found = Vec::new();
    for node in &request.fragment.roles {
        let replicated = owner(components, node)
            .and_then(|declarations| declarations.role(node))
            .is_some_and(|role| !matches!(role.replication, RoleReplication::Static));
        if replicated {
            found.push(Undecidable::DynamicRole(node.clone()));
        }
    }
    for node in &request.fragment.channels {
        let symbolic = owner(components, node)
            .and_then(|declarations| declarations.channel(node))
            .is_some_and(|channel| matches!(channel.capacity, Capacity::Symbolic(_)));
        if symbolic {
            found.push(Undecidable::UnboundedQueue(node.clone()));
        }
    }
    let mut constructs: Vec<(Party, UnsteppedConstruct)> = [
        (Party::Implementation, &request.implementation),
        (Party::Specification, &request.specification),
    ]
    .into_iter()
    .flat_map(|(party, side)| {
        side.system
            .unsupported
            .iter()
            .map(move |u| (party, u.clone()))
    })
    .collect();
    constructs.sort_by_cached_key(|(party, construct)| (*party, construct_rank(construct)));
    constructs.dedup();
    found.extend(
        constructs
            .into_iter()
            .map(|(party, construct)| Undecidable::Construct(party, construct)),
    );
    found
}

/// Decide one `refinement` claim (FR-301): bind it, then run the procedure.
/// A `refines` or `refuted` verdict needs a source identity to be attributed
/// to (FR-030).
pub fn decide(components: &[&Declarations], request: &RefinementRequest) -> RefinementOutcome {
    if let Err(refusal) = bind(components, request) {
        return RefinementOutcome::Refused(refusal);
    }
    let sides = [
        &request.implementation.system,
        &request.specification.system,
    ];
    // A system with no steps because its builder could not step the protocol,
    // or stopped, is not the protocol: exploring it would invent faults.
    let steppable = sides
        .iter()
        .all(|system| system.unsupported.is_empty() && system.incomplete.is_none());
    let explored = if steppable {
        explore(
            &Graph::new(&request.implementation.system),
            &Graph::new(&request.specification.system),
            request.max_states,
        )
    } else {
        Explored {
            findings: BTreeSet::new(),
            states: 0,
            complete: sides.iter().all(|system| system.incomplete.is_none()),
        }
    };
    let offered: BTreeSet<&CheckedNodeId> = request.assumptions.iter().collect();
    let stronger_assumptions: BTreeSet<StrongerAssumption> = [
        (Party::Implementation, &request.implementation),
        (Party::Specification, &request.specification),
    ]
    .into_iter()
    .flat_map(|(party, side)| {
        side.system
            .assumptions
            .iter()
            .filter(|assumption| !offered.contains(assumption))
            .map(move |assumption| (party, assumption))
    })
    .map(|(party, assumption)| StrongerAssumption {
        party,
        assumption: assumption.clone(),
        offered: request.assumptions.clone(),
    })
    .collect();
    let refinement = Refinement {
        request: request.clone(),
        undecidable: undecidable(components, request),
        counterexamples: explored.findings.into_iter().collect(),
        stronger_assumptions: stronger_assumptions.into_iter().collect(),
        explored: explored.states,
        complete: explored.complete,
    };
    let decisive = matches!(refinement.verdict(), Verdict::Refines | Verdict::Refuted);
    if decisive && request.sources.is_empty() {
        return RefinementOutcome::Refused(DispositionRefusal::NoSourceIdentity);
    }
    RefinementOutcome::Decided(refinement)
}
