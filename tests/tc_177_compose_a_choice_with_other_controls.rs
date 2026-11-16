// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-177: a choice composed with other controls, over a protocol written by
//! FSL's checked-handoff writer, compiled back and linked. A choice inside a
//! `parallel` branch, inside a `repeat` body and inside another choice's case
//! is linked with its own owner, visible set and branch labels, exactly as
//! FSL's control reader and `choice_visibility` publish them; the library
//! evaluates no guard (FR-052-AC-7).

mod support {
    pub mod dynamic;
    pub mod handoff;
    pub mod projection;
}

use filament_contract_model::CheckedNodeId;
use filament_protocol::declarations::ChoiceDeclaration;
use fsl_package::protocol_view::{choice_visibility, control_body, ControlBody, EdgeKind};
use ix_trace_rs::trace;
use support::dynamic::Dynamic;
use support::handoff::MONEY_UNIT;
use support::projection::{MEMBERS, PICK, RECORDS};

/// Four choices: `InBranch` in a `parallel` branch, owned by `Warehouse`
/// and seeing `not q.ok`; `InLoop` in a `repeat` body, and `Outer` with `Inner`
/// in its `right` case, each owned by `Seller` and seeing `got.ok`. The
/// owners and visible nodes differ, so a library that reused one choice's
/// owner or visible set for another would fail. Only the `again` case of
/// `InLoop` has a guard that can never hold (`got.ok and not got.ok`): a
/// library that evaluated guards would drop that branch. `Inner`'s guards are
/// complementary.
fn composed() -> Dynamic {
    Dynamic::from_source(&format!(
        "language \"ix:native\" edition \"1-draft\";\n\
         profile pp = \"filament.protocol.finite-global/v1\";\n\
         model M = \"agent-ix/checked-handoff\";\n{MONEY_UNIT}\
         {RECORDS}protocol Flow using pp over (input: M::Workflow) on origin {{\n{MEMBERS}\n\
         run sequence Main {{\n\
         send Placed via Order as (placed: Ack) {{ true }};\n\
         receive Got via Order of Placed as (got: Ack) {{ true }};\n\
         {PICK}\n\
         parallel Fan memory sc {{\n\
           branch a sequence BranchA {{\n\
             choice InBranch by Warehouse visible (not q.ok) {{\n\
               case alpha when {{ not q.ok }} sequence AlphaBody {{ }}\n\
               case beta when {{ not (not q.ok) }} sequence BetaBody {{ }}\n\
             }} }}\n\
           branch b sequence BranchB {{ }}\n\
         }} join all [a,b];\n\
         repeat Again by Seller visible (got.ok) max 2 while {{ got.ok }} sequence AgainBody {{\n\
           event Tick by Seller as (tick: Ack) {{ true }};\n\
           choice InLoop by Seller visible (got.ok) {{\n\
             case again when {{ got.ok and not got.ok }} sequence AgainCase {{ }}\n\
             case stop when {{ true }} sequence StopCase {{ }}\n\
           }} }} exhausted sequence AgainOut {{ }}\n\
         choice Outer by Seller visible (got.ok) {{\n\
           case left when {{ not got.ok }} sequence LeftBody {{ }}\n\
           case right when {{ got.ok }} sequence RightBody {{\n\
             choice Inner by Seller visible (got.ok) {{\n\
               case no when {{ not got.ok }} sequence InnerNo {{ }}\n\
               case yes when {{ got.ok }} sequence InnerYes {{ }}\n\
             }} }}\n\
         }} }}\n\
         finish Done as (zz: Boolean) {{ true }};\n}}\n"
    ))
}

fn choice_named<'d>(dynamic: &'d Dynamic, name: &str) -> &'d ChoiceDeclaration {
    let control = dynamic.named("control", name);
    dynamic
        .linked()
        .declarations()
        .choices()
        .iter()
        .find(|choice| choice.control == control)
        .unwrap_or_else(|| panic!("`{name}` is a linked choice"))
}

/// The branch labels of the choice `choice`, as FSL's control reader gives them.
fn labels(dynamic: &Dynamic, choice: &ChoiceDeclaration) -> Vec<String> {
    let node = dynamic
        .package()
        .graph()
        .nodes
        .iter()
        .find(|node| node.node_id == choice.control)
        .expect("the choice node");
    let ControlBody::Choice { owner, cases } = control_body(node).expect("a choice body") else {
        panic!("not a choice");
    };
    assert_eq!(owner, &choice.owner);
    for (label, body) in &cases {
        assert!(
            dynamic.linked().controls().contains(body),
            "the `{label}` body is a control of the linked protocol"
        );
    }
    cases.iter().map(|(label, _)| (*label).to_owned()).collect()
}

fn children_of(dynamic: &Dynamic, sequence: &CheckedNodeId) -> Vec<CheckedNodeId> {
    let node = dynamic
        .package()
        .graph()
        .nodes
        .iter()
        .find(|node| &node.node_id == sequence)
        .expect("a control node");
    match control_body(node) {
        Ok(ControlBody::Sequence { children }) => children.into_iter().cloned().collect(),
        _ => Vec::new(),
    }
}

/// The bodies a control enters: a choice's case bodies, a parallel's branch
/// bodies, or the targets of a repeat's progress edges.
fn bodies_below(dynamic: &Dynamic, control: &CheckedNodeId) -> Vec<CheckedNodeId> {
    let node = dynamic
        .package()
        .graph()
        .nodes
        .iter()
        .find(|node| &node.node_id == control)
        .expect("a control node");
    match control_body(node).expect("a control body") {
        ControlBody::Choice { cases, .. } => cases.into_iter().map(|(_, b)| b.clone()).collect(),
        ControlBody::Parallel { branches, .. } => {
            branches.into_iter().map(|(_, b)| b.clone()).collect()
        }
        _ => dynamic
            .linked()
            .edges()
            .iter()
            .filter(|edge| edge.kind == EdgeKind::RepeatProgress && &edge.from.node == control)
            .map(|edge| edge.to.node.clone())
            .collect(),
    }
}

fn nests(dynamic: &Dynamic, outer: &str, inner: &str) -> bool {
    let inner = dynamic.named("control", inner);
    bodies_below(dynamic, &dynamic.named("control", outer))
        .iter()
        .any(|body| children_of(dynamic, body).contains(&inner))
}

#[test]
#[trace("TC-177", "FR-052-AC-7")]
fn a_choice_inside_a_parallel_branch_a_loop_body_and_a_choice_is_linked_with_its_own_structure() {
    let dynamic = composed();
    let declarations = dynamic.linked().declarations();
    assert_eq!(declarations.choices().len(), 4, "every composed choice");
    // The composition is the authored one.
    assert!(nests(&dynamic, "Fan", "InBranch"), "in a parallel branch");
    assert!(nests(&dynamic, "Again", "InLoop"), "in a repeat body");
    assert!(
        nests(&dynamic, "Outer", "Inner"),
        "in another choice's case"
    );
    for (name, owner, expected) in [
        ("InBranch", "Warehouse", vec!["alpha", "beta"]),
        ("InLoop", "Seller", vec!["again", "stop"]),
        ("Outer", "Seller", vec!["left", "right"]),
        ("Inner", "Seller", vec!["no", "yes"]),
    ] {
        let choice = choice_named(&dynamic, name);
        assert_eq!(
            choice.owner,
            dynamic.named("role", owner),
            "{name}: the owner"
        );
        assert_eq!(labels(&dynamic, choice), expected, "{name}: branch labels");
        // The visible set is what FSL's `choice_visibility` reads, per choice.
        let node = dynamic
            .package()
            .graph()
            .nodes
            .iter()
            .find(|node| node.node_id == choice.control)
            .expect("the choice node");
        let evidence = choice_visibility(node).expect("FSL reads the choice");
        assert_eq!(&choice.control, evidence.choice, "{name}");
        assert_eq!(
            choice.visible.iter().collect::<Vec<_>>(),
            evidence.visible,
            "{name}: the visible nodes in FSL's order"
        );
        assert_eq!(choice.key, dynamic.key(&choice.control), "{name}: the key");
    }
    // `InBranch` has its own owner and its own visible node: neither is
    // merged with or shadowed by the `Seller` choices'. FSL shares the one
    // `got.ok` node among the others.
    let branch = choice_named(&dynamic, "InBranch");
    let outer = choice_named(&dynamic, "Outer");
    assert_ne!(branch.owner, outer.owner);
    assert_ne!(branch.visible, outer.visible);
    assert_eq!(
        choice_named(&dynamic, "Inner").visible,
        outer.visible,
        "choices that name the same fact see the same node"
    );
    // Four distinct controls: the composition merges and shadows none.
    let controls: std::collections::BTreeSet<_> = declarations
        .choices()
        .iter()
        .map(|choice| &choice.control)
        .collect();
    assert_eq!(controls.len(), 4);
}

#[test]
#[trace("TC-177", "FR-052-AC-7")]
fn linking_a_composed_choice_evaluates_no_guard_so_a_contradictory_case_is_still_a_branch() {
    let dynamic = composed();
    // `InLoop`'s `again` guard is `got.ok and not got.ok`, never true: the
    // linked choice still carries both labels, and the loop beside it keeps
    // its authored maximum, as declared.
    let choice = choice_named(&dynamic, "InLoop");
    assert_eq!(labels(&dynamic, choice), ["again", "stop"]);
    let [again] = dynamic.linked().declarations().loops() else {
        panic!("one loop");
    };
    assert_eq!(again.control, dynamic.named("control", "Again"));
    assert_eq!(again.maximum, Some(2));
}
