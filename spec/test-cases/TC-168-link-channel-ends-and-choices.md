---
id: TC-168
title: "Link channel ends and choices from FSL's read view"
type: TC
relationships:
  - target: ix://agent-ix/filament-protocol/FR-051
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-052
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-151
    type: verifies
---
# TC-168: Link channel ends and choices from FSL's read view

## Description

Verify that a linked channel carries its sender, receiver and message type and a linked choice its owner and visible set exactly as FSL's read view publishes them, and that the dispatcher forwards the channel ends it linked.

## Test Procedure

Compile a three-role protocol with a choice and a fan-in/fan-out of channels of distinct message types with FSL's writer, admit and link it, compare each linked channel and choice with FSL's `channel_endpoints` and `choice_visibility`, and dispatch a claim to a stand-in that records the call.

## Expected Results

Every linked channel equals FSL's reading; two channels of different message types carry different type nodes; the choice carries its owner and visible nodes in order; the recorded call carries the linked channel ends.
