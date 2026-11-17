---
id: TC-178
title: "Replay a supplied finite trace through FSL's finite-replay engine"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-303
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-207
    type: verifies
---
# TC-178: Replay a supplied finite trace through FSL's finite-replay engine

## Description

Verify that a `finite-replay` claim is decided by FSL's real `protocol-analysis` engine registered at the provider boundary, with no stand-in for the replay, and that FSL's terminal record is carried unchanged or its refusal carried with its code.

## Test Procedure

Write a handoff directory for a three-role protocol over the recipe model with FSL's writer, compile it back with FSL's `compile`, and take the steps of a trace from FSL's own protocol system (transition identity, post-state key, send record) in the trace spelling of FSpec FR-433. Provision the trace document beside the initial state, build FSL's `analyze` request for a `finite-replay` item, register FSL's engine under FSL's descriptor and dispatch the claim for the whole trace, for a trace beginning with a step not enabled at the initial state, and for a trace whose `send` step lacks its `send_record`; dispatch a `finite-replay` claim with a `realizability` request and the reverse; dispatch a trace from an initial state the subject lacks, an item stated `unsupported`, a request for another protocol's clause, and a request FSL's `analyze` refuses to admit; dispatch with an empty registry; and dispatch to a stand-in whose result lacks the terminal record, carries a record that is not `proved`, states another truth or basis than the record, or carries a witness. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

The whole trace settles `satisfied` on `closed-scope` and carries FSL's `proved` record, equal to the record `analyze` returns; the stale and malformed traces are refused with FSL's `invalid_runtime_input` codes and declined records, told apart by their detail, and settle no truth; the unsupported item settles on the typed path; the foreign clause is not run; the unadmitted request fails with FSL's text; FSL's axes give each record kind its truth and basis; a request of another kind is refused `inapplicable-kind`; an empty registry settles `unsupported`; the stand-in results without a coherent record are `unsupported` or refused.
