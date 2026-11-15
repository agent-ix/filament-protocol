---
id: TC-203
title: "Refuse circular guarantee dependencies as self-justifying"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-203
    type: verifies
---
# TC-203: Refuse circular guarantee dependencies as self-justifying

## Description

Verify cycle detection over the declared guarantee dependencies and that no cycle guarantee is available to discharge a premise.

## Test Procedure

Decide compositions whose guarantees use one another, use themselves, are used by a dependent guarantee, or are offered as an assumption; decide an acyclic composition in two declaration orders and beside the cyclic claim. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

The cycle is listed in node order and read as circular; a proved cycle guarantee discharges nothing; a dependent is unavailable; offering a cycle guarantee as an assumption is refused; the acyclic claim is unchanged and order-independent.
