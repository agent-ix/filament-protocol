---
id: TC-174
title: "Keep send, receipt, message, delivery and attempt identities distinct"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-023
    type: verifies
---
# TC-174: Keep send, receipt, message, delivery and attempt identities distinct

## Description

Verify V1-PROT-006 in one test: the send, receive, message, delivery and attempt identities of one exchange stay distinct from one another and from the effect identity, and none of them registers a business effect.

## Test Procedure

Build a send, a receive, a message, a delivery and an attempt identity from the same producer bytes, and an effect identity from those bytes too. Observe a send, a receive, two deliveries of the one message and an attempt as forward facts against a definition paired to the effect, then the established effect. Admit a failed attempt that establishes nothing and a successful retry that establishes the effect to an attempt ledger. Tag the test with the exact acceptance criteria it verifies.

## Expected Results

The five identity types report five kinds and the attempt reports a kind other than the effect's though its bytes are the effect's. The send, receive, deliveries and attempt register nothing; only the established effect registers, once. The ledger keeps both attempts and reports one effect.
