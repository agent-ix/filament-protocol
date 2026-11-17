---
id: TC-179
title: "Compose the scenario record from real engine dispositions for Quoin's intake"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-206
    type: verifies
---
# TC-179: Compose the scenario record from real engine dispositions for Quoin's intake

## Description

Verify that the scenario record is the typed `ScenarioEvidence` of Quoin's intake `quoin.protocol-scenario-intake/v1`, composed from the live dispositions of claims FSL's real `protocol-analysis` engine settled, accepted by `write_scenario_evidence`, with the execution, truth and basis axes of each claim independent of every other claim's, and that a scenario which cannot retain a component is reported unexecuted.

## Test Procedure

Take the three-role reference protocol over the recipe model compiled from FSL's checked-handoff writer. Register FSL's engine at the provider boundary and dispatch projection claims with `dispatch_claims`: one the engine decides, and one it refuses. Compose the record with `scenario::compose` from the `OperationOutcome` and a version-lock manifest, and file it with Quoin's `write_scenario_evidence` into an in-memory store; read the filed text back as the owner's `ScenarioEvidence`. Dispatch the decided claim alone and beside the refused one, over a violated and over a satisfied protocol, and compare the decided claim's record. Compose over an empty manifest, a value owner the manifest does not pin, and a pinned entry without a revision. Compose over a manifest holding an unresolved entry. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

The filed record equals the composed one. A violated claim reads `completed`, `violated`, `decisive-counterexample`; a satisfied one `completed`, `satisfied`, `closed-scope`; the refused request is a claim-local refusal, not a row; the first claim's record is the same alone and beside the second. An absent grant stays absent. The three incomplete manifests are reported unexecuted; the unresolved entry yields the partial resolution state and its causing entry.
