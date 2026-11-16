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

This requirement succeeds FR-027, retired with the evaluator (FSL-1549). The adverse kinds it reports are the ones the live axes model: a role lifetime or population breach, an over-capacity queue, an overflow differing from the policy, a delivery out of send order, a loss or duplication the premise forbids, a join settling before its threshold, a loop past its maximum, an observation of a subject or a join branch the protocol does not declare, and a lifecycle event of a static role. A fact that cannot be read as an occurrence at all (a workflow identity that is not text, an occurrence FSL's key refuses) breaches no declaration and is not adverse: its axis is unavailable.

## Inputs

The declarations and one finite observed history (FR-208).

## Outputs

A verdict for each of the four axes (`conformant`, `non-conformant`, `unsupported`, `unexpected`, `unreadable`) and the history's truth.

## Behavior

- The library shall settle a history `violated` when any axis is non-conformant or holds an unexpected observation.
- The library shall settle a history `unavailable` and not `violated` or `satisfied` when an axis is unsupported (FR-057) and none is adverse.
- The library shall name the axis and the exact declaration the observation violates, by checked node identity.
- The library shall report an observation of an undeclared subject or join branch, or a lifecycle event of a static role, as `unexpected` on its axis, carrying the refusal that names the node.
- The library shall report a fact it cannot read as an occurrence as `unreadable` on its axis, settling the history `unavailable` and not `violated`.
- The library shall repair, reorder, merge or discard no observation in order to report it.
- The library shall leave every other axis's verdict as it would be without the adverse observation.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-209-AC-1 | A history that exceeds a role's population bound, a queue's capacity, a join's threshold or a loop's maximum is `non-conformant` on exactly that axis, `conformant` on the other three, and `violated`; the conforming history is `satisfied`. | Test (TC-208) |
| FR-209-AC-2 | A fact naming a node the protocol does not declare as that kind of subject is `unexpected` on its axis, carrying the node; the other axes stay `conformant` and the history is `violated`. | Test (TC-208) |
| FR-209-AC-3 | A delivery out of send order within an ordered channel's key is a queue order violation, `non-conformant` on the channel axis only. | Test (TC-208) |
| FR-209-AC-4 | An axis FSL does not step (a channel of symbolic capacity) is `unsupported`; with no adverse axis the history is `unavailable`, and with an adverse axis it is `violated`. | Test (TC-208) |
| FR-209-AC-5 | A violation names the declaration it breaches by its checked node: a capacity violation carries the channel node, the authored capacity and the occupancy, and a population violation is carried on the assessment of the role node. | Test (TC-208) |
| FR-209-AC-6 | A workflow identity that is not text is `unreadable` on its axis and the history is `unavailable`; a lifecycle event of a static role is `unexpected` and the history is `violated`. | Test (TC-208) |

## Dependencies

- [FR-208](./FR-208-admit-every-permitted-observation-order-of-a-history.md) supplies the assessment.
