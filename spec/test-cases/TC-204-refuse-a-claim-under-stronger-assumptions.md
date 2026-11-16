---
id: TC-204
title: "Refuse a claim established under assumptions stronger than the request's"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-204
    type: verifies
---
# TC-204: Refuse a claim established under assumptions stronger than the request's

## Description

Verify that a refinement whose system rests on an assumption the request did not offer is refuted with a replayable finding, and that a composition resting on an unoffered assumption yields a paired replayable counterexample.

## Test Procedure

Decide refinements, over protocols written by FSL's checked-handoff writer that require an `infinite-trace` temporal clause, whose implementation or specification rests on offered, unoffered and unused assumptions, with and without an extra trace, beside other claims. Decide compositions whose relied-on proof reaches an unoffered assumption directly, through a sibling, in reversed order and through an unproved or cycle-member guarantee, replay each counterexample, and alter its steps, premise and offered side. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

Each unoffered assumption yields one finding naming its system; offered-but-unused assumptions change nothing; the findings replay and an altered one does not; the composition counterexample is unchanged by order.
