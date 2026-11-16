---
id: FR-210
title: "Withhold complete global success unless the history conforms and every closure axis is established"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-208, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-209, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-247, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-284, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-241, type: depends_on }
---
# FR-210: Withhold complete global success unless the history conforms and every closure axis is established

## Description

When a history is settled for global conformance, the library SHALL return complete global success only when every dynamic axis is conformant, the five closure axes are each established and the assessment execution completed, with every reason it is withheld reported otherwise.

This requirement succeeds FR-028, retired with the evaluator (FSL-1549). The five closure axes are decision-scope progress, decision-scope closure, surrounding-execution progress, surrounding-execution closure (each `open` or `closed`, FSpec FR-284) and complete-global-conformance closure (`closed`, `open`, `incomplete`, `contradicted` or `not-required`, FSpec FR-247). They are supplied by the caller, who observed them; none is derived from the history.

## Inputs

A history assessment (FR-208) and a `ProtocolAssessment`: the five closure axes and the assessment execution state (FSpec FR-241).

## Outputs

The history's truth, and either the complete-global-conformance witness or every reason it is withheld: the non-conformant dynamic axes and the closure refusals.

## Behavior

- The library shall issue the complete-global-conformance witness only for a conformant history, so that the closure alone, which proves the five axes and the execution, can issue no witness.
- The library shall withhold complete global success unless the complete-global-conformance closure is `closed`.
- The library shall not substitute a closed decision scope, a closed surrounding execution, a completed execution or a settled truth for any closure axis.
- The library shall withhold complete global success for `not-required`.
- The library shall withhold complete global success when the execution is refused, unsupported, resource-incomplete or failed.
- The library shall leave the history's truth unchanged by every state of the closure record.
- The library shall refuse an omitted closure axis and default none to `open`, `closed` or `not-required`.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-210-AC-1 | With a conformant history and a completed execution, complete global success is returned when all five axes are established, and each axis alone not established withholds it, naming only that axis; every fifth-axis member but `closed`, `not-required` included, withholds it. | Test (TC-208) |
| FR-210-AC-2 | A refused, unsupported, resource-incomplete or failed execution withholds complete global success, naming the execution state, with every closure axis established. | Test (TC-208) |
| FR-210-AC-3 | Closing the decision scope with the surrounding execution and the global closure open names those three axes and not the decision scope. | Test (TC-208) |
| FR-210-AC-4 | A non-conformant history with every closure axis established and a completed execution withholds complete global success, naming the dynamic axes and no closure refusal. | Test (TC-208) |
| FR-210-AC-5 | The history's truth is the same under every closure state. | Test (TC-208) |
| FR-210-AC-6 | An omitted closure axis is refused as omitted, every omitted axis is reported, and none is defaulted. | Test (TC-208) |

## Dependencies

- [FR-208](./FR-208-admit-every-permitted-observation-order-of-a-history.md) and [FR-209](./FR-209-report-adverse-observations-of-a-history-on-their-axis.md) supply the history's truth and axes.
- The closure algebra is `closure`: `ProtocolAssessment::closure_established` proves the closure half as `ClosureEstablished`, and only `history::settle_global` turns it into `CompleteGlobalConformance`, whose constructor is crate-private (a `compile_fail` doctest); the five axes are distinct types over their owners' vocabularies, so no axis substitutes for another at compile time.
