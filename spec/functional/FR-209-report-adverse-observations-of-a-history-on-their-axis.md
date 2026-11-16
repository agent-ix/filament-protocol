---
id: FR-209
title: "Report adverse observations of a history against the axis they violate"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-208, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-053, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-054, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-055, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-056, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-059, type: references }
---
# FR-209: Report adverse observations of a history against the axis they violate

## Description

When an observation of an admitted history violates the linked protocol's declarations, the library SHALL settle the history `violated` and name the dynamic axis the observation violates.

This requirement succeeds FR-027, retired with the evaluator (FSL-1549). The adverse kinds it reports are the ones the live axes model: a role lifetime or population breach, an over-capacity queue, an overflow differing from the policy, a delivery out of send order, a loss or duplication the premise forbids, a join settling before its threshold, a loop past its maximum, and an observation of a subject the protocol does not declare.

## Inputs

The declarations and one finite observed history (FR-208).

## Outputs

A verdict for each of the four axes (`conformant`, `non-conformant`, `unsupported`, `unexpected`) and the history's truth.

## Behavior

- The library shall settle a history `violated` when any axis is non-conformant or holds an unexpected observation.
- The library shall settle a history `unavailable` and not `violated` or `satisfied` when an axis is unsupported (FR-057) and none is adverse.
- The library shall name the axis and the exact declaration the observation violates, by checked node identity.
- The library shall report an observation of an undeclared subject as `unexpected` on its axis, carrying the node it named.
- The library shall leave the admitted facts unaltered: nothing is repaired, reordered, merged or discarded in order to report it.
- The library shall leave every other axis's verdict as it would be without the adverse observation.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-209-AC-1 | A history that exceeds a role's population bound, a queue's capacity, a join's threshold or a loop's maximum is `non-conformant` on exactly that axis, `conformant` on the other three, and `violated`; the conforming history is `satisfied`. | Test (TC-208) |
| FR-209-AC-2 | A fact naming a node the protocol does not declare as that kind of subject is `unexpected` on its axis, carrying the node; the other axes stay `conformant`, the history is `violated` and the facts are unaltered. | Test (TC-208) |
| FR-209-AC-3 | A delivery out of send order within an ordered channel's key is a queue order violation, `non-conformant` on the channel axis only. | Test (TC-208) |

## Dependencies

- [FR-208](./FR-208-admit-every-permitted-observation-order-of-a-history.md) supplies the assessment.
