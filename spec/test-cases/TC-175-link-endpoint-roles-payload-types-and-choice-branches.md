---
id: TC-175
title: "Link channel endpoint roles, message payload types and choice branch labels"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-051
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-052
    type: verifies
---
# TC-175: Link channel endpoint roles, message payload types and choice branch labels

## Description

Verify V1-PROT-003, V1-PROT-004 and V1-PROT-009 over a protocol written by FSL's checked-handoff writer, compiled back and linked: each channel end is a declared role, each send and receive carries its own payload binder and type, and a choice's branches carry their labels.

## Test Procedure

Link the reference protocol (`Order`, then `Pick` or `Cancelled`). Check the sender and receiver of every linked channel against the protocol's declared roles. Read each send and receive with FSL's control reader and compare its payload binder's type with its channel's message type. Read the choice's cases with FSL's control reader. Tag each test with the exact acceptance criterion it verifies.

## Expected Results

Every channel end is a declared role of the protocol and is the role the source names. Each send and receive binder carries the type its channel carries, and channels of different types give different binder types. The choice has one case per authored label, in label order, each a declared control.
