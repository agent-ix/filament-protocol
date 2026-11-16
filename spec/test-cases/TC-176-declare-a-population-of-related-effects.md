---
id: TC-176
title: "Declare a population of related effects for duplicate detection"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-023
    type: verifies
---
# TC-176: Declare a population of related effects for duplicate detection

## Description

Verify that a registration declaring a finite population of related effects does not read distinct members as duplicate business effects, while an effect outside the population still collides with the registration's singleton occurrence.

## Test Procedure

Declare a three-member population with the recovery relation's `PopulationRequirement`. Retain the effects of attempts naming the three members, a member twice, and members interleaved with two effects outside the population. Compare the first case with the same attempts and no declared population. Offer a population with no member and one with a repeated member.

## Expected Results

Distinct members are retained as established with no disposition; the same three effects with no population give a duplicate for the second and third. A member named twice is retained once. The first effect outside the population is established and the second is a duplicate. An empty population and a repeated member are refused.
