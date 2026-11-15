---
id: FR-046
title: "Consume temporal evidence through the in-memory evidence types"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/FR-003, type: references }
  - { target: ix://agent-ix/filament-protocol/FR-004, type: references }
  - { target: ix://agent-ix/filament-protocol/FR-044, type: depends_on }
  - { target: ix://agent-ix/filament-spec-language/FR-516, type: depends_on }
  - { target: ix://agent-ix/filament-spec-language/FR-521, type: references }
---
# FR-046: Consume temporal evidence through the in-memory evidence types

## Description

When the library assesses a temporal obligation, it SHALL accept the caller's
`fsl_eval::temporal::evidence::EvidenceRequest`, call the temporal-evidence
entry that evaluates from the checked package alone and read the
`EvidenceResult`, in
place of the retired `native_temporal::v2::ValidatedRequest`,
`native_temporal::v2::ValidatedResult`, `native_temporal::v2::EvidenceRef` and
`native_temporal::v2::ActivationState` and their four producer contracts.

## Inputs

The admitted checked package from recipe **R** (FR-043), a `CheckedNodeId`
naming a temporal clause, an `EvidenceRequest` constructed by the caller from
observation evidence under FSL FR-516, and a `Meter`. This library accepts and
passes the request to FSL; it does not define or perform a mapping from
observation evidence to `EvidenceRequest`. No source package, emission or
admitted-read value of FSL's emit side is an input.

## Outputs

An `EvidenceResult` carrying the activation state, truth, settlement and decision
support, joined to the library's result record; or the call's typed refusal.

## Behavior

- The assessment, result record, conformance kernel and the IT-001 and IT-002
  procedures shall pass `EvidenceRequest`, `EvidenceResult` and typed call
  refusals through the public temporal-evidence boundary.
- The library shall carry a call refusal (`CallFailure::Admission` with its cause
  name) into its own typed refusal unchanged and shall reinterpret none.
- The library shall carry a construction refusal (`PackageMismatch`) as a typed
  refusal before any call and charge nothing for it.
- The caller constructs the `EvidenceRequest` from observation evidence; this
  requirement does not assign that mapping to this library.
- The library shall keep using FSL's public `temporal::` surface (`Closure`,
  `Completeness`, `Execution`, `Evidence`, `Position`, `OrderKey`) until FSL
  retires it (FSL FR-521 row 5), and shall fix its own use of
  `Position.instance_valuations` for the FSL revision it consumes.
- The request and result are in-memory values; the consumer does not serialize
  them as request/result documents or assign them document contract identifiers.
- The result record shall be the same for two evidence results that FSL reports
  as equal.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-046-AC-1 | Over **R**'s four temporal clauses, the caller supplies an `EvidenceRequest` built from observation evidence; the assessment passes it to FSL and returns the `EvidenceResult`'s activation state, truth, settlement and decision support in its result record, equal to the values FSL's evidence call reports. | Test |
| FR-046-AC-2 | `EvidenceRequest` and `EvidenceResult` cross the FSL call as in-memory values; the consumer neither serializes them as request/result documents nor assigns them document contract identifiers. | Test |
| FR-046-AC-3 | The temporal evidence boundary accepts FSL's current `EvidenceRequest` and checked clause identity and returns FSL's `EvidenceResult` or typed call refusal. | Test |
| FR-046-AC-4 | For the six call-refusal causes reachable from a caller-built request over an admitted package in **R**—`trigger-required`, `trigger-unknown`, `occurrence-unknown`, `activation-mapping-missing`, `clause-root-unresolved`, and `binding-mismatch`—the library returns the typed refusal naming the unchanged cause, charges nothing, and leaves caller ledgers unchanged. Causes requiring a package or request state that R cannot admit are outside this criterion. | Test |
| FR-046-AC-5 | The bytes reader refuses duplicate temporal-clause name rows as `AmbiguousName`; because such rows cannot occur in an admitted package, no admitted **R** package reaches a `PackageMismatch` construction refusal. This criterion therefore has no post-admission test input; reader refusal is covered by FR-043. | Analysis |
| FR-046-AC-6 | The IT-002 scenarios defined against recipe **R** pass on the evidence route, and an unavailable or withdrawn anchor settles as a missing non-value and never as a Boolean. The caller supplies the `EvidenceRequest`; IT-002 asserts the request passed to FSL and does not assert an observation-to-request mapping owned by this library. | Test |
| FR-046-AC-7 | On the consumed FSL revision, the library builds against the retained `temporal::` surface and passes the `Position.instance_valuations` values that the evaluation reads. The integration verifies this positively; removal of the retained surface is not a supported build configuration. | Test |

## Dependencies

- FSL FR-516, FR-515 (the evidence package), FR-521 rows 3, 4 and 5, FR-044.
  FSL delivers the entry that evaluates from a `CheckedPackageV2` alone as
  `EvidencePackage::from_package` over a `BarePackage` (FSL FR-531, FR-532).
- [FR-003](FR-003-assess-compensation-state.md) and
  [FR-004](FR-004-assess-finite-global-conformance.md) state the assessment
  contracts; this requirement changes only their temporal evidence source.
