---
id: FR-058
title: "Map FSL requires-bound outcomes to typed dispositions"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-005, type: references }
  - { target: ix://agent-ix/filament-protocol/FR-029, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-030, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-008, type: references }
  - { target: ix://agent-ix/filament-spec-language/ADR-017, type: depends_on }
  - { target: ix://agent-ix/filament-spec-language/ADR-020, type: references }
  - { target: ix://agent-ix/filament-spec-language/ADR-027, type: depends_on }
---
# FR-058: Map FSL requires-bound outcomes to typed dispositions

## Description

When FSL reports a requires-bound outcome for a protocol construct whose bound is missing, the library SHALL return a typed non-success disposition carrying the unbounded domains FSL names. The library SHALL NOT truncate the construct at a finite bound to finish.

## Inputs

A FSL requires-bound outcome naming the construct, and the request's premises.

## Outputs

A typed non-success disposition carrying the FSL outcome unchanged, and each unbounded domain with the domain kind and domain key FSL names, unchanged (FSL kinds: `Collection`, `Population`, `Integer`, `Recursive`, `Loop`, `InfiniteTrace`, `Quantity`).

## Behavior

- The library shall define no bound kind of its own. A channel with a symbolic capacity that the protocol subject's capacity binding leaves unbound is a requires-bound outcome (FSpec FR-785), not an `unsupported` one: FSL reports it as a `RequiresBound` naming the unbound capacities with the unbounded domains, or, when no domain is unbounded, as its list of unbound capacity names (each name with the channels whose queue uses it); the library shall carry either unchanged.
- When the assessment of an observed history is given no binding for a symbolic queue capacity, the library shall report the channel outcome and the axis verdict as requires-bound, naming the channel and the authored capacity, and not `unsupported`.
- The library shall carry a capacity binding FSL refuses (an unknown name, a repeated name, a value that is not canonical decimal) as FSL's `invalid_runtime_input` refusal with FSL's cause, unchanged.
- The library shall not pick, default or search for a bound.
- The library shall not complete the analysis over a bound it chose.
- The library shall report the disposition as non-success on its own axis.
- The library shall not fold it into conformance, progress or closure (FR-028).
- The library shall carry no partial result with the disposition that would read as complete.
- When the same request asks other claims, the library shall leave their dispositions unchanged (FR-029).

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-058-AC-1 | Given a requires-bound outcome naming a `Population`, a `Collection` and a `Loop` domain, each disposition is typed non-success and carries FSL's outcome unchanged, with each domain's kind and key equal to FSL's and no kind the library defined. | Test (TC-055) |
| FR-058-AC-2 | The library calls no FSL analysis with a bound the library derived, and no disposition field holds a bound the library chose. | Test (TC-055) |
| FR-058-AC-3 | A requires-bound disposition on one claim leaves every other requested claim's disposition unchanged and changes none of the conformance, progress or closure axis values. | Test (TC-055) |
| FR-058-AC-4 | Given a protocol with a symbolic-capacity channel dispatched to FSL's real checker with no binding for it, the disposition is requires-bound, carries FSL's list of unbound capacity names and channels unchanged and no result, and, beside an unbounded root, the outcome FSL's `RequiresBound` names the capacity with the domains. | Test (TC-209) |
| FR-058-AC-5 | Given an observed history of a channel whose capacity is `symbolic(<name>)` and no binding, the assessment reports the channel outcome and the axis verdict requires-bound, and names the channel and the authored capacity. | Test (TC-208) |

## Dependencies

- [ADR-010](../adr/ADR-010-consume-fsl-protocol-analysis-own-assessment-and-dispatch.md) fixes what this library owns for dynamic roles, bounded queues, joins and loops, and what FSL owns.
- FSL ADR-027 states the step semantics this requirement does not restate; FSpec FR-170 and FR-171 state the thresholds and overflow rules the criteria cite.
