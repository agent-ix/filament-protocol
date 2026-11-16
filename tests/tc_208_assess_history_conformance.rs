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
    IncrementalHistory,
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
    let before = facts.clone();
    let result = judge(&dynamic, &facts);
    assert_eq!(
        result.verdict(DynamicAxis::Channels),
        AxisVerdict::Unexpected(HistoryRefusal::UndeclaredSubject { node: role })
    );
    for axis in [DynamicAxis::Roles, DynamicAxis::Joins, DynamicAxis::Loops] {
        assert_eq!(result.verdict(axis), AxisVerdict::Conformant);
    }
    assert_eq!(result.truth(), Truth::Violated);
    // The history is judged where it stands, not repaired.
    assert_eq!(facts, before);
}

#[test]
#[trace("TC-208", "FR-209-AC-3")]
fn delivery_out_of_send_order_and_over_capacity_are_non_conformant_on_the_channel_axis() {
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
    let Ok(outcomes) = result.channels() else {
        panic!("the channels are assessed");
    };
    let violations: Vec<_> = outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            ChannelOutcome::Assessed(channel) => Some(&channel.violations),
            ChannelOutcome::Unsupported { .. } => None,
        })
        .flatten()
        .collect();
    assert!(
        matches!(violations.as_slice(), [QueueViolation::Order { .. }]),
        "{violations:?}"
    );
    // The over-capacity case is FR-209-AC-1's channel fault above.
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

/// The history cut into three admitted prefixes by position: the roles and the
/// channel, then the join, then the loop.
fn prefixes(facts: &HistoryFacts) -> [HistoryFacts; 3] {
    [
        HistoryFacts {
            roles: facts.roles.clone(),
            channels: facts.channels.clone(),
            ..HistoryFacts::default()
        },
        HistoryFacts {
            joins: facts.joins.clone(),
            ..HistoryFacts::default()
        },
        HistoryFacts {
            loops: facts.loops.clone(),
            ..HistoryFacts::default()
        },
    ]
}

#[test]
#[trace("TC-208", "FR-211-AC-1")]
fn batch_and_incremental_assessment_agree_on_conforming_and_faulty_histories() {
    let dynamic = Dynamic::with_finite_capacities();
    for facts in [conforming(&dynamic), faulty(&dynamic)] {
        let batch = judge(&dynamic, &facts);
        let mut path = IncrementalHistory::new(dynamic.linked().declarations(), |node| {
            dynamic.construct(node)
        });
        let last = prefixes(&facts)
            .into_iter()
            .map(|prefix| path.advance(prefix))
            .last()
            .expect("three prefixes");
        assert_eq!(agree(&batch, &last), Ok(()));
        assert_eq!(batch, last);
    }
}

#[test]
#[trace("TC-208", "FR-211-AC-2")]
fn the_incremental_path_settles_a_join_before_the_final_fact_arrives() {
    let dynamic = Dynamic::with_finite_capacities();
    let facts = conforming(&dynamic);
    let batch = judge(&dynamic, &facts);
    let mut path = IncrementalHistory::new(dynamic.linked().declarations(), |node| {
        dynamic.construct(node)
    });
    let [first, second, third] = prefixes(&facts);
    let early = path.advance(first);
    let Ok(joins) = early.joins() else {
        panic!("assessed");
    };
    assert!(joins.is_empty(), "no join fact has arrived yet");
    let mid = path.advance(second);
    // The loop facts are still to come, yet the join is settled, and as the
    // whole history settles it.
    let (Ok(settled), Ok(final_joins)) = (mid.joins(), batch.joins()) else {
        panic!("assessed");
    };
    assert!(
        matches!(settled.as_slice(), [JoinOutcome::Assessed(join)] if join.settled),
        "{settled:?}"
    );
    assert_eq!(settled, final_joins);
    let Ok(loops) = mid.loops() else {
        panic!("assessed")
    };
    assert!(loops.is_empty());
    let last = path.advance(third);
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
