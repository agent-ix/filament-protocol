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
  - { target: ix://agent-ix/filament-protocol/FR-303, type: references }
  - { target: ix://agent-ix/filament-specification/FR-433, type: references }
  - { target: ix://agent-ix/filament-specification/FR-636, type: references }
---
# FR-207: Replay a serialized observed history through the dynamics assessment

## Description

When an observed finite history is supplied in the serialized form its owner publishes, the library SHALL read it with the owner's reader and assess it on the role, queue, join and loop axes (FR-053 to FR-056), with the same result as the history built in memory (V1-PROT-025).

A supplied finite *protocol trace* (a trace of protocol steps in FSpec FR-433's spelling, not of observations) is the half FSL now serves: FSL's `finite-replay` engine reads and replays it (FSL FR-636), and FR-303 dispatches it. The serialized observed-history half below still waits on its owner.

## Inputs

A serialized history in its owner's published form and the linked protocol.

## Outputs

The four axis assessments (FR-053 to FR-056), each over the facts the owner's reader returned.

## Behavior

- The library shall read the serialized form only through the owner's reader, declaring no serialization of its own.
- The library shall refuse a history the reader refuses, repairing none.
- The library shall carry a supplied finite protocol trace only through FSL's `finite-replay` engine (FR-303), declaring no trace spelling and replaying no step.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-207-AC-1 | A serialized history assessed through the owner's reader gives the same result on every axis as the equal history built in memory (TC-167). | Test |
| FR-207-AC-2 | A serialized history the owner's reader refuses is refused and not assessed. | Test |
| FR-207-AC-3 | A finite protocol trace the caller supplies is replayed by FSL's `finite-replay` engine and FSL's terminal record of the replay is carried unchanged (FR-303-AC-1). | Test (TC-178) |

## Dependencies

- The supplied finite protocol trace half is backed: FSL's `finite-replay` item (FSL FR-636, merged in agent-ix/filament-spec-language#2785) replays a trace in FSpec FR-433's spelling, and FR-303 dispatches it (FR-207-AC-3, TC-178).
- The serialized observed-history half (FR-207-AC-1, FR-207-AC-2) stays blocked on QSL-2210: the reader and assessor of a serialized observed history (FSL FR-737) are not published. FSpec FR-433's trace is a trace of protocol steps, not of observations, and no owner defines how an observation record becomes a `RoleFact`, `ChannelFact`, `JoinFact` or `LoopFact`, the input types of the assessment; this library builds no such mapping. FR-207-AC-1 and FR-207-AC-2 have no test case.
