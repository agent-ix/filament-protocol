---
id: TC-201
title: "Bind composition dispositions to their premise lists and component subjects"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-201
    type: verifies
---
# TC-201: Bind composition dispositions to their premise lists and component subjects

## Description

Verify that the four premise lists stay separate and that each premise is discharged only by a node of the kind its list requires.

## Test Procedure

Decide compositions with a premise in each list; name an offered assumption in a rely list, an unoffered node in a fairness list, a guarantee of the other form, an unproved guarantee and an undeclared node. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

Each premise is discharged or left undischarged with its cause by its own list's rule; the discharge record names list, index and discharger; an undeclared node refuses the disposition naming it.
