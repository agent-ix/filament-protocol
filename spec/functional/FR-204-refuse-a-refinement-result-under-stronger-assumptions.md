---
id: FR-204
title: "Refuse a claim established under assumptions stronger than the request's"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-008, type: references }
  - { target: ix://agent-ix/filament-protocol/FR-029, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-030, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-040, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-200, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-202, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-301, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-177, type: depends_on }
  - { target: ix://agent-ix/filament-spec-language/FR-148, type: references }
---
# FR-204: Refuse a claim established under assumptions stronger than the request's

## Description

When a claim would rest on an assumption identity absent from the request's assumptions, the library SHALL NOT settle it as a success: it refuses a refinement result that retains such an identity, and returns a paired replayable counterexample for a composition whose relied-on proof reaches one.

## Inputs

A requested `refinement` claim (FR-301) with its assumption identities and states (FR-030-AC-4), and the backend's returned result with the assumption identities it retains. A requested `composition` claim (FR-200) with its offered assumptions, its declared guarantees and the nodes each proof used.

## Outputs

The disposition carried under FR-202 when every retained assumption identity is among the request's, or one refusal of that disposition under FR-030 naming each extra assumption identity. For a composition, the counterexample: the premise, the offered assumptions and the proof path to the first unoffered node.

## Behavior

- When the proof of a guarantee a premise relies on, through any chain of sibling guarantees, used a node that is neither a declared guarantee nor an offered assumption, the library shall return the composition as incompatible with a counterexample for that premise.
- The library shall find the counterexample for every relied-on guarantee before judging that guarantee's label or availability, by the shortest proof path with ties broken by node order, so that it does not depend on the order of the declared guarantees or of the nodes a proof used, and so that a relied-on guarantee that is itself not `proved`, or reaches the unoffered node through an unproved or cycle-member guarantee, still yields it.
- The library shall name an incompatible composition after a circular one (FR-203) and before an unproved one.
- The library shall build the counterexample as a pair: the offered side, the request's assumptions, and the relied side, the path of proof edges from the relied-on guarantee to the first unoffered node.
- The library shall let the counterexample be replayed against the request: the premise is a `relies` or `liveness_dependencies` premise, the path starts at that premise's discharger and the discharger's form matches the list, each step is a declared proof edge, the steps chain, no guarantee repeats on the path, the offered side is the request's, and the path ends at a node that is neither a declared guarantee nor offered.
- The library shall reuse no FSL witness type for this counterexample, because FSL's assumption witness is state-based and the composition has no state.
- The library shall read an incompatible composition as `completed`, `violated`, `decisive-witness` with no success value (FR-040).
- The library shall leave a premise whose proof reaches only offered assumptions out of any counterexample, whether or not the request offers assumptions no proof used.
- The library shall compare the returned result's retained assumption identities with the request's by exact identity.
- When a retained assumption identity is absent from the request's assumptions, the library shall refuse the refinement disposition with the cause `identity-mismatch` naming that identity.
- When the returned result omits its retained assumption identities, the library shall refuse the refinement disposition with the cause `identity-mismatch` naming the absent field, because absence compares only to absence (FR-030-AC-4).
- When every retained assumption identity is among the request's, the library shall carry the result, whether or not the request holds assumptions the result did not use.
- When the refused disposition would have been success, the library shall emit no success value for it (FR-040).
- The library shall leave every independent disposition unchanged (FR-029).
- The library shall not replace the refused result with a fallback backend's (FR-030).
- When the backend itself reports a strengthened assumption as a non-success finding, the library shall carry that finding unchanged under FR-202 and not refuse it.

The library decides inclusion of identities only. Whether one assumption entails another, and whether an implementation's assumptions are no stronger than the specification's, is the relation `ix://agent-ix/filament-specification/FR-177` defines and FSL's refinement backend decides. This requirement stops a result from being attributed to a request it was not established under.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-204-AC-1 | Given a stand-in result retaining exactly the request's assumption identities, the disposition is carried unchanged. | Test (TC-204) |
| FR-204-AC-2 | Given a stand-in result that proves refinement and retains one assumption identity absent from the request, the disposition is refused with `identity-mismatch` naming that identity, and carries no success value. | Test (TC-204) |
| FR-204-AC-3 | Given a stand-in result retaining a strict subset of the request's assumption identities, the disposition is carried unchanged. | Test (TC-204) |
| FR-204-AC-4 | Given a stand-in result that reports a violation and retains an extra assumption identity, the disposition is refused with the same cause and the counterexample is not presented as a carried result of the request. | Test (TC-204) |
| FR-204-AC-5 | Given the refused refinement beside a conformance claim and a composition claim, their dispositions equal those returned without the refinement claim, and no other backend is invoked for the refused claim. | Test (TC-204) |
| FR-204-AC-6 | Given a stand-in that itself returns the `protocol-assumption-not-established` cause with a strengthened-assumption witness, the cause and witness appear unchanged and the disposition is not refused. | Test (TC-204) |
| FR-204-AC-7 | Given a stand-in result that omits its retained assumption identities, and another that retains an empty set while the request holds assumptions, the first is refused with `identity-mismatch` naming the absent field and the second is carried as a strict subset. | Test (TC-204) |
| FR-204-AC-8 | Given a composition whose relied-on guarantee used an assumption the request did not offer, directly or through a sibling guarantee, the composition is incompatible, carries a counterexample naming the premise, the offered assumptions and the path to the unoffered node, and is not success. | Test (TC-204) |
| FR-204-AC-9 | The counterexample replays against its request, and altering a step, the premise or the offered side makes the replay fail. | Test (TC-204) |
| FR-204-AC-10 | A composition whose relied-on proofs use only offered assumptions is not incompatible, whether or not the request offers assumptions no proof used. | Test (TC-204) |
| FR-204-AC-11 | The counterexample and the verdict do not change when the declared guarantees or the nodes a proof used are reordered, and a relied-on guarantee that is not `proved`, or that reaches the unoffered node through an unproved or cycle-member guarantee, still yields the counterexample. | Test (TC-204) |
| FR-204-AC-12 | A counterexample repointed to another valid premise, to a `environment` or `fairness` premise, to a path with a repeated guarantee or to a guarantee of the other form fails replay. | Test (TC-204) |

## Dependencies

- [FR-301](./FR-301-admit-protocol-refinement-dispatch.md) supplies the refinement dispatch and the request this requirement checks the result against.
- [FR-202](./FR-202-carry-refinement-results-unchanged.md) supplies the carrying that applies when the result is admitted or is the backend's own finding.
- [FR-030](./FR-030-bind-each-disposition-to-its-exact-premises.md) supplies the refusal of a disposition whose assumption component is substituted, and the no-fallback rule.
- [FR-029](./FR-029-return-one-disposition-per-requested-claim.md) and [FR-040](./FR-040-emit-no-success-beside-fatal-refusal.md) supply independence and the no-success-beside-refusal rule.
- [FR-008](./FR-008-return-typed-refusals.md) supplies the closed cause vocabulary that holds `identity-mismatch`.
- [FR-200](./FR-200-decide-composition-claims.md) supplies the composition decision the counterexample belongs to.
- `ix://agent-ix/filament-specification/FR-177` owns the assumption-weakening relation; FSL FR-148 owns the backend's strengthened-assumption finding.
