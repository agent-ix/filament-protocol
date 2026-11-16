---
id: FR-211
title: "Agree across batch and incremental assessment of a history"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-208, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-059, type: references }
---
# FR-211: Agree across batch and incremental assessment of a history

## Description

When one history is assessed whole and assessed prefix by prefix as the prefixes arrive, the library SHALL give the same assessment on every axis, with a typed refusal naming the axes returned when the two differ.

This requirement succeeds FR-041, retired with the evaluator (FSL-1549).

## Inputs

One history and the admitted prefixes it arrives in.

## Outputs

The assessment after each prefix, and the agreement of the last with the batch assessment, or the divergence.

## Behavior

- The library shall assess each prefix as it is admitted, so a value the prefix decides is settled before the rest of the history is known.
- The library shall admit a prefix only when every fact of it is after every fact already admitted, in the history's own order, and refuse any other prefix admitting nothing.
- The library shall return, for a divergence, the axes on which the two assessments differ, reporting no agreement over a difference.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-211-AC-1 | With prefixes that cut inside the role, channel and join facts, the assessment after the last prefix equals the batch assessment on every axis, for a conforming and for a faulty history, and the assessment of the last prefix alone does not. | Test (TC-208) |
| FR-211-AC-2 | A join is unsettled in the assessment after the prefix holding two of its branches, settled in the assessment after the prefix holding its settlement, before the loop facts arrive, and equals the batch assessment's join. | Test (TC-208) |
| FR-211-AC-3 | Two assessments that differ are refused with the axes that differ, in declared order, and an assessment agrees with itself. | Test (TC-208) |
| FR-211-AC-4 | A prefix with a fact not after the latest admitted fact is refused naming both observations and admits nothing; the next true extension then equals the batch assessment. | Test (TC-208) |

Each advance assesses everything admitted so far, so a history of `n` facts in `k` prefixes costs `k` assessments of up to `n` facts.

## Dependencies

- [FR-208](./FR-208-admit-every-permitted-observation-order-of-a-history.md) supplies the assessment both paths use. Both paths use one assessment, so agreement is a property of the prefix handling and not of two evaluators; FR-211-AC-3 tests the refusal on assessments that differ, because the two cannot be made to differ by a correct path.
