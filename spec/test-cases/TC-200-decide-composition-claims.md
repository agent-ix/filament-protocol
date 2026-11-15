---
id: TC-200
title: "Decide composition claims in this library"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-200
    type: verifies
---
# TC-200: Decide composition claims in this library

## Description

Verify that a composition claim is decided by the library's procedure with no backend call, that a named backend is refused, independence of other claims, label-driven availability and the reading of each verdict into the axes.

## Test Procedure

Request a composition claim whose premises are all discharged, with a recording stand-in registered for the composition capability, and again with no backend. Name a backend. Request it beside finite global conformance and a refinement claim. Change one guarantee's FSL label to each non-`proved` label. Remove or substitute each retained disposition component one at a time. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

The claim is decided `proved` with a discharge record and the stand-in is never called; a named backend is refused; other dispositions are unchanged; a non-`proved` guarantee label never yields `proved`; each verdict reads into its stated execution, truth and basis.
