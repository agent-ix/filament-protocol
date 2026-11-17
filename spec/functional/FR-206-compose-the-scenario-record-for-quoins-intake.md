---
id: FR-206
title: "Compose the scenario record from live dispatch outcomes in Quoin's intake shape"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-004, type: implements }
  - { target: ix://agent-ix/filament-protocol/FR-010, type: part_of }
  - { target: ix://agent-ix/filament-protocol/FR-012, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-014, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-016, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-029, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/NFR-005, type: specifies }
  - { target: ix://agent-ix/quoin/FR-130, type: depends_on }
---
# FR-206: Compose the scenario record from live dispatch outcomes in Quoin's intake shape

## Description

When an integration scenario of IT-001, IT-002 or IT-003 reaches a typed outcome, the library SHALL compose one scenario record from the live dispositions of its claims (FR-029), carrying their execution, truth and basis axes independently (V1-RUN-005, V1-RUN-006), as the typed `ScenarioEvidence` value Quoin's intake `quoin.protocol-scenario-intake/v1` publishes, importing that type and declaring no member, enumeration or wire string of it.

This requirement succeeds FR-006, FR-007 and FR-017, retired with the evaluator (FSL-1549).

## Inputs

The dispositions of the scenario's claims, the resolution state and value owners it ran against (FR-012), the budget granted and consumed per dimension (FR-016), and the acceptance criterion the scenario is bound to.

## Outputs

One `quoin_evidence::scenario::ScenarioEvidence` value per scenario, handed to Quoin's intake. The library keeps, indexes, scores and disposes of nothing it composes.

## Behavior

- The library shall build the record only from a live `OperationOutcome` and the dispositions of the claims it returned, never from a transcript or a stored result.
- The library shall report a scenario that cannot retain every component as unexecuted, never as passing.
- The library shall leave an absent grant absent, never read as zero.
- The library shall identify each claim of the record by its claim kind and its axes alone: a row by the kind, the execution, the truth and the settlement basis the disposition carries; a refusal by the kind and the cause it was refused with. It derives no identity from display text, and one claim's identity is never derived from another claim's disposition.
- The library shall report the resolution it ran against as the full manifest, or as the entries reached before the first unresolved entry together with that entry and its reason.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-206-AC-1 | The record is `quoin_evidence::scenario::ScenarioEvidence`, imported, and this repository declares no enum, struct or string of its shape. | Test (TC-179) |
| FR-206-AC-2 | A scenario's record built from live dispositions is accepted by Quoin's `write_scenario_evidence`, and the record of one claim is the same whether or not another claim was requested beside it. | Test (TC-179) |
| FR-206-AC-3 | A scenario that cannot retain every component is reported unexecuted. | Test (TC-179) |

## Dependencies

- `quoin-evidence` (agent-ix/quoin, `FR-130`) publishes `ScenarioEvidence` and `write_scenario_evidence`; it is a dependency of this crate beside `filament-observation`, both on `serde` 1.0.229.
- `LegacyCompatibility` (IT-001-SC-06a, a frozen facade with no resolver) is not composed: this library holds no live selection for it.
- [FR-029](./FR-029-return-one-disposition-per-requested-claim.md) supplies the dispositions.
