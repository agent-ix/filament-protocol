// SPDX-License-Identifier: AGPL-3.0-or-later
//! TC-174: send, receipt, message, delivery and attempt identities stay
//! distinct from one another and from the effect identity, over identical
//! producer bytes, and none of them establishes a business effect
//! (FR-023-AC-3, FR-023-AC-4).

use std::collections::BTreeSet;
use std::num::NonZeroU32;

use filament_protocol::compensation::algebra::{
    AttemptFact, AttemptLedger, AttemptOutcome, Definition, ForwardFact, Limits, Registry,
};
use filament_protocol::compensation::purity::{Capability, CapabilityInventory, PureAssessment};
use filament_protocol::ids::{
    AttemptId, DefinitionId, DeliveryId, EffectId, IdentityKind, MessageId, ReceiveId, SendId,
};
use ix_trace_rs::trace;

struct Pure;

impl CapabilityInventory for Pure {
    fn capabilities(&self) -> &[Capability] {
        &[]
    }
}

fn pure() -> PureAssessment {
    PureAssessment::checked(&Pure).expect("empty capability surface")
}

const SAME: &[u8] = b"same";

#[test]
#[trace("TC-174", "FR-023-AC-3", "FR-023-AC-4")]
fn send_receipt_message_delivery_and_attempt_identities_stay_distinct_and_establish_no_effect() {
    // The same producer bytes under five identity types are five kinds, and
    // an attempt carries the bytes of an effect without being one.
    let (send, receive) = (SendId::new(SAME, "s"), ReceiveId::new(SAME, "r"));
    let (message, delivery) = (MessageId::new(SAME, "m"), DeliveryId::new(SAME, "d"));
    let (attempt, effect) = (AttemptId::new(SAME, "a"), EffectId::new(SAME, "e"));
    let kinds: BTreeSet<IdentityKind> = [
        send.kind(),
        receive.kind(),
        message.kind(),
        delivery.kind(),
        attempt.kind(),
    ]
    .into();
    assert_eq!(kinds.len(), 5);
    assert_ne!(attempt.kind(), effect.kind());
    assert_eq!(attempt.identity_bytes(), effect.identity_bytes());

    // A send, a receive, two deliveries of one message and an attempt, every
    // one carrying the effect's bytes or its own, establish no registration;
    // only the established effect does, once.
    let definition = Definition {
        identity: DefinitionId::new(b"definition:refund", "refund"),
        paired_forward_effect: effect.clone(),
        retry_bound: NonZeroU32::new(4),
    };
    let mut registry = Registry::default();
    for fact in [
        ForwardFact::Send(send),
        ForwardFact::Receive(receive),
        ForwardFact::Delivery(delivery),
        ForwardFact::Delivery(DeliveryId::new(b"redelivered", "d2")),
        ForwardFact::Attempt(attempt.clone()),
    ] {
        let events = registry
            .observe(pure(), std::slice::from_ref(&definition), &fact)
            .expect("bounded");
        assert!(events.is_empty(), "{fact:?} establishes no effect");
    }
    let events = registry
        .observe(
            pure(),
            std::slice::from_ref(&definition),
            &ForwardFact::EstablishedEffect(effect.clone()),
        )
        .expect("bounded");
    assert_eq!(events.len(), 1);
    let registration = events[0].registration.clone();

    // Two attempts, one of them with the effect's bytes, are two attempts and
    // establish one effect; a failed attempt establishes none.
    let mut ledger = AttemptLedger::new(registration.clone(), Limits::owner_max()).expect("ledger");
    for (identity, established, outcome) in [
        (attempt, None, AttemptOutcome::Failed),
        (
            AttemptId::new(b"retry", "retry"),
            Some(effect),
            AttemptOutcome::Successful,
        ),
    ] {
        ledger.admit(
            pure(),
            AttemptFact {
                attempt: identity,
                registration: registration.identity.clone(),
                established_effect: established,
                outcome: Some(outcome),
            },
        );
    }
    assert_eq!(ledger.attempts().len(), 2);
    assert_eq!(ledger.effects().len(), 1);
}
