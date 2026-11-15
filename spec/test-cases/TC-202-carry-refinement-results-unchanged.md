---
id: TC-202
title: "Carry refinement results without reinterpretation"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-202
    type: verifies
---
# TC-202: Carry refinement results without reinterpretation

## Description

Verify that a refinement result's certificate, counterexample halves, witness and cause are carried unchanged and that no axis value is derived, with a stand-in for the FSL call boundary and no local analysis.

## Test Procedure

Feed a stand-in refinement result carrying certificate bytes, a one-halved and a two-halved counterexample, each cause name, a refusal and each label. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

Every artifact is carried byte-equal; no second half appears; causes are unmapped and none is a success; no execution, truth or basis is derived; `refused` is derived on nothing.
