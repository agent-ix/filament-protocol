// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-175: over a protocol written by FSL's checked-handoff writer, compiled
//! back and linked, every channel end is a declared role, a send's and a
//! receive's own payload binder carries the type its linked channel carries,
//! and a choice's branches carry their authored labels (FR-051-AC-6,
//! FR-051-AC-7, FR-052-AC-6). A channel end that is no declared role is
//! refused.

mod support {
    pub mod dynamic;
    pub mod handoff;
    pub mod projection;
    pub mod realizability;
}

use filament_contract_model::{
    CheckedNodeId, CheckedPackageDispatchResult, CheckedPackageRefusalCode, CheckedPackageV2,
};
use filament_protocol::link::link;
use fsl_package::protocol_view::{control_body, ControlBody};
use ix_trace_rs::trace;
use serde_json::Value;
use support::projection::{worked, CANCELLED, PICK};
use support::realizability::Realizability;

/// The payload binder node of the `send` or `receive` control `control`, and
/// its channel.
fn payload_of<'p>(
    package: &'p CheckedPackageV2,
    control: &CheckedNodeId,
) -> (&'p CheckedNodeId, &'p CheckedNodeId) {
    let node = package
        .graph()
        .nodes
        .iter()
        .find(|node| &node.node_id == control)
        .expect("the control node");
    match control_body(node).expect("FSL reads the control") {
        ControlBody::Send { channel, binder }
        | ControlBody::Receive {
            channel, binder, ..
        } => (channel, binder),
        other => panic!("not a send or receive: {other:?}"),
    }
}

#[test]
#[trace("TC-175", "FR-051-AC-6")]
fn every_channel_end_is_a_declared_role_of_the_protocol() {
    let dynamic = worked(PICK, CANCELLED);
    let declarations = dynamic.linked().declarations();
    for (channel, sender, receiver) in [
        ("Order", "Buyer", "Seller"),
        ("Pick", "Seller", "Warehouse"),
        ("Cancelled", "Seller", "Warehouse"),
    ] {
        let declared = declarations
            .channel(&dynamic.named("channel", channel))
            .expect("a declared channel");
        for (end, name) in [(&declared.sender, sender), (&declared.receiver, receiver)] {
            let role = declarations
                .role(end)
                .unwrap_or_else(|| panic!("{channel}: the end is a declared role"));
            assert_eq!(role.node, dynamic.named("role", name), "{channel}");
        }
    }
    assert_ne!(
        declarations
            .channel(&dynamic.named("channel", "Order"))
            .map(|channel| &channel.receiver),
        declarations
            .channel(&dynamic.named("channel", "Pick"))
            .map(|channel| &channel.receiver),
        "distinct routes bind distinct roles"
    );
}

/// Point the argument `name` of every node that has all of `with` at the node
/// the same node's argument `to` names.
fn retarget(wire: &mut Value, name: &str, to: &str, with: &[&str]) {
    let nodes = wire["semantic_graph"]["nodes"]
        .as_array_mut()
        .expect("nodes");
    let mut changed = 0;
    for node in nodes {
        let Some(arguments) = node["body"]["arguments"].as_array_mut() else {
            continue;
        };
        let target_of = |arguments: &[Value], wanted: &str| {
            arguments
                .iter()
                .find(|argument| argument["name"].as_str() == Some(wanted))
                .map(|argument| argument["value"]["target"].clone())
        };
        if !with
            .iter()
            .all(|wanted| target_of(arguments, wanted).is_some())
        {
            continue;
        }
        let destination = target_of(arguments, to).expect("the destination argument");
        for argument in arguments
            .iter_mut()
            .filter(|argument| argument["name"].as_str() == Some(name))
        {
            argument["value"]["target"] = destination.clone();
        }
        changed += 1;
    }
    assert!(changed > 0, "no node carries {with:?}");
}

#[test]
#[trace("TC-175", "FR-051-AC-6")]
fn a_channel_end_that_is_no_declared_role_is_refused_before_it_can_be_linked() {
    let fixture = Realizability::worked(PICK, CANCELLED);
    // Either end repointed at the channel's message type node: a node that is
    // no role. Contract IR's reader refuses the graph, so no channel with an
    // unbound end is linked; the unedited package links.
    for end in ["from", "to"] {
        let outcome =
            fixture.read_mutated(|wire| retarget(wire, end, "carries", &["from", "to", "carries"]));
        assert!(
            matches!(
                &outcome,
                CheckedPackageDispatchResult::Refused(refusal)
                    if refusal.code == CheckedPackageRefusalCode::InvalidSemanticGraph
            ),
            "{end}: {outcome:?}"
        );
    }
    assert!(link(fixture.package(), fixture.clause()).is_ok());
}

#[test]
#[trace("TC-175", "FR-051-AC-7")]
fn each_sends_and_receives_own_payload_type_is_the_type_its_channel_carries() {
    let dynamic = worked(PICK, CANCELLED);
    let declarations = dynamic.linked().declarations();
    let mut seen = Vec::new();
    for control in dynamic.linked().controls() {
        let node = dynamic
            .package()
            .graph()
            .nodes
            .iter()
            .find(|node| &node.node_id == control)
            .expect("a control node");
        if !matches!(
            control_body(node),
            Ok(ControlBody::Send { .. } | ControlBody::Receive { .. })
        ) {
            continue;
        }
        let (channel, binder) = payload_of(dynamic.package(), control);
        let declared = declarations.channel(channel).expect("a linked channel");
        let payload = dynamic
            .package()
            .graph()
            .nodes
            .iter()
            .find(|node| &node.node_id == binder)
            .expect("the payload binder node");
        assert_eq!(
            payload.semantic_type, declared.message_type,
            "{control:?}: the payload binder's type is the channel's"
        );
        seen.push((channel.clone(), declared.message_type.clone()));
    }
    // Order, Pick and Cancelled each carry a send and a receive.
    assert_eq!(seen.len(), 6);
    seen.sort();
    seen.dedup();
    assert_eq!(
        seen.len(),
        3,
        "three channels, three distinct payload types"
    );
    let types: std::collections::BTreeSet<_> = seen.iter().map(|(_, ty)| ty).collect();
    assert_eq!(types.len(), 3);
}

#[test]
#[trace("TC-175", "FR-052-AC-6")]
fn a_choice_carries_one_declared_branch_per_authored_label() {
    let dynamic = worked(PICK, CANCELLED);
    let [choice] = dynamic.linked().declarations().choices() else {
        panic!("one choice");
    };
    let node = dynamic
        .package()
        .graph()
        .nodes
        .iter()
        .find(|node| node.node_id == choice.control)
        .expect("the choice node");
    let ControlBody::Choice { owner, cases } = control_body(node).expect("FSL reads the choice")
    else {
        panic!("a choice body");
    };
    assert_eq!(owner, &choice.owner);
    let labels: Vec<&str> = cases.iter().map(|(label, _)| *label).collect();
    assert_eq!(labels, ["no", "yes"], "the authored labels, in label order");
    for (label, body) in &cases {
        assert!(
            dynamic.linked().controls().contains(body),
            "the `{label}` branch body is a control of the linked protocol"
        );
    }
    let bodies: std::collections::BTreeSet<_> = node_bodies(&cases);
    assert_eq!(bodies.len(), 2, "each label has a branch of its own");
}

fn node_bodies<'n>(
    cases: &[(&'n str, &'n CheckedNodeId)],
) -> std::collections::BTreeSet<&'n CheckedNodeId> {
    cases.iter().map(|(_, body)| *body).collect()
}
