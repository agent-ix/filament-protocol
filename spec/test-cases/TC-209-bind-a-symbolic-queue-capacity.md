---
id: TC-209
title: "Bind a symbolic queue capacity in the protocol subject and carry FSL's outcome"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-058
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-150
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-205
    type: verifies
---
# TC-209: Bind a symbolic queue capacity in the protocol subject and carry FSL's outcome

## Description

Verify that a `symbolic(<name>)` queue capacity is bound through FSL's own `CapacityBinding` in the request's protocol subject, and that FSL's decision, requires-bound outcome or `invalid_runtime_input` refusal is carried unchanged (FSpec FR-785), over protocols written by FSL's checked-handoff writer and dispatched to FSL's real engines.

## Test Procedure

Dispatch a `global-conformance` claim to FSL's explicit-state checker over a protocol whose ship branch sends twice on a blocking channel nothing receives from, the channel's capacity written `symbolic(Bound)`: with no binding, with `Bound` bound to one and to two, with a binding naming no capacity and with `Bound` bound twice; call the checker directly in each case and compare. Dispatch the same claim over the recipe model as FSL ships it with the symbolic channel added. Dispatch a `realizability` claim to FSL's protocol analysis engine over the same protocol under the `bounded-static` profile (bound and unbound) and under the `bounded` profile with no binding, with `Bound` bound (compared with the capacity written as a literal) and with a binding naming no capacity. Dispatch a `finite-replay` claim over a supplied trace of the same protocol, which takes no binding. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

With no binding the checker reports the unbound capacity and the dispatch settles the requires-bound disposition carrying FSL's list (name and channel) unchanged; beside an unbounded root FSL's one requires-bound names the capacity with the domains and the disposition carries that outcome unchanged. With `Bound` at one FSL finds the deadlock and at two it does not, as with the capacity written as a literal, and the dispatched record equals the checker's own. A binding naming no capacity and a repeated binding settle FSL's `invalid_runtime_input` refusal with causes `unknown-member` and `duplicate-member` unchanged. The `realizability` claim under `bounded-static` settles FSL's `unsupported` outcome with cause `symbolic-queue-capacity` naming `Bound`, bound or not. Under `bounded` it settles FSL's `inconclusive` outcome with cause `premise-not-stated` naming the member `capacities` when unbound, and FSL decides it as it decides the literal capacity when bound. FSL's `unknown-member` refusal is carried for the unknown name. The `finite-replay` claim over the symbolic capacity settles the disposition FSL's analyze gives (`unsupported_projection`, cause `unbounded-extent`), bound or not, since FSL's replay request takes no binding.
