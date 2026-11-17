---
id: TC-205
title: "Decide finite global conformance with FSL's protocol checker over compiled protocols"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-205
    type: verifies
---
# TC-205: Decide finite global conformance with FSL's protocol checker over compiled protocols

## Description

Verify V1-PROT-026 of IT-005 and the requires-bound refusal of V1-PROT-014 over protocols written by FSL's checked-handoff writer, compiled back by FSL's `compile` and linked, with the protocol subject built by FSL's `protocol_subject` and no result, record or counterexample built by hand.

## Test Procedure

Register FSL's EN-1 descriptor and the provider that makes FSL's two calls (`check_protocol`, `settle_protocol_check`) at a provider boundary. Dispatch a `global-conformance` claim over a deadlock-free reference protocol and read the record. Run the same item through FSL's two calls directly and compare. Dispatch the claim over a protocol whose ship branch sends twice on a blocking channel of capacity one that nothing receives, with a replay that refuses. Dispatch it at an empty boundary. Dispatch it over the recipe model as FSL ships it, whose `Total` and `Amount` are unbounded quantity roots, with no bounding rewrite, read the checker's refusal by calling it directly, and compare. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

The deadlock-free protocol settles `satisfied` on `closed-scope`, label `proved`, no counterexample. The dispatched record equals the direct record in value, label, basis, truth and limits. The deadlocked protocol, whose counterexample FSL cannot replay, settles neither `violated` nor `satisfied` and carries no counterexample. The empty boundary settles the claim `unsupported` with a warning naming `temporal-satisfaction`. The unbounded model is refused by the checker as requiring a bound, and the dispatch carries that refusal as the backend's failure with the checker's own text, no record and no truth.
