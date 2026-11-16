---
id: FR-207
title: "Replay a serialized observed history through the dynamics assessment"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/FR-053, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-054, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-055, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-056, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-433, type: references }
  - { target: ix://agent-ix/filament-specification/FR-636, type: references }
---
# FR-207: Replay a serialized observed history through the dynamics assessment

## Description

When an observed finite history is supplied in the serialized form its owner publishes, the library SHALL read it with the owner's reader and assess it on the role, queue, join and loop axes (FR-053 to FR-056), with the same result as the history built in memory (V1-PROT-025).

## Inputs

A serialized history in its owner's published form and the linked protocol.

## Outputs

The four axis assessments (FR-053 to FR-056), each over the facts the owner's reader returned.

## Behavior

- The library shall read the serialized form only through the owner's reader, declaring no serialization of its own.
- The library shall refuse a history the reader refuses, repairing none.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-207-AC-1 | A serialized history assessed through the owner's reader gives the same result on every axis as the equal history built in memory (TC-167). | Test |
| FR-207-AC-2 | A serialized history the owner's reader refuses is refused and not assessed. | Test |

## Dependencies

- Blocked on an owner: no repository publishes a reader of a serialized protocol history. FSpec FR-433 states a trace spelling for protocol counterexamples (a trace of protocol steps, not of observations) and FSL FR-636's `finite-replay` item that would replay a supplied trace is not implemented; `fsl-replay` reads no such spelling. The fact types the assessment takes (`RoleFact`, `ChannelFact`, `JoinFact`, `LoopFact`) are this crate's input, and no owner defines how an observation record becomes one. No test case exists for this requirement.
