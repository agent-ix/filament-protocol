---
id: FR-203
title: "Refuse circular guarantee dependencies as self-justifying"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-029, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-200, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-176, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-491, type: references }
  - { target: ix://agent-ix/filament-spec-language/FR-530, type: depends_on }
---
# FR-203: Refuse circular guarantee dependencies as self-justifying

## Description

When the declared guarantees of a composition depend on one another in a cycle, the library SHALL refuse the cycle as circular reasoning, making no guarantee of the cycle available to discharge any premise, including a premise on its own cycle.

## Inputs

A composition request (FR-200) whose declared guarantees each name the nodes their proofs used (`ix://agent-ix/filament-specification/FR-176` "Discharge fixed point", `ix://agent-ix/filament-specification/FR-491`).

## Outputs

The composition disposition listing every dependency cycle as a strongly connected component of guarantees in node order, read as execution `refused`, truth `unavailable` and basis `unavailable`, with no guarantee of a cycle in the discharge record; or one refusal of that disposition under FR-030.

## Behavior

- The library shall find the dependency cycles as the strongly connected components of the declared guarantees, a guarantee that uses itself included.
- The library shall make no guarantee on a cycle available to discharge any premise, whatever FSL's label for it is.
- The library shall make no guarantee whose proof used an unavailable guarantee available either.
- The library shall refuse a disposition that offers a guarantee of a cycle as an assumption (FR-030), naming that guarantee.
- The library shall compute no iteration, lattice element or least fixed point, because no profile that declares one is admitted here.
- The library shall emit no success for a composition with a cycle (FR-040).
- The library shall name a composition with a cycle `circular` before any counterexample or undischarged premise, retaining those findings beside it.
- The library shall leave every other claim's disposition unchanged (FR-029).

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-203-AC-1 | Given two guarantees whose proofs use one another, the composition lists the two as one cycle in node order, reads as circular, and carries no success value; a guarantee that uses itself is a cycle of one. | Test (TC-203) |
| FR-203-AC-2 | Given a premise whose discharger is a `proved` guarantee on a cycle, the premise stays undischarged with the cause `in-cycle` and the discharge record holds no cycle guarantee. | Test (TC-203) |
| FR-203-AC-3 | Offering a guarantee of a cycle as an assumption refuses the disposition naming that guarantee. | Test (TC-203) |
| FR-203-AC-4 | Given a proved guarantee whose proof used a cycle member, its premise stays undischarged as dependent on the unavailable member and the disposition is not success. | Test (TC-203) |
| FR-203-AC-5 | Given the cyclic claim beside an acyclic composition claim, the acyclic disposition equals the one returned without the cyclic claim. | Test (TC-203) |
| FR-203-AC-6 | Given a composition whose guarantees are acyclic, the disposition lists no cycle, and the result does not depend on the order of the declared guarantees. | Test (TC-203) |
| FR-203-AC-7 | A composition with an unrelied cycle and a proof that rests on an unoffered assumption has verdict `circular` and still lists the counterexample. | Test (TC-203) |

## Dependencies

- [FR-200](./FR-200-decide-composition-claims.md) supplies the composition decision the cycle is part of.
- [FR-029](./FR-029-return-one-disposition-per-requested-claim.md) supplies the independence of the other claims and [FR-040](./FR-040-emit-no-success-beside-fatal-refusal.md) the no-success rule.
- `ix://agent-ix/filament-specification/FR-176` and `ix://agent-ix/filament-specification/FR-491` define the cycle; the library accepts no fixed-point rule.
