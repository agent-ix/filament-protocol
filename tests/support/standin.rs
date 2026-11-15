// SPDX-License-Identifier: AGPL-3.0-or-later
//! The stand-in backend of TC-160 to TC-163. It is registered at the provider
//! boundary under a descriptor FSL's registry accepts, records every call it
//! receives, and answers from a function of the call. It decides nothing.

// Each test binary uses a different part of these helpers.
#![allow(dead_code)]

use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;

use filament_protocol::dispatch::{
    Answer, BackendResult, Binding, ChannelPremise, Claim, Fragment, Premises, ProtocolClaimKind,
    ProtocolRequest, ProviderBoundary,
};
use filament_protocol::disposition::{FslBoundary, FslReport};
use fsl_foundation::source::provenance::OccurrenceKey;
use fsl_foundation::witness::{
    ObservationIdentity, RuntimeValuePath, ValuePathStep, ValuePathSubject,
};
use fsl_foundation::BackendId;
use fsl_replay::{
    InconclusiveCause, SeparatingWitnessRecord, SettlementBasis, TracePosition, Truth,
};
use fsl_route::{BackendDescriptor, Candidate, Mode, ProviderOrigin};
use fsl_semantics::check::Capability;

use super::dynamic::Dynamic;

/// The channel components FSL's read view does not publish yet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rest {
    pub sender: &'static str,
    pub receiver: &'static str,
    pub message_type: &'static str,
}

pub type Assumptions = &'static str;
pub type Req = ProtocolRequest<Assumptions, Rest, &'static str>;
pub type Bind = Binding<Assumptions, Rest, &'static str>;
/// The unified claim a provider is asked about; these tests ask no
/// composition or refinement, whose entries are therefore `()`.
pub type Asked = Claim<Assumptions, Rest, &'static str, ()>;
pub type Boundary = ProviderBoundary<'static, Asked, Answer<Sentinel, Infallible>>;

/// The result a stand-in returns, with the parts the dispatcher reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sentinel {
    pub label: &'static str,
    pub truth: Truth,
    pub basis: SettlementBasis,
    pub witness: Option<SeparatingWitnessRecord>,
    pub inconclusive: Option<InconclusiveCause>,
    pub artifact: Option<&'static str>,
}

impl Sentinel {
    pub fn new(label: &'static str, truth: Truth, basis: SettlementBasis) -> Self {
        Self {
            label,
            truth,
            basis,
            witness: None,
            inconclusive: None,
            artifact: None,
        }
    }
}

impl BackendResult for Sentinel {
    fn truth(&self) -> Truth {
        self.truth
    }
    fn basis(&self) -> SettlementBasis {
        self.basis
    }
    fn witness(&self) -> Option<&SeparatingWitnessRecord> {
        self.witness.as_ref()
    }
    fn inconclusive(&self) -> Option<&InconclusiveCause> {
        self.inconclusive.as_ref()
    }
}

pub type Reply = Box<dyn Fn(&Bind) -> FslReport<Sentinel>>;

/// The calls a stand-in received.
#[derive(Clone, Default)]
pub struct Calls(Rc<RefCell<Vec<Bind>>>);

impl Calls {
    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }
    pub fn all(&self) -> Vec<Bind> {
        self.0.borrow().clone()
    }
}

pub struct Standin {
    answer: Reply,
    calls: Calls,
}

impl Standin {
    pub fn new(answer: Reply, calls: Calls) -> Self {
        Self { answer, calls }
    }
}

impl FslBoundary<Asked, Answer<Sentinel, Infallible>> for Standin {
    fn report(&mut self, claim: &Asked) -> FslReport<Answer<Sentinel, Infallible>> {
        let Claim::Protocol(bound) = claim else {
            return FslReport::Failed(None);
        };
        self.calls.0.borrow_mut().push(bound.clone());
        (self.answer)(bound).map(Answer::Protocol)
    }
}

pub fn id(name: &str) -> BackendId {
    BackendId::new(name)
}

pub fn candidate(name: &str) -> Candidate {
    Candidate::new(id(name))
}

/// Register a stand-in named `name` serving `kinds`; its calls are the
/// returned record.
pub fn serve(boundary: &mut Boundary, name: &str, kinds: &[Capability], answer: Reply) -> Calls {
    let calls = Calls::default();
    let descriptor = BackendDescriptor::new(
        candidate(name),
        ProviderOrigin::Linked,
        kinds.iter().map(|kind| (*kind, Mode::Unbounded)),
        None,
        None,
    )
    .expect("a descriptor FSL's registry accepts");
    boundary
        .register(
            descriptor,
            Standin {
                answer,
                calls: calls.clone(),
            },
        )
        .expect("the registration is accepted");
    calls
}

/// An answer that returns `result` for every call.
pub fn always(result: Sentinel) -> Reply {
    Box::new(move |_| FslReport::Result(result.clone()))
}

pub fn rest() -> Rest {
    Rest {
        sender: "S",
        receiver: "R",
        message_type: "Notice",
    }
}

/// A request over the fragment of the channels `channels` and roles `S`, `R`,
/// with every premise supplied.
pub fn request(dynamic: &Dynamic, kind: ProtocolClaimKind, channels: &[&str]) -> Req {
    let channel_nodes: Vec<_> = channels
        .iter()
        .map(|name| dynamic.named("channel", name))
        .collect();
    let roles = vec![dynamic.named("role", "S"), dynamic.named("role", "R")];
    Req {
        kind,
        fragment: Fragment {
            channels: channel_nodes.clone(),
            roles: roles.clone(),
        },
        backend: None,
        assumptions: "met",
        premises: Premises {
            channels: channel_nodes
                .into_iter()
                .map(|node| {
                    let declared = dynamic
                        .linked()
                        .declarations()
                        .channel(&node)
                        .expect("a declared channel");
                    let premise = ChannelPremise {
                        ordering: declared.ordering.clone(),
                        delivery: declared.delivery,
                        unread: rest(),
                    };
                    (node, premise)
                })
                .collect(),
            roles: roles.into_iter().map(|node| (node, "internal")).collect(),
        },
        sources: vec![dynamic.clause().clone()],
    }
}

/// A separating-witness record over `dynamic`, with `steps` in its value path.
pub fn witness(dynamic: &Dynamic, steps: Vec<ValuePathStep>) -> SeparatingWitnessRecord {
    let occurrence = OccurrenceKey::generated(dynamic.key(dynamic.clause()));
    SeparatingWitnessRecord {
        quantifier: Some(occurrence.clone()),
        deciding_element: fsl_replay::DecidingElement::Value(filament_exact::Value::Integer(
            filament_exact::Integer::from(600_i64),
        )),
        index: Some(2),
        value_path: RuntimeValuePath {
            observation: ObservationIdentity {
                authority: "fixture".to_owned(),
                identity: "o1".to_owned(),
            },
            subject: ValuePathSubject::Built(occurrence),
            steps,
        },
        trace_position: Some(TracePosition::new("frame-7".to_owned())),
    }
}
