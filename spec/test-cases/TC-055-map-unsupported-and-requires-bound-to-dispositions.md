---
id: TC-055
title: "Map FSL Unsupported and requires-bound outcomes to dispositions"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-057
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-058
    type: verifies
---
# TC-055: Map FSL Unsupported and requires-bound outcomes to dispositions

## Description

Verify map FSL Unsupported and requires-bound outcomes to dispositions over a checked package and an admitted observation history, with no exploration of the protocol.

## Test Procedure

Feed each FSL Unsupported cause ADR-010 lists, an unknown cause, and requires-bound outcomes naming a Population, a Collection and a Loop domain, alongside independently supported claims. Inspect the arguments of the FSL calls made. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

Each disposition is typed non-success with FSL's name unchanged and the construct's node id; independent dispositions and axes are unchanged; no bound is chosen and none is passed.
