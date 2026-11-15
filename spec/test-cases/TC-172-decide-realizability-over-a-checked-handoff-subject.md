---
id: TC-172
title: "Decide realizability over a subject built from a checked-handoff directory alone"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-150
    type: verifies
---
# TC-172: Decide realizability over a subject built from a checked-handoff directory alone

## Description

Verify that FSL's real `protocol-analysis` engine decides `realizability` claims when the subject comes only from a checked-handoff directory FSL's writer wrote, with no front end driven by this repository.

## Test Procedure

Write a handoff directory for a three-role protocol over the recipe model with FSL's writer, compile it back with FSL's `compile`, build the snapshot from the compiled handoff's domain package documents, and build the subject with FSL's `protocol_subject` from the compiled handoff's checked package, documents and emission ceiling. Dispatch a `realizability` claim for a protocol whose roles can tell every branch apart and for one with an incompatible merge. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

The realizable protocol settles `satisfied` on `closed-scope`; the incompatible merge settles `violated` on `decisive-counterexample` and carries FSL's separating-witness record unchanged.
