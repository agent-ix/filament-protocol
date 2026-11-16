---
id: TC-208
title: "Assess an observed history for finite global conformance over a compiled choreography"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-208
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-209
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-210
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-211
    type: verifies
---
# TC-208: Assess an observed history for finite global conformance over a compiled choreography

## Description

Verify the history half of V1-PROT-026 over the reference choreography written by FSL's checked-handoff writer and linked (the choreography TC-167 uses), with the facts built by hand as admitted observations and no FSL engine involved.

## Test Procedure

Build a conforming history (two role instances, two queued messages, a settled join of three branches, a loop of three iterations) and faulty ones. Assess each presented forward and reversed; exchange the positions of independent facts and of constrained ones; break one axis at a time; offer a fact naming an undeclared node; deliver out of send order. Settle global conformance under every closure state, every non-completing execution and every omitted axis. Assess the history whole and as three admitted prefixes. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

The same assessment under every permitted order and a different one under a forbidden order. Each fault is non-conformant on its one axis and an undeclared subject is unexpected on its axis; the history is `violated`. Complete global success only with a conformant history, five established axes and a completed execution; the truth never moves with the closure. The last incremental assessment equals the batch one, a join settles before the loop facts arrive, and a difference is refused with its axes.
