---
id: FR-204
title: "Refuse a claim established under assumptions stronger than the request's"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-029, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-030, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-040, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-200, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-202, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-301, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-177, type: depends_on }
---
# FR-204: Refuse a claim established under assumptions stronger than the request's

## Description

When a claim would rest on an assumption identity absent from the request's assumptions, the library SHALL NOT settle it as a success: it returns a paired replayable counterexample for a composition whose relied-on proof reaches one and a stronger-assumption finding for a refinement whose system rests on one.

## Inputs

A requested `refinement` claim (FR-301) with its offered assumption identities and the assumptions each of its two systems rests on. A requested `composition` claim (FR-200) with its offered assumptions, its declared guarantees and the nodes each proof used.

## Outputs

For a refinement, one stronger-assumption finding per assumption a system rests on that the request did not offer, naming the system, the assumption and the offered assumptions. For a composition, the counterexample: the premise, the offered assumptions and the proof path to the first unoffered node.

## Behavior

- When the proof of a guarantee a premise relies on, through any chain of sibling guarantees, used a node that is neither a declared guarantee nor an offered assumption, the library shall return the composition as incompatible with a counterexample for that premise.
- The library shall find the counterexample for every relied-on guarantee before judging that guarantee's label or availability, by the shortest proof path with ties broken by node order, so that it does not depend on the order of the declared guarantees or of the nodes a proof used, and so that a relied-on guarantee that is itself not `proved`, or reaches the unoffered node through an unproved or cycle-member guarantee, still yields it.
- The library shall name an incompatible composition after a circular one (FR-203) and before an unproved one.
- The library shall build the counterexample as a pair: the offered side, the request's assumptions, and the relied side, the path of proof edges from the relied-on guarantee to the first unoffered node.
- The library shall let the counterexample be replayed against the request: the premise is a `relies` or `liveness_dependencies` premise, the path starts at that premise's discharger and the discharger's form matches the list, each step is a declared proof edge, the steps chain, no guarantee repeats on the path, the offered side is the request's, and the path ends at a node that is neither a declared guarantee nor offered.
- The library shall reuse no FSL witness type for this counterexample, because FSL's assumption witness is state-based and the composition has no state.
- The library shall read an incompatible composition as `completed`, `violated`, `decisive-witness` with no success value (FR-040).
- The library shall leave a premise whose proof reaches only offered assumptions out of any counterexample, whether or not the request offers assumptions no proof used.
- The library shall compare the assumptions each refinement system rests on with the request's by exact identity, returning a stronger-assumption finding for each one the request did not offer, whichever system rests on it.
- The library shall let a stronger-assumption finding be replayed: the named system rests on the assumption, the request does not offer it, and the offered side is the request's.
- The library shall settle a refinement with a stronger-assumption finding `refuted`, beside any counterexample it also found, with no success value (FR-040).
- The library shall leave every independent disposition unchanged (FR-029).
The library decides inclusion of identities only. Whether one assumption entails another is the relation `ix://agent-ix/filament-specification/FR-177` defines and is not decided here.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-204-AC-1 | Given two systems that rest on exactly the offered assumptions, or on a subset of them, the refinement has no stronger-assumption finding and refines. | Test (TC-204) |
| FR-204-AC-2 | Given an implementation resting on assumptions the request did not offer, the refinement is refuted with one finding per such assumption naming the implementation and the assumption, and each finding replays. | Test (TC-204) |
| FR-204-AC-3 | Given offered assumptions that no system rests on, or no assumptions on either side, the refinement is unchanged and refines. | Test (TC-204) |
| FR-204-AC-4 | Given an implementation that has an extra trace and rests on an unoffered assumption, both the counterexample and the finding are retained and the refinement is refuted. | Test (TC-204) |
| FR-204-AC-5 | Given a refinement with a stronger assumption requested beside a conformance claim and another refinement, their dispositions equal those returned without it. | Test (TC-204) |
| FR-204-AC-6 | Given a specification that rests on an unoffered assumption, the finding names the specification. | Test (TC-204) |
| FR-204-AC-7 | Given a stronger-assumption finding, altering its system, its assumption or its offered side fails the replay, and so does offering the assumption in the request. | Test (TC-204) |
| FR-204-AC-8 | Given a composition whose relied-on guarantee used an assumption the request did not offer, directly or through a sibling guarantee, the composition is incompatible, carries a counterexample naming the premise, the offered assumptions and the path to the unoffered node, and is not success. | Test (TC-204) |
| FR-204-AC-9 | The counterexample replays against its request, and altering a step, the premise or the offered side makes the replay fail. | Test (TC-204) |
| FR-204-AC-10 | A composition whose relied-on proofs use only offered assumptions is not incompatible, whether or not the request offers assumptions no proof used. | Test (TC-204) |
| FR-204-AC-11 | The counterexample and the verdict do not change when the declared guarantees or the nodes a proof used are reordered, and a relied-on guarantee that is not `proved`, or that reaches the unoffered node through an unproved or cycle-member guarantee, still yields the counterexample. | Test (TC-204) |
| FR-204-AC-12 | A counterexample repointed to another valid premise, to a `environment` or `fairness` premise, to a path with a repeated guarantee or to a guarantee of the other form fails replay. | Test (TC-204) |

## Dependencies

- [FR-301](./FR-301-decide-protocol-refinement-claims.md) supplies the refinement decision and the request this requirement checks.
- [FR-202](./FR-202-report-refinement-counterexamples-and-verdicts.md) supplies the reading of the findings into the axes.
- [FR-030](./FR-030-bind-each-disposition-to-its-exact-premises.md) supplies the refusal of a disposition whose assumption component is substituted, and the no-fallback rule.
- [FR-029](./FR-029-return-one-disposition-per-requested-claim.md) and [FR-040](./FR-040-emit-no-success-beside-fatal-refusal.md) supply independence and the no-success-beside-refusal rule.
- [FR-008](./FR-008-return-typed-refusals.md) supplies the closed cause vocabulary that holds `identity-mismatch`.
- [FR-200](./FR-200-decide-composition-claims.md) supplies the composition decision the counterexample belongs to.
- `ix://agent-ix/filament-specification/FR-177` owns the assumption-weakening relation; FSL FR-148 owns the backend's strengthened-assumption finding.
