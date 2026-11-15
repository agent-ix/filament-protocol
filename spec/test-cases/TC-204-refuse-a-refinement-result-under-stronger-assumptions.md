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

Verify that a refinement result retaining an assumption the request did not state is refused, and that a composition resting on an unoffered assumption yields a paired replayable counterexample.

## Test Procedure

Feed stand-in refinement results retaining extra, fewer, omitted and the backend's own strengthened assumptions. Decide compositions whose relied-on proof reaches an unoffered assumption directly and through a sibling guarantee, replay each counterexample, alter its steps, premise and offered side, and decide one that uses only offered assumptions. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

Extra identities refuse the refinement naming each; the composition is incompatible with a counterexample naming premise, offered side and path; the counterexample replays and an altered one does not; offered-but-unused assumptions change nothing.
