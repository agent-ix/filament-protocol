// SPDX-License-Identifier: AGPL-3.0-or-later
//! The composition requests of TC-200 to TC-204. A composition is decided by
//! the library, so no backend is registered.

// Each test binary uses a different part of these helpers.
#![allow(dead_code)]

use std::convert::Infallible;

use filament_contract_model::CheckedNodeId;
use filament_protocol::assume_guarantee::{AssumeGuaranteeRequest, PremiseLists};
use filament_protocol::composition::Guarantee;
use filament_protocol::dispatch::{
    Binding, ClaimDisposition, ClaimRequest, Fragment, ProviderBoundary,
};
use fsl_replay::planner::ClauseForm;
use fsl_replay::ItemLabel;

use super::dynamic::Dynamic;

pub type Entry = CheckedNodeId;
pub type Req = AssumeGuaranteeRequest<Entry>;
/// A boundary that serves no backend: composition and refinement ask none.
pub type Boundary = ProviderBoundary<'static, Binding<()>, Infallible>;
pub type Claims = ClaimRequest<&'static str, (), Entry>;
pub type Settled = ClaimDisposition<&'static str, (), Entry, Infallible>;

/// A request over the roles `S` and `R` as component subjects, with the
/// channels `Head` and `Tail` as assumption identities and a distinct entry in
/// each of the four premise lists.
pub fn request(dynamic: &Dynamic) -> Req {
    Req {
        fragment: Fragment {
            channels: vec![dynamic.named("channel", "Head")],
            roles: vec![dynamic.named("role", "S")],
        },
        subjects: vec![dynamic.named("role", "S"), dynamic.named("role", "R")],
        assumptions: vec![
            dynamic.named("channel", "Head"),
            dynamic.named("channel", "Tail"),
        ],
        premises: PremiseLists {
            environment: vec![dynamic.named("channel", "Head")],
            relies: vec![dynamic.named("channel", "Reject")],
            liveness_dependencies: vec![dynamic.named("channel", "New")],
            fairness: vec![dynamic.named("channel", "Tail")],
        },
        guarantees: vec![
            guarantee(dynamic, "Reject", ClauseForm::Safety, &["Head"]),
            guarantee(dynamic, "New", ClauseForm::Liveness, &["Reject", "Tail"]),
        ],
        sources: vec![dynamic.clause().clone()],
    }
}

/// A `proved` guarantee at the channel `name` whose proof used the channels
/// `uses`.
pub fn guarantee(dynamic: &Dynamic, name: &str, form: ClauseForm, uses: &[&str]) -> Guarantee {
    Guarantee {
        id: dynamic.named("channel", name),
        form,
        label: ItemLabel::Proved,
        uses: uses
            .iter()
            .map(|used| dynamic.named("channel", used))
            .collect(),
    }
}

/// A node of the checked package that its protocol does not declare.
pub fn foreign(dynamic: &Dynamic) -> CheckedNodeId {
    let declarations = dynamic.linked().declarations();
    dynamic
        .package()
        .graph()
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .find(|node| !declarations.declares(node))
        .expect("a node the protocol does not declare")
}
