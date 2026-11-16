---
id: FR-208
title: "Assess a history to the same result in every order the declarations permit"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-053, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-054, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-055, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-056, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-059, type: references }
---
# FR-208: Assess a history to the same result in every order the declarations permit

## Description

When one finite observed history is presented in an order the authored declarations permit, the library SHALL assess it to the same result on the role, channel, join and loop axes as in every other permitted order.

This requirement succeeds FR-026, retired with the evaluator (FSL-1549).

## Inputs

The linked protocol's declarations and one finite observed history, as the facts of each axis (FR-053 to FR-056).

## Outputs

One [`HistoryAssessment`]: the four axis assessments, each as the assessment of its axis returned it.

## Behavior

- The library shall read the facts of each axis in the history's own order, by observation position and identity, so the order a caller presents them in changes no value.
- The library shall derive no causal edge from a timestamp, an arrival position, an ingestion order or a key order.
- The library shall not reject a permitted order in order to obtain a determinate result.
- When two observations are ordered by the declarations (an instance's spawn before its acts, a channel's send order within a key), the library shall report an order that reverses them and not treat it as equal.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-208-AC-1 | Presenting each axis's facts in the opposite order gives the same assessment, for a conforming and for a faulty history, and the two histories assess differently from each other. | Test (TC-208) |
| FR-208-AC-2 | Exchanging the positions of independent facts (two role instances' spawns and acts, two branch completions of a join, two sends to a channel) gives the same assessment. | Test (TC-208) |
| FR-208-AC-3 | An order the declarations do not permit (an instance acting before it is spawned) is reported on its axis and the assessment differs from the permitted order's. | Test (TC-208) |
| FR-208-AC-4 | No timestamp, arrival position, ingestion order or key order is read as a causal edge. | Inspection |

## Dependencies

- [FR-053](./FR-053-assess-role-lifetime-and-population.md) to [FR-056](./FR-056-assess-loop-iteration-bound.md) assess one axis each.
- The module is `history`; the assessments are `dynamics`'.
