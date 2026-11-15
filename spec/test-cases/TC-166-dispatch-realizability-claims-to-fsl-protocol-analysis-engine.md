---
id: TC-166
title: "Dispatch realizability claims to FSL's protocol analysis engine"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-150
    type: verifies
---
# TC-166: Dispatch realizability claims to FSL's protocol analysis engine

## Description

Verify that a `realizability` claim is decided by FSL's real `protocol-analysis` engine registered at the provider boundary, with no stand-in, and that the engine's result and separating-witness record are carried unchanged.

## Test Procedure

Write a handoff directory for a three-role protocol over the recipe model with FSL's writer, compile it back with FSL's `compile`, build the subject with `protocol_subject`, and register FSL's engine under FSL's descriptor. Dispatch a `realizability` claim for a protocol whose roles can tell every branch apart and for one with a hidden choice. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

The realizable protocol settles `satisfied` on `closed-scope`; the hidden choice settles `violated` on `decisive-counterexample` and carries FSL's separating-witness record unchanged. The selected backend is `protocol-analysis`.
