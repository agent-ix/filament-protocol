---
id: FR-202
title: "Report refinement counterexamples and read verdicts into the axes"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-008, type: references }
  - { target: ix://agent-ix/filament-protocol/FR-030, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-301, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-241, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-242, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-177, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-243, type: depends_on }
  - { target: ix://agent-ix/filament-spec-language/FR-148, type: references }
---
# FR-202: Report refinement counterexamples and read verdicts into the axes

## Description

When a refinement is refuted, the library SHALL return a paired, replayable counterexample for each extra trace, deadlock and divergence it finds, reading each verdict into the assessment-execution, truth and settlement-basis values.

## Inputs

A requested `refinement` claim (FR-301) with its implementation and specification transition systems.

## Outputs

The counterexamples, each a typed value pairing the implementation run with what the specification can do on the same observable trace, and the verdict with its execution, truth and basis.

## Behavior

- When the implementation has an observable trace the specification does not allow, the library shall return an extra-trace counterexample: the implementation run ending in the offending action, the observable trace before it, and the specification states reached on that trace.
- When the implementation reaches a non-final state with no step at all after an observable trace on which a specification state can still take a step, the library shall return a deadlock counterexample.
- When the implementation can take internal steps forever after an observable trace on which no specification state can, the library shall return a divergence counterexample with the lasso of internal steps.
- The library shall count a stop in a final state, a stop where the specification cannot continue and a loop the specification shares as no fault.
- The library shall let each counterexample be replayed against the request: the implementation run starts at the initial state and takes only the system's transitions, the specification side is exactly what the specification reaches on the run's observable trace, and the fault holds there.
- The library shall find the counterexamples by breadth-first search, shortest implementation run first, returning them in a total order so that the findings depend on neither the order of the transitions nor the order of any list.
- The library shall name the verdict by the precedence unsupported, then refuted, then incomplete, then refines, retaining every finding beside it.
- The library shall read `refines` as execution `completed`, truth `satisfied` and basis `closed-scope`; `refuted` as `completed`, `violated` and `decisive-counterexample`; `incomplete` as `resource-incomplete`, `unavailable` and `unavailable`; and `unsupported` as `unsupported`, `unavailable` and `unavailable`.
- The library shall set the execution value `refused` on no refinement disposition, because a refusal under FR-030 is a separate act and sets no execution value.
- The library shall reuse no FSL witness type for a counterexample, because FSL's refinement witnesses are state-based over a model subject and these systems are protocol-level.

FR-302's trace-position witness for a `refinement` claim is FSL's `RefinementWitness` record (`ix://agent-ix/filament-spec-language/FR-578`), built by `Refinement::witness` for the deciding node and position; the counterexamples stay this library's own paths.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-202-AC-1 | Given an implementation whose second action the specification does not allow, the library returns an extra-trace counterexample naming the action, the implementation run, the observable trace before the action and the specification states reached on it. | Test (TC-202) |
| FR-202-AC-2 | Given an implementation that stops in a non-final state after an action where the specification can continue, the library returns a deadlock counterexample with the run and the specification states reached. | Test (TC-202) |
| FR-202-AC-3 | Given an implementation that loops on internal steps after an action where the specification finishes, the library returns a divergence counterexample with the lasso. | Test (TC-202) |
| FR-202-AC-4 | A stop in a final state, two systems that are both stuck and a loop the specification shares are not faults. | Test (TC-202) |
| FR-202-AC-5 | The four verdicts read into the execution, truth and settlement-basis values the Behavior section states, and only `refines` reads as `satisfied`. | Test (TC-202) |
| FR-202-AC-6 | A request with an extra trace, a deadlock and a divergence returns all three, unchanged when every transition and state list is reversed; `unsupported` outranks a refutation and keeps its counterexamples, and a refutation outranks an exhausted bound. | Test (TC-202) |
| FR-202-AC-7 | Each counterexample replays against its request, and altering its fault, its implementation run, its specification trace or reached states, or its lasso, or replaying it against a request whose systems differ, fails the replay. | Test (TC-202) |

## Dependencies

- [FR-301](./FR-301-decide-protocol-refinement-claims.md) supplies the refinement decision whose findings this requirement reports.
- [FR-030](./FR-030-bind-each-disposition-to-its-exact-premises.md) supplies the refusal that sets no execution value.
- [FR-008](./FR-008-return-typed-refusals.md) supplies the typed refusal vocabulary.
- `ix://agent-ix/filament-specification/FR-241`, `ix://agent-ix/filament-specification/FR-242` and `ix://agent-ix/filament-specification/FR-243` own the axis vocabularies; `ix://agent-ix/filament-specification/FR-177` owns the model-level relation this library does not evaluate.
