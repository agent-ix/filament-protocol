---
id: TC-177
title: "Link a choice composed with other controls"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-052
    type: verifies
---
# TC-177: Link a choice composed with other controls

## Description

Verify V1-PROT-009 composition over a protocol written by FSL's checked-handoff writer, compiled back and linked: a choice inside a `parallel` branch, inside a `repeat` body and inside another choice's case.

## Test Procedure

Link a protocol whose `Seller` owns four choices: one in a parallel branch, one in a loop body, and an outer choice with an inner choice in its `right` case; one case guard is contradictory. Read each linked choice's declaration and compare its owner, visible set and key with FSL's `choice_visibility`, and its branch labels with FSL's control reader. Check the nesting with FSL's control reader and causal edges. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

Four choices are linked, each with its own control, owner, visible node and branch labels in label order; each is a child of the control that authored it; the contradictory case is still a linked branch, because the library evaluates no guard.
