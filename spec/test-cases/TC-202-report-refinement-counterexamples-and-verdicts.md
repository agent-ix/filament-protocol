---
id: TC-202
title: "Report refinement counterexamples and read verdicts into the axes"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-202
    type: verifies
---
# TC-202: Report refinement counterexamples and read verdicts into the axes

## Description

Verify the extra-trace, deadlock and divergence counterexamples, the cases that are no fault, the reading of each verdict, the order independence and precedence, and that each counterexample replays and an altered one does not.

## Test Procedure

Decide refinements with an extra trace, a deadlock, a divergence, a stop in a final state, two stuck systems and a shared loop. Reverse every list of a request with all three faults. Alter each counterexample and replay it. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

Each fault yields its paired counterexample; no-fault cases refine; each verdict reads into its stated axes; findings are unchanged by reordering; unsupported outranks refuted outranks incomplete; an altered counterexample fails the replay.
