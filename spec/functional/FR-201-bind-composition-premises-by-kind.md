---
id: FR-201
title: "Bind composition dispositions to their premise lists and component subjects"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-030, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-200, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-008, type: references }
  - { target: ix://agent-ix/filament-specification/FR-489, type: depends_on }
  - { target: ix://agent-ix/filament-spec-language/FR-530, type: depends_on }
---
# FR-201: Bind composition dispositions to their premise lists and component subjects

## Description

When a composition is decided, the library SHALL keep the request's environment, rely, liveness-dependency and fairness premises as four separate lists, discharge each premise only by a node of the kind its list requires, and record each discharge against its premise.

## Inputs

One composition request (FR-200) carrying the component subjects (checked package and subject identity each), the declared guarantees and the four premise lists of `ix://agent-ix/filament-specification/FR-489`, each entry naming its discharger.

## Outputs

The composition disposition retaining the four lists and the subjects exactly as requested, the discharge record with one entry per discharged premise naming the list, the index and the discharger, the undischarged premises each with its cause, or one refusal of that disposition under FR-030 naming the node that is not declared.

## Behavior

- The library shall retain all four lists in the disposition exactly as requested, collapsing none into a flag and merging none into another.
- The library shall discharge an `environment` or `fairness` premise only by an assumption identity the request offered.
- The library shall discharge a `relies` premise only by a declared safety guarantee that is proved, outside every dependency cycle and whose proof used only offered assumptions and available guarantees.
- The library shall discharge a `liveness_dependencies` premise only by such a guarantee of liveness form, because a guarantee of one form never discharges a premise of the other.
- The library shall leave undischarged, with its cause, a premise whose discharger is not offered, not declared, of the other form, not `proved`, on a cycle or dependent on an unavailable guarantee.
- When a premise names a discharger that is not a node of its owning checked package, the library shall refuse the disposition under FR-030 naming that node.
- The library shall mark no premise discharged on any basis other than the procedure's own discharge record.

Entailment between different clauses and the fairness implication of `ix://agent-ix/filament-specification/FR-380` are not decided here: a fairness premise is discharged by identity only.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-201-AC-1 | A premise in each of the four lists is discharged or left undischarged by its own list's rule: the same node discharges an `environment` premise when offered and fails a `relies` premise when it is no declared guarantee, and no Boolean flag stands in for any list. | Test (TC-201) |
| FR-201-AC-2 | The disposition retains all four lists exactly as requested and a premise's discharge is recorded against its own list and index; no list is merged into another. | Test (TC-201) |
| FR-201-AC-3 | The discharge record has one entry per discharged premise naming its list, index and discharger, an assumption for `environment` and `fairness` and a guarantee for `relies` and `liveness_dependencies`. | Test (TC-201) |
| FR-201-AC-4 | A premise naming a node its package does not declare refuses the disposition, and the refusal names the node. | Test (TC-201) |
| FR-201-AC-5 | A safety guarantee named by a `liveness_dependencies` premise, a liveness guarantee named by a `relies` premise and a guarantee that is not `proved` each leave the premise undischarged with the cause, and the disposition is not `proved`. | Test (TC-201) |
| FR-201-AC-6 | Given no discharger that satisfies any premise, the discharge record is empty and every premise is listed undischarged. | Test (TC-201) |

## Dependencies

- [FR-200](./FR-200-decide-composition-claims.md) supplies the composition request this requirement binds.
- [FR-030](./FR-030-bind-each-disposition-to-its-exact-premises.md) supplies the premise-binding refusal this requirement applies to the four lists and the nodes.
- `ix://agent-ix/filament-specification/FR-489` defines the four lists this requirement binds.
