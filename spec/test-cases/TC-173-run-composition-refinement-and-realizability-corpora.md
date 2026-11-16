---
id: TC-173
title: "Run the composition, refinement and realizability corpora over real compiled protocols"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-150
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-200
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-202
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-203
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-204
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-301
    type: verifies
---
# TC-173: Run the composition, refinement and realizability corpora over real compiled protocols

## Description

Verify the V1-PROT-028, V1-PROT-029 and V1-PROT-030 corpora of IT-005 over real protocols written by FSL's checked-handoff writer, compiled back by FSL's `compile` and linked, with no system, subject or guarantee label built by hand.

## Test Procedure

Refinement: take the reference workflow family (`Order`, then `Pick` or `Cancelled`; a narrow variant; a lossy at-most-once variant; a variant with a hidden `Pick` loop; a variant with a replicated role) and decide each pair with `refinement::decide` and through `dispatch_claims`: refines, extra trace, deadlock, divergence, unsupported, incomplete. Realizability: settle the family through FSL's real engine: each role distinguishes the branches, a hidden choice, an incompatible merge. Composition: compile three components into separate packages, each declaring channels of its own as guarantee and assumption identities, take each guarantee's label from FSL's settled realizability result of its component, and decide: all premises discharged, an unproved premise, a cycle, an unoffered assumption, and reordered guarantees and subjects. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

Every refinement case reads into the verdict and axes of FR-202-AC-5, carries a replayable counterexample where it is refuted and none where it is unsupported or incomplete. The realizable members settle `satisfied` on `closed-scope`; the others settle `violated` on `decisive-counterexample` with the separating-witness record unchanged. The compositions are proved, unproved, circular and incompatible as stated, the incompatible one with a counterexample that replays, and reordering changes no verdict or counterexample.
