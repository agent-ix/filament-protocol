---
id: FR-301
title: "Decide protocol refinement claims in this library"
type: FR
relationships:
  - target: ix://agent-ix/filament-protocol/US-001
    type: implements
  - target: ix://agent-ix/filament-protocol/AD-001
    type: references
  - target: ix://agent-ix/filament-protocol/FR-002
    type: depends_on
  - target: ix://agent-ix/filament-protocol/FR-005
    type: depends_on
  - target: ix://agent-ix/filament-protocol/FR-202
    type: depends_on
  - target: ix://agent-ix/filament-protocol/FR-204
    type: depends_on
  - target: ix://agent-ix/filament-protocol/FR-029
    type: depends_on
  - target: ix://agent-ix/filament-protocol/FR-030
    type: depends_on
  - target: ix://agent-ix/filament-specification/FR-060
    type: depends_on
  - target: ix://agent-ix/filament-specification/FR-176
    type: depends_on
  - target: ix://agent-ix/filament-specification/FR-177
    type: depends_on
  - target: ix://agent-ix/filament-specification/FR-290
    type: references
  - target: ix://agent-ix/filament-specification/AD-007
    type: references
  - target: ix://agent-ix/filament-protocol/ADR-010
    type: references
  - target: ix://agent-ix/filament-protocol/FR-043
    type: depends_on
  - target: ix://agent-ix/filament-protocol/FR-044
    type: depends_on
  - target: ix://agent-ix/filament-spec-language/ADR-020
    type: depends_on
  - target: ix://agent-ix/filament-spec-language/FR-142
    type: depends_on
  - target: ix://agent-ix/filament-spec-language/FR-144
    type: depends_on
  - target: ix://agent-ix/filament-spec-language/FR-147
    type: depends_on
---
# FR-301: Decide protocol refinement claims in this library

## Description

When a `refinement` claim is requested, the dispatcher SHALL decide it with the library's own protocol-level refinement procedure over the implementation and specification transition systems, with no backend call.

## Inputs

One requested `refinement` claim (an `ix://agent-ix/filament-specification/FR-290` label) with its admitted fragment, the implementation and specification protocols (each a checked node and a finite transition system of observable and internal steps, final states, the assumptions it rests on and the constructs its builder could not step), the offered assumption identities, the source identities and the most product states the decision may explore. The checked-package references (FR-043, FR-044) naming the two protocols. The fragment's role and channel declarations (FR-002), read only to find a replicated role or a symbolic capacity.

## Outputs

One source-bound refinement disposition retaining its exact fragment, systems, assumptions and sources as FR-030 requires of every disposition, together with the counterexamples, the stronger assumptions and the unsupported constructs the procedure found; or one refusal of that disposition under FR-030.

## Behavior

- The dispatcher shall decide each requested `refinement` claim by the procedure FR-202 and FR-204 state, with no backend call.
- The dispatcher shall explore the specification by subset construction over its internal-step closure, so that a nondeterministic specification and internal steps on either side change no observable trace.
- The dispatcher shall settle a `refinement` claim `unsupported` when the fragment holds a replicated role or a channel of symbolic capacity, or when a system reports a construct its builder could not step, and `refines` in no such case.
- The dispatcher shall settle a `refinement` claim `incomplete` when the exploration reaches the request's bound before every product state, truncating no bound to finish.
- The dispatcher shall not treat an `unsupported` or `incomplete` settlement as a hold, a blocker or a refusal error.
- The dispatcher shall leave the independent global conformance result and every other claim's disposition unchanged (FR-029).
- The dispatcher shall not carry the complete-global-conformance premise set as evidence for a refinement disposition's own success, as `ix://agent-ix/filament-specification/FR-290`-AC-3 requires of every non-global claim.
- When a refinement disposition would be `refines` or `refuted` without a source identity, the dispatcher shall refuse it (FR-030).

This requirement reverses the dispatch FR-301 stated before ADR-010 Decision 7. FSL still owns atomic and temporal obligations and model-level refinement (`ix://agent-ix/filament-specification/FR-147`: a model that refines a protocol is never kind `refinement`); neither is evaluated here. The transition systems are this library's own small type. Deriving one from a linked protocol needs FSL's compile and link call (S2); until then a caller supplies the systems, and the procedure decides nothing about a construct the supplier could not step.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-301-AC-1 | A refinement disposition never carries the complete-global-conformance premise set as evidence for its own success, exactly as `ix://agent-ix/filament-specification/FR-290`-AC-3 requires, and a conformance result requested beside it is returned unchanged. | Test (TC-147) |
| FR-301-AC-2 | A fragment with a replicated role, a fragment with a channel of symbolic capacity and a system that reports an unsupported construct each settle `unsupported`, naming the role, channel or construct, with execution `unsupported`, truth `unavailable` and basis `unavailable`, and never `refines`; the conformance result requested beside it is unchanged. | Test (TC-147) |
| FR-301-AC-3 | A requested refinement is decided here: an implementation whose observable traces are among the specification's refines, and one with an extra trace is `refuted`. | Test (TC-147) |
| FR-301-AC-4 | A refinement whose exploration reaches the request's bound settles `incomplete` with execution `resource-incomplete`, truth `unavailable` and basis `unavailable`, never `refines`, and a counterexample found before the bound still refutes it. | Test (TC-147) |
| FR-301-AC-5 | A refinement disposition retains its exact fragment, systems, assumptions and sources, and removing or substituting the fragment, a protocol node, an assumption, a source or an observable label one at a time refuses only that disposition, naming the node (FR-030). | Test (TC-147) |
| FR-301-AC-6 | A nondeterministic specification, and internal steps on either side, are judged by observable traces: an implementation that follows one branch of the specification refines it, and hidden steps change no verdict. | Test (TC-147) |

## Adopted vocabulary

| Noun | Upstream object | Identity kind |
| --- | --- | --- |
| Refinement claim kind | `ix://agent-ix/filament-specification/FR-290` | value |
| Assume-guarantee composition | `ix://agent-ix/filament-specification/FR-176` | value |
| Refinement relation | `ix://agent-ix/filament-specification/FR-177` | value |

### Accepted owner rulings

`ix://agent-ix/filament-specification/FR-177` states the refinement relation FSL's
model-level engine decides. The protocol-level procedure of this library decides
trace inclusion, deadlock, divergence and assumption strength over finite
protocol systems (ADR-010 Decision 7), and defines no model-level relation, no
temporal evaluator and no fixed-point rule.

## Dependencies

- [FR-002](./FR-002-link-finite-protocol-representation.md) supplies the declarations whose replicated roles and symbolic capacities make a claim unsupported.
- [FR-005](./FR-005-dispatch-independent-protocol-claims.md) supplies closed claim dispatch.
- [FR-029](./FR-029-return-one-disposition-per-requested-claim.md) and [FR-030](./FR-030-bind-each-disposition-to-its-exact-premises.md) supply independence and premise binding.
- [FR-202](./FR-202-report-refinement-counterexamples-and-verdicts.md) and [FR-204](./FR-204-refuse-a-claim-under-stronger-assumptions.md) state the counterexamples and the stronger-assumption finding.
- `ix://agent-ix/filament-specification/FR-060` states the projection-absence pattern this requirement mirrors for an unsupported refinement.
