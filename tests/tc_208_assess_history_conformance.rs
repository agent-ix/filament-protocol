// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-208: finite global conformance of one observed history of a
//! choreography compiled by FSL's checked-handoff writer, judged by this
//! library's own assessment (FR-208 to FR-211). No FSL engine is involved.

mod support {
    pub mod dynamic;
    pub mod handoff;
}

use filament_exact::Integer;
use filament_protocol::closure::{
    AssessmentExecution, Axis, ClosureRefusal, ClosureState, DecisionScopeClosure,
    DecisionScopeProgress, GlobalConformanceClosure, ProtocolAssessment, ScopeAxis, ScopeState,
    SurroundingExecutionClosure, SurroundingExecutionProgress,
};
use filament_protocol::dynamics::{
    ChannelEvent, ChannelFact, ChannelOutcome, HistoryRefusal, JoinEvent, JoinFact, JoinOutcome,
    LoopEvent, LoopFact, QueueViolation, RoleEvent, RoleFact,
};
use filament_protocol::history::{
    agree, assess, settle_global, AxisVerdict, DynamicAxis, HistoryAssessment, HistoryFacts,
    IncrementalHistory, PrefixRefusal,
};
use fsl_replay::Truth;
use ix_trace_rs::trace;
use support::dynamic::{at, instance, message, workflow, Dynamic};

/// `workers` spawn and act; `queued` messages enter the `Reject` queue
/// (capacity 2) with none delivered; `branches` of `JoinAll` complete and the
/// join settles; `iterations` of `Bounded` (maximum 3) run and the loop exits.
/// Positions are consecutive from 1, in the order the groups are built.
fn history(
    dynamic: &Dynamic,
    workers: u64,
    queued: u64,
    branches: &[&str],
    iterations: u64,
) -> HistoryFacts {
    let mut position = 0;
    let mut next = || {
        position += 1;
        at(position)
    };
    let role = dynamic.named("role", "R");
    let mut roles = Vec::new();
    for worker in 0..workers {
        let name = format!("r{worker}");
        for event in [RoleEvent::Spawn, RoleEvent::Act] {
            roles.push(RoleFact {
                at: next(),
                role: role.clone(),
                workflow: workflow("w1"),
                instance: instance(&name),
                event,
            });
        }
    }
    let channel = dynamic.named("channel", "Reject");
    let channels = (0..queued)
        .map(|sent| ChannelFact {
            at: next(),
            channel: channel.clone(),
            event: ChannelEvent::Enqueue {
                message: message(&format!("m{sent}")),
                key: Box::from(&b"k"[..]),
            },
        })
        .collect();
    let join = dynamic.named("control", "JoinAll");
    let join_fact = |at, event| JoinFact {
        at,
        join: join.clone(),
        workflow: workflow("w1"),
        repeat_ordinals: Vec::new(),
        event,
    };
    let mut joins: Vec<JoinFact> = branches
        .iter()
        .map(|branch| {
            join_fact(
                next(),
                JoinEvent::BranchCompleted {
                    branch: (*branch).into(),
                },
            )
        })
        .collect();
    joins.push(join_fact(next(), JoinEvent::Settled));
    let control = dynamic.named("control", "Bounded");
    let ordinals: Vec<Integer> = Vec::new();
    let loops = (0..iterations)
        .map(|_| LoopEvent::Iteration)
        .chain([LoopEvent::Exit])
        .map(|event| LoopFact {
            at: next(),
            control: control.clone(),
            workflow: workflow("w1"),
            repeat_ordinals: ordinals.clone(),
            event,
        })
        .collect();
    HistoryFacts {
        roles,
        channels,
        joins,
        loops,
    }
}

fn judge(dynamic: &Dynamic, facts: &HistoryFacts) -> HistoryAssessment {
    assess(
        dynamic.linked().declarations(),
        |node| dynamic.construct(node),
        facts,
    )
}

fn conforming(dynamic: &Dynamic) -> HistoryFacts {
    history(dynamic, 2, 2, &["a", "b", "c"], 3)
}

fn faulty(dynamic: &Dynamic) -> HistoryFacts {
    history(dynamic, 4, 3, &["a", "b"], 4)
}

fn reversed(facts: &HistoryFacts) -> HistoryFacts {
    let mut facts = facts.clone();
    facts.roles.reverse();
    facts.channels.reverse();
    facts.joins.reverse();
    facts.loops.reverse();
    facts
}

// ---------------------------------------------------------------------------
// FR-208: order invariance
// ---------------------------------------------------------------------------

#[test]
#[trace("TC-208", "FR-208-AC-1")]
fn presenting_the_same_facts_in_the_opposite_order_changes_no_result() {
    let dynamic = Dynamic::with_finite_capacities();
    for facts in [conforming(&dynamic), faulty(&dynamic)] {
        assert_eq!(judge(&dynamic, &facts), judge(&dynamic, &reversed(&facts)));
    }
    // The two histories are not equal to each other, so equality above is not
    // an assessment that ignores its input.
    assert_ne!(
        judge(&dynamic, &conforming(&dynamic)),
        judge(&dynamic, &faulty(&dynamic))
    );
}

#[test]
#[trace("TC-208", "FR-208-AC-2")]
fn exchanging_the_positions_of_unconstrained_facts_changes_no_result() {
    let dynamic = Dynamic::with_finite_capacities();
    let facts = conforming(&dynamic);
    let expected = judge(&dynamic, &facts);

    // Two role instances spawn independently: r0 and r1 exchange their spawns
    // and their acts.
    let mut roles = facts.clone();
    let (first, second) = (0, 2);
    let held = roles.roles[first].at.clone();
    roles.roles[first].at = roles.roles[second].at.clone();
    roles.roles[second].at = held;
    let held = roles.roles[first + 1].at.clone();
    roles.roles[first + 1].at = roles.roles[second + 1].at.clone();
    roles.roles[second + 1].at = held;
    assert_ne!(roles, facts);
    assert_eq!(judge(&dynamic, &roles), expected);

    // The branches of a join complete in no authored order.
    let mut joins = facts.clone();
    let held = joins.joins[0].at.clone();
    joins.joins[0].at = joins.joins[2].at.clone();
    joins.joins[2].at = held;
    assert_ne!(joins, facts);
    assert_eq!(judge(&dynamic, &joins), expected);

    // Two sends to a channel enter its queue in either order.
    let mut channels = facts.clone();
    let held = channels.channels[0].at.clone();
    channels.channels[0].at = channels.channels[1].at.clone();
    channels.channels[1].at = held;
    assert_ne!(channels, facts);
    assert_eq!(judge(&dynamic, &channels), expected);
}

#[test]
#[trace("TC-208", "FR-208-AC-3")]
fn an_order_the_declarations_do_not_permit_is_reported_and_not_admitted_as_equal() {
    let dynamic = Dynamic::with_finite_capacities();
    let facts = conforming(&dynamic);
    let expected = judge(&dynamic, &facts);
    // r0 acts before it is spawned.
    let mut swapped = facts.clone();
    let held = swapped.roles[0].at.clone();
    swapped.roles[0].at = swapped.roles[1].at.clone();
    swapped.roles[1].at = held;
    let result = judge(&dynamic, &swapped);
    assert_ne!(result, expected);
    assert_eq!(result.withheld(), [DynamicAxis::Roles]);
    assert_eq!(
        result.verdict(DynamicAxis::Roles),
        AxisVerdict::NonConformant
    );
}

// ---------------------------------------------------------------------------
// FR-209: adverse observations
// ---------------------------------------------------------------------------

#[test]
#[trace("TC-208", "FR-209-AC-1")]
fn an_adverse_observation_names_the_one_axis_it_violates() {
    let dynamic = Dynamic::with_finite_capacities();
    let cases = [
        (
            history(&dynamic, 4, 2, &["a", "b", "c"], 3),
            DynamicAxis::Roles,
        ),
        (
            history(&dynamic, 2, 3, &["a", "b", "c"], 3),
            DynamicAxis::Channels,
        ),
        (history(&dynamic, 2, 2, &["a", "b"], 3), DynamicAxis::Joins),
        (
            history(&dynamic, 2, 2, &["a", "b", "c"], 4),
            DynamicAxis::Loops,
        ),
    ];
    for (facts, axis) in cases {
        let result = judge(&dynamic, &facts);
        assert_eq!(result.withheld(), [axis]);
        assert_eq!(result.verdict(axis), AxisVerdict::NonConformant);
        assert_eq!(result.truth(), Truth::Violated);
    }
    let holds = judge(&dynamic, &conforming(&dynamic));
    assert!(holds.withheld().is_empty(), "{:?}", holds.verdicts());
    assert_eq!(holds.truth(), Truth::Satisfied);
}

#[test]
#[trace("TC-208", "FR-209-AC-2")]
fn an_observation_of_an_undeclared_subject_is_unexpected_on_its_axis_only() {
    let dynamic = Dynamic::with_finite_capacities();
    let mut facts = conforming(&dynamic);
    // A role node offered as a channel: the protocol declares no such channel.
    let role = dynamic.named("role", "R");
    facts.channels.push(ChannelFact {
        at: at(100),
        channel: role.clone(),
        event: ChannelEvent::Deliver {
            message: message("ghost"),
        },
    });
    let result = judge(&dynamic, &facts);
    assert_eq!(
        result.verdict(DynamicAxis::Channels),
        AxisVerdict::Unexpected(HistoryRefusal::UndeclaredSubject { node: role })
    );
    for axis in [DynamicAxis::Roles, DynamicAxis::Joins, DynamicAxis::Loops] {
        assert_eq!(result.verdict(axis), AxisVerdict::Conformant);
    }
    assert_eq!(result.truth(), Truth::Violated);
}

#[test]
#[trace("TC-208", "FR-209-AC-3")]
fn delivery_out_of_send_order_is_non_conformant_on_the_channel_axis_only() {
    let dynamic = Dynamic::with_finite_capacities();
    let head = dynamic.named("channel", "Head");
    let fact = |position, event| ChannelFact {
        at: at(position),
        channel: head.clone(),
        event,
    };
    let enqueue = |name: &str| ChannelEvent::Enqueue {
        message: message(name),
        key: Box::from(&b"k1"[..]),
    };
    let deliver = |name: &str| ChannelEvent::Deliver {
        message: message(name),
    };
    let facts = HistoryFacts {
        channels: vec![
            fact(1, enqueue("m1")),
            fact(2, enqueue("m2")),
            fact(3, deliver("m2")),
            fact(4, deliver("m1")),
        ],
        ..HistoryFacts::default()
    };
    let result = judge(&dynamic, &facts);
    assert_eq!(result.withheld(), [DynamicAxis::Channels]);
    assert_eq!(
        result.verdict(DynamicAxis::Channels),
        AxisVerdict::NonConformant
    );
    let Ok(outcomes) = result.channels() else {
        panic!("the channels are assessed");
    };
    let violations: Vec<_> = outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            ChannelOutcome::Assessed(channel) => Some(&channel.violations),
            ChannelOutcome::Unsupported { .. } | ChannelOutcome::RequiresBound { .. } => None,
        })
        .flatten()
        .collect();
    assert!(
        matches!(violations.as_slice(), [QueueViolation::Order { .. }]),
        "{violations:?}"
    );
    for axis in [DynamicAxis::Roles, DynamicAxis::Joins, DynamicAxis::Loops] {
        assert_eq!(result.verdict(axis), AxisVerdict::Conformant);
    }
}

#[test]
#[trace("TC-208", "FR-209-AC-4")]
fn an_unsupported_axis_is_unavailable_and_a_violation_outranks_it() {
    // The full choreography declares an unordered channel losing its head,
    // which FSL does not step, so its channel axis is unsupported for any
    // history.
    let dynamic = Dynamic::with("workflow", 3);
    let unsupported = judge(&dynamic, &conforming(&dynamic));
    assert_eq!(
        unsupported.verdict(DynamicAxis::Channels),
        AxisVerdict::Unsupported
    );
    for axis in [DynamicAxis::Roles, DynamicAxis::Joins, DynamicAxis::Loops] {
        assert_eq!(unsupported.verdict(axis), AxisVerdict::Conformant);
    }
    assert_eq!(unsupported.truth(), Truth::Unavailable);
    // The same unsupported channel axis beside a role breach is violated.
    let breached = judge(&dynamic, &history(&dynamic, 4, 2, &["a", "b", "c"], 3));
    assert_eq!(
        breached.verdict(DynamicAxis::Roles),
        AxisVerdict::NonConformant
    );
    assert_eq!(
        breached.verdict(DynamicAxis::Channels),
        AxisVerdict::Unsupported
    );
    assert_eq!(breached.truth(), Truth::Violated);
}

#[test]
#[trace("TC-208", "FR-209-AC-4")]
fn a_channel_of_symbolic_capacity_is_requires_bound_not_unsupported() {
    // Only the channel of symbolic capacity is left unjudged: its bound is a
    // subject binding FSL admits, which a history assessment is not given.
    let dynamic = Dynamic::with_symbolic_capacity_only();
    let result = judge(&dynamic, &conforming(&dynamic));
    assert_eq!(
        result.verdict(DynamicAxis::Channels),
        AxisVerdict::RequiresBound
    );
    for axis in [DynamicAxis::Roles, DynamicAxis::Joins, DynamicAxis::Loops] {
        assert_eq!(result.verdict(axis), AxisVerdict::Conformant);
    }
    assert_eq!(result.truth(), Truth::Unavailable);
    // A violation on another axis outranks it.
    let breached = judge(&dynamic, &history(&dynamic, 4, 2, &["a", "b", "c"], 3));
    assert_eq!(
        breached.verdict(DynamicAxis::Roles),
        AxisVerdict::NonConformant
    );
    assert_eq!(
        breached.verdict(DynamicAxis::Channels),
        AxisVerdict::RequiresBound
    );
    assert_eq!(breached.truth(), Truth::Violated);
}

#[test]
#[trace("TC-208", "FR-209-AC-5")]
fn a_violation_names_the_declaration_it_breaches_by_checked_node() {
    let dynamic = Dynamic::with_finite_capacities();
    let result = judge(&dynamic, &history(&dynamic, 2, 3, &["a", "b", "c"], 3));
    let Ok(outcomes) = result.channels() else {
        panic!("assessed");
    };
    let reject = dynamic.named("channel", "Reject");
    let violations: Vec<_> = outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            ChannelOutcome::Assessed(channel) => Some(&channel.violations),
            ChannelOutcome::Unsupported { .. } | ChannelOutcome::RequiresBound { .. } => None,
        })
        .flatten()
        .collect();
    assert_eq!(
        violations,
        [&QueueViolation::Capacity {
            channel: reject,
            capacity: 2,
            occupancy: 3,
        }]
    );
    let roles = judge(&dynamic, &history(&dynamic, 4, 2, &["a", "b", "c"], 3));
    let Ok(assessed) = roles.roles() else {
        panic!("assessed");
    };
    let role = dynamic.named("role", "R");
    assert!(assessed
        .iter()
        .any(|found| found.role == role && found.population_violation.is_some()));
}

#[test]
#[trace("TC-208", "FR-209-AC-6")]
fn only_a_breach_of_the_declarations_is_adverse_and_an_unreadable_fact_is_unavailable() {
    let dynamic = Dynamic::with_finite_capacities();
    // A workflow identity that is not text cannot be keyed as an occurrence.
    let join = dynamic.named("control", "JoinAll");
    let unreadable = HistoryFacts {
        joins: vec![JoinFact {
            at: at(1),
            join,
            workflow: filament_protocol::ids::WorkflowInstanceId::new(&[0xff], "ff"),
            repeat_ordinals: Vec::new(),
            event: JoinEvent::Settled,
        }],
        ..HistoryFacts::default()
    };
    let result = judge(&dynamic, &unreadable);
    assert!(matches!(
        result.verdict(DynamicAxis::Joins),
        AxisVerdict::Unreadable(HistoryRefusal::WorkflowNotText { .. })
    ));
    assert_eq!(result.truth(), Truth::Unavailable);
    // A lifecycle event of a static role breaches the declaration.
    let role = dynamic.named("role", "S");
    let spawned = HistoryFacts {
        roles: vec![RoleFact {
            at: at(1),
            role: role.clone(),
            workflow: workflow("w1"),
            instance: instance("s"),
            event: RoleEvent::Spawn,
        }],
        ..HistoryFacts::default()
    };
    let result = judge(&dynamic, &spawned);
    assert_eq!(
        result.verdict(DynamicAxis::Roles),
        AxisVerdict::Unexpected(HistoryRefusal::StaticRoleLifecycle { role })
    );
    assert_eq!(result.truth(), Truth::Violated);
}

// ---------------------------------------------------------------------------
// FR-210: closure axes
// ---------------------------------------------------------------------------

fn closed_state() -> ClosureState {
    ClosureState::new(
        Some(DecisionScopeProgress::of(ScopeState::Closed)),
        Some(DecisionScopeClosure::of(ScopeState::Closed)),
        Some(SurroundingExecutionProgress::of(ScopeState::Closed)),
        Some(SurroundingExecutionClosure::of(ScopeState::Closed)),
        Some(GlobalConformanceClosure::Closed),
    )
    .expect("five axes")
}

fn with_axis(axis: Axis, open: ScopeState, global: GlobalConformanceClosure) -> ClosureState {
    let mut state = closed_state();
    match axis {
        Axis::DecisionScopeProgress => {
            state.decision_scope_progress = DecisionScopeProgress::of(open);
        }
        Axis::DecisionScopeClosure => state.decision_scope_closure = DecisionScopeClosure::of(open),
        Axis::SurroundingExecutionProgress => {
            state.surrounding_execution_progress = SurroundingExecutionProgress::of(open);
        }
        Axis::SurroundingExecutionClosure => {
            state.surrounding_execution_closure = SurroundingExecutionClosure::of(open);
        }
        Axis::CompleteGlobalConformanceClosure => {
            state.complete_global_conformance_closure = global;
        }
    }
    state
}

#[test]
#[trace("TC-208", "FR-210-AC-1")]
fn each_closure_axis_alone_withholds_complete_global_success_and_names_only_itself() {
    let dynamic = Dynamic::with_finite_capacities();
    let holds = judge(&dynamic, &conforming(&dynamic));
    let complete = |state| {
        settle_global(
            &holds,
            ProtocolAssessment::new(state, AssessmentExecution::Completed),
        )
    };
    assert!(complete(closed_state()).complete.is_ok());
    for axis in Axis::ALL {
        let state = with_axis(axis, ScopeState::Open, GlobalConformanceClosure::Open);
        let settled = complete(state);
        let withheld = settled.complete.expect_err("an open axis withholds");
        assert!(withheld.axes.is_empty());
        let named: Vec<_> = withheld.closure.iter().map(|r| r.axis()).collect();
        assert_eq!(named, [Some(axis)], "{axis:?}");
        // The history's own truth is not moved by any closure state.
        assert_eq!(settled.truth, Truth::Satisfied);
    }
    // The fifth axis has its own vocabulary: every member but `closed`
    // withholds, `not-required` included.
    for global in GlobalConformanceClosure::ALL {
        let settled = complete(with_axis(
            Axis::CompleteGlobalConformanceClosure,
            ScopeState::Closed,
            global,
        ));
        assert_eq!(
            settled.complete.is_ok(),
            global == GlobalConformanceClosure::Closed,
            "{global:?}"
        );
    }
}

#[test]
#[trace("TC-208", "FR-210-AC-2")]
fn a_non_completing_execution_cannot_become_complete_global_success() {
    let dynamic = Dynamic::with_finite_capacities();
    let holds = judge(&dynamic, &conforming(&dynamic));
    for execution in AssessmentExecution::NON_COMPLETING {
        let settled = settle_global(&holds, ProtocolAssessment::new(closed_state(), execution));
        let withheld = settled.complete.expect_err("the execution withholds");
        assert_eq!(
            withheld.closure,
            execution.refusal().into_iter().collect::<Vec<_>>()
        );
        assert!(!withheld.closure.is_empty());
    }
}

#[test]
#[trace("TC-208", "FR-210-AC-3")]
fn closing_the_decision_scope_neither_closes_the_rest_nor_establishes_conformance() {
    let dynamic = Dynamic::with_finite_capacities();
    let holds = judge(&dynamic, &conforming(&dynamic));
    let state = ClosureState::new(
        Some(DecisionScopeProgress::of(ScopeState::Closed)),
        Some(DecisionScopeClosure::of(ScopeState::Closed)),
        Some(SurroundingExecutionProgress::of(ScopeState::Open)),
        Some(SurroundingExecutionClosure::of(ScopeState::Open)),
        Some(GlobalConformanceClosure::Open),
    )
    .expect("five axes");
    let settled = settle_global(
        &holds,
        ProtocolAssessment::new(state, AssessmentExecution::Completed),
    );
    let withheld = settled.complete.expect_err("withheld");
    let named: Vec<_> = withheld.closure.iter().filter_map(|r| r.axis()).collect();
    assert_eq!(
        named,
        [
            Axis::SurroundingExecutionProgress,
            Axis::SurroundingExecutionClosure,
            Axis::CompleteGlobalConformanceClosure
        ]
    );
    assert!(withheld.closure.iter().all(|refusal| !matches!(
        refusal,
        ClosureRefusal::DecisionScopeProgressNotClosed(_)
            | ClosureRefusal::DecisionScopeNotClosed(_)
    )));
}

#[test]
#[trace("TC-208", "FR-210-AC-4")]
fn closed_axes_do_not_make_a_non_conformant_history_conform() {
    let dynamic = Dynamic::with_finite_capacities();
    let fails = judge(&dynamic, &faulty(&dynamic));
    let settled = settle_global(
        &fails,
        ProtocolAssessment::new(closed_state(), AssessmentExecution::Completed),
    );
    assert_eq!(settled.truth, Truth::Violated);
    let withheld = settled
        .complete
        .expect_err("a violated history is not success");
    assert_eq!(withheld.axes, fails.withheld());
    assert!(withheld.closure.is_empty());
}

#[test]
#[trace("TC-208", "FR-210-AC-5")]
fn the_closure_state_leaves_the_histories_truth_unchanged() {
    let dynamic = Dynamic::with_finite_capacities();
    for facts in [conforming(&dynamic), faulty(&dynamic)] {
        let result = judge(&dynamic, &facts);
        let truths: Vec<_> = Axis::ALL
            .into_iter()
            .map(|axis| {
                let state = with_axis(axis, ScopeState::Open, GlobalConformanceClosure::Incomplete);
                settle_global(
                    &result,
                    ProtocolAssessment::new(state, AssessmentExecution::Completed),
                )
                .truth
            })
            .chain([settle_global(
                &result,
                ProtocolAssessment::new(closed_state(), AssessmentExecution::Completed),
            )
            .truth])
            .collect();
        assert!(truths.iter().all(|truth| *truth == result.truth()));
    }
}

#[test]
#[trace("TC-208", "FR-210-AC-6")]
fn an_absent_closure_axis_is_refused_and_never_defaulted() {
    let refusals = ClosureState::new(None, None, None, None, None).expect_err("all omitted");
    assert_eq!(
        refusals,
        Axis::ALL.map(ClosureRefusal::AxisOmitted).to_vec()
    );
    let one = ClosureState::new(
        Some(DecisionScopeProgress::of(ScopeState::Closed)),
        Some(DecisionScopeClosure::of(ScopeState::Closed)),
        Some(SurroundingExecutionProgress::of(ScopeState::Closed)),
        None,
        Some(GlobalConformanceClosure::Closed),
    )
    .expect_err("one omitted");
    assert_eq!(
        one,
        [ClosureRefusal::AxisOmitted(
            Axis::SurroundingExecutionClosure
        )]
    );
}

// ---------------------------------------------------------------------------
// FR-211: batch and incremental agreement
// ---------------------------------------------------------------------------

/// The facts at positions after `after` and up to `through`, on every axis: a
/// cut of the history by position, which falls inside the facts of an axis.
fn cut(facts: &HistoryFacts, after: u64, through: u64) -> HistoryFacts {
    let within = |position: u64| after < position && position <= through;
    HistoryFacts {
        roles: facts
            .roles
            .iter()
            .filter(|f| within(f.at.position))
            .cloned()
            .collect(),
        channels: facts
            .channels
            .iter()
            .filter(|f| within(f.at.position))
            .cloned()
            .collect(),
        joins: facts
            .joins
            .iter()
            .filter(|f| within(f.at.position))
            .cloned()
            .collect(),
        loops: facts
            .loops
            .iter()
            .filter(|f| within(f.at.position))
            .cloned()
            .collect(),
    }
}

/// Admit `facts` in prefixes ending at each position of `ends`, the last
/// through the end of the history, and return the assessment after each.
fn advance_through(
    dynamic: &Dynamic,
    facts: &HistoryFacts,
    ends: &[u64],
) -> Vec<HistoryAssessment> {
    let mut path = IncrementalHistory::new(dynamic.linked().declarations(), |node| {
        dynamic.construct(node)
    });
    let mut after = 0;
    ends.iter()
        .chain([&u64::MAX])
        .map(|end| {
            let assessed = path.advance(cut(facts, after, *end)).expect("extends");
            after = *end;
            assessed
        })
        .collect()
}

#[test]
#[trace("TC-208", "FR-211-AC-1")]
fn batch_and_incremental_assessment_agree_when_prefixes_cut_inside_every_axis() {
    let dynamic = Dynamic::with_finite_capacities();
    // Positions: roles 1-4 (2 workers), channel 5-6, join 7-10, loop 11-14.
    // The cuts at 2, 5 and 8 fall inside the role facts, after the first
    // channel fact and inside the join facts.
    for facts in [conforming(&dynamic), faulty(&dynamic)] {
        let batch = judge(&dynamic, &facts);
        let steps = advance_through(&dynamic, &facts, &[2, 5, 8]);
        let last = steps.last().expect("prefixes");
        assert_eq!(agree(&batch, last), Ok(()));
        assert_eq!(&batch, last);
        // A path that kept only the last prefix instead of appending to what
        // it had admitted would not agree, so the agreement above is the
        // appending of `HistoryFacts::extend`.
        let replaced = judge(&dynamic, &cut(&facts, 8, u64::MAX));
        assert!(agree(&batch, &replaced).is_err());
    }
}

#[test]
#[trace("TC-208", "FR-211-AC-2")]
fn the_incremental_path_settles_a_join_before_the_final_fact_arrives() {
    let dynamic = Dynamic::with_finite_capacities();
    let facts = conforming(&dynamic);
    let batch = judge(&dynamic, &facts);
    // Cuts at 8 (branches a and b done, the join not settled) and 10 (settled).
    let [before, settled, last] = advance_through(&dynamic, &facts, &[8, 10])
        .try_into()
        .expect("three assessments");
    let Ok(open) = before.joins() else {
        panic!("assessed")
    };
    assert!(
        matches!(open.as_slice(), [JoinOutcome::Assessed(join)] if !join.settled),
        "{open:?}"
    );
    let (Ok(joins), Ok(final_joins)) = (settled.joins(), batch.joins()) else {
        panic!("assessed");
    };
    // The loop facts are still to come, yet the join is settled, and as the
    // whole history settles it.
    assert!(
        matches!(joins.as_slice(), [JoinOutcome::Assessed(join)] if join.settled),
        "{joins:?}"
    );
    assert_eq!(joins, final_joins);
    let Ok(loops) = settled.loops() else {
        panic!("assessed")
    };
    assert!(loops.is_empty());
    assert_eq!(agree(&batch, &last), Ok(()));
}

#[test]
#[trace("TC-208", "FR-211-AC-3")]
fn a_divergence_between_the_paths_is_a_refusal_naming_the_axes_that_differ() {
    let dynamic = Dynamic::with_finite_capacities();
    let batch = judge(&dynamic, &conforming(&dynamic));
    // A path that dropped the last loop fact and added a fourth role instance.
    let mut skewed = conforming(&dynamic);
    skewed.loops.pop();
    skewed.roles.extend(history(&dynamic, 4, 0, &[], 0).roles);
    let incremental = judge(&dynamic, &skewed);
    let divergence = agree(&batch, &incremental).expect_err("they differ");
    assert_eq!(divergence.axes, [DynamicAxis::Roles, DynamicAxis::Loops]);
    assert_eq!(agree(&batch, &batch), Ok(()));
}

#[test]
#[trace("TC-208", "FR-211-AC-4")]
fn a_prefix_that_does_not_extend_the_history_is_refused_and_admits_nothing() {
    let dynamic = Dynamic::with_finite_capacities();
    let facts = conforming(&dynamic);
    let mut path = IncrementalHistory::new(dynamic.linked().declarations(), |node| {
        dynamic.construct(node)
    });
    path.advance(cut(&facts, 0, 8)).expect("extends");
    // Positions 7 and 8 are already admitted; 9 and 10 are after them.
    let refused = path.advance(cut(&facts, 6, 10)).expect_err("overlaps");
    assert_eq!(
        refused,
        PrefixRefusal::NotExtending {
            admitted: at(8),
            offered: at(7),
        }
    );
    // Nothing was admitted: the next true extension still agrees with batch.
    let last = path.advance(cut(&facts, 8, u64::MAX)).expect("extends");
    assert_eq!(last, judge(&dynamic, &facts));
}
