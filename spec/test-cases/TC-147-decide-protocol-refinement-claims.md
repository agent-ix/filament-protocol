---
id: TC-147
title: "Decide protocol refinement claims in this library"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-301
    type: verifies
---
# TC-147: Decide protocol refinement claims in this library

## Description

Verify that a refinement claim is decided by the library's procedure over two finite transition systems with no backend call, that unsupported dynamics and an exhausted bound never refine, that a nondeterministic specification and internal steps are judged by observable traces, and that components are bound and independent of other claims.

## Test Procedure

Request refinements between systems whose traces are included, equal and extended. Put a replicated role, a symbolic-capacity channel and an unsupported construct in turn into the claim. Bound the exploration. Remove or substitute each retained component one at a time. Request a refinement beside finite global conformance. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

Included traces refine and an extra trace refutes; unsupported dynamics settle `unsupported`; an exhausted bound settles `incomplete`; a counterexample found before the bound still refutes; a component change refuses only that disposition; conformance is unchanged.
