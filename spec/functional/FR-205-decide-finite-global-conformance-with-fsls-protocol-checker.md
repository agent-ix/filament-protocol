---
id: FR-205
title: "Decide finite global conformance with FSL's protocol checker"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-005, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-029, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-030, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-040, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-150, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-153, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-208, type: references }
  - { target: ix://agent-ix/filament-protocol/FR-209, type: references }
  - { target: ix://agent-ix/filament-protocol/FR-210, type: references }
  - { target: ix://agent-ix/filament-protocol/FR-211, type: references }
  - { target: ix://agent-ix/filament-specification/FR-059, type: references }
  - { target: ix://agent-ix/filament-specification/FR-060, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-290, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-429, type: references }
  - { target: ix://agent-ix/filament-specification/FR-433, type: references }
---
# FR-205: Decide finite global conformance with FSL's protocol checker

## Description

When a `global-conformance` claim is requested for a finite protocol, the dispatcher SHALL bind the claim to its premises (FR-151), take the one backend FSL's registry selects (FR-290), and carry the result FSL's explicit-state protocol checker settled, unchanged.

This requirement succeeds FR-004 for the question FSL decides: whether every behaviour of the protocol is free of deadlock (FSL FR-211, FR-215; FSL FR-636 names this item `global-conformance`). Finite global conformance is that question together with the history questions this library decides itself over the live assessment axes, which succeed FR-026, FR-027, FR-028 and FR-041: order invariance (FR-208), adverse observations (FR-209), closure axes (FR-210) and batch against incremental agreement (FR-211). FSL has no engine for the history questions (ADR-010 Decision 8), so they are not dispatched; this requirement dispatches only the protocol check. The protocol check is narrower than global conformance, and its `refuted` settlement is untested (FSL advertises no `global-conformance` kind and a handoff cannot supply the replay, ADR-010 G8, G9), so V1-PROT-026 stays Partial until QSL-2209 lands and that path has a real test.

## Inputs

One requested `global-conformance` claim: its fragment, premises, source identities and optional backend (FR-150), and the item FSL's checker is asked to decide, with the protocol subject, the certificate request and the counterexample replay FSL's `ProtocolItem` states. The caller states them; FSL refuses an absent member.

## Outputs

FSL's `ModelCheckRecord` of the run (terminal value, label, settlement basis, truth, the limits the run used, the replayed counterexample), carried beside the request's exact premises (FR-030), or one non-success disposition (FR-153).

## Behavior

- The dispatcher shall hold no decision procedure, no search, no temporal evaluator and no refinement here (ADR-010 Decision 1; FSL FR-530, FR-147 stand).
- The dispatcher shall select the backend through FSL's registry on the capability FSL routes the item under. FSL routes a protocol check as a `temporal-satisfaction` item, and no engine advertises the `global-conformance` kind itself, so that is the capability the claim selects on; the dispatcher defines no capability of its own.
- The dispatcher shall read the truth and settlement basis from FSL's record and add none (FR-040): a decisive `violated` is reported only when FSL's replay reproduced the counterexample.
- The dispatcher shall carry a decisive result only beside at least one source identity (FR-152). A `closed-scope` proof is not decisive in that sense (FR-152), so it carries none and no test refuses it.
- While no backend serves the capability, the dispatcher shall settle the claim `unsupported` (FR-153).
- When FSL's checker refuses the subject as requiring a bound (`ModelCheckRefusal::RequiresBound`, FSL FR-336), the dispatcher shall settle the claim with the typed requires-bound disposition of FR-058 carrying FSL's outcome and its domain kinds and keys unchanged.
- The capacity binding of a `symbolic(<name>)` queue is part of the protocol subject the caller states, in FSL's own `CapacityBinding` (FSpec FR-785); the dispatcher shall add, default or derive no binding.
- When FSL's checker reports a symbolic capacity the subject's binding leaves unbound (`ModelCheckRefusal::UnboundCapacity`), the dispatcher shall settle the claim with the requires-bound disposition of FR-058 carrying FSL's list of unbound capacity names and channels unchanged, never `unsupported`.
- When FSL's checker refuses the capacity binding (an unknown name, a repeated name; `invalid_runtime_input`), the dispatcher shall settle the claim with FSL's refusal, its code and cause unchanged.
- When FSL's checker refuses the item otherwise, the dispatcher shall report a failure of the selected backend and no result. FSL classifies those refusals in a map it does not publish (ADR-010 G9), so only the refusal's text is carried.
- The dispatcher shall settle the claim independently of every other requested claim (FR-029).

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-205-AC-1 | Given a protocol compiled from a checked handoff that FSL's checker proves deadlock-free, the dispatch returns the checker's record with truth `satisfied`, settlement basis `closed-scope`, label `proved` and no counterexample. | Test (TC-205) |
| FR-205-AC-2 | Given the same protocol checked by the checker directly, the dispatched record equals the direct record in value, label, basis, truth and limits, so the dispatcher changed nothing. | Test (TC-205) |
| FR-205-AC-3 | Given a protocol the checker finds deadlocked and a replay that cannot reproduce the counterexample, the dispatch holds neither truth `violated` nor a counterexample, and the record is FSL's refused settlement. | Test (TC-205) |
| FR-205-AC-4 | Given an empty registry, a `global-conformance` claim settles `unsupported` with a warning naming the capability it selects on, and holds neither `satisfied` nor `violated`. | Test (TC-205) |
| FR-205-AC-5 | Given the checked-handoff recipe model as FSL ships it, whose `Total` and `Amount` are unbounded quantity roots (FSL FR-120), with no bound supplied and none chosen by a rewrite of the model, the checker refuses the subject as requiring a bound and the dispatch settles the typed requires-bound disposition (FR-058), whose outcome equals the one `check_protocol` returns directly and whose domain kinds and keys are FSL's unchanged, with no record and neither truth `satisfied` nor `violated`. | Test (TC-205) |
| FR-205-AC-6 | Given a protocol with a symbolic-capacity channel and a subject whose binding leaves it unbound, the checker reports the unbound capacity and the dispatch settles the requires-bound disposition (FR-058) carrying FSL's list unchanged, with no record and neither truth `satisfied` nor `violated`. | Test (TC-209) |
| FR-205-AC-7 | Given the same protocol with the capacity bound, FSL decides with the bound (a deadlock at capacity one, none at two, equal to the literal capacity's) and the dispatched record equals the one FSL's two calls give directly. | Test (TC-209) |
| FR-205-AC-8 | Given a binding that names no capacity of the protocol, and one that names a capacity twice, the dispatch settles FSL's `invalid_runtime_input` refusal with FSL's cause (`unknown-member`, `duplicate-member`) unchanged. | Test (TC-209) |

## Adopted vocabulary

| Noun | Upstream object | Identity kind |
| --- | --- | --- |
| Protocol check item, record, certificate request, replay | FSL FR-215, FR-217, FR-338 (`ProtocolItem`, `ModelCheckRecord`) | FSL-owned, consumed |
| Truth, settlement basis | FSpec FR-242, FR-243 | FSpec-owned, consumed |
| Capability selection | FSL FR-290 | FSL-owned, consumed |

## Dependencies

- [ADR-010](../adr/ADR-010-consume-fsl-protocol-analysis-own-assessment-and-dispatch.md) Decision 8 fixes that the library dispatches the check and decides nothing.
- [FR-150](./FR-150-dispatch-projection-and-realizability-to-fsl.md) and [FR-153](./FR-153-settle-unsupported-while-no-fsl-backend-serves-the-kind.md) state the dispatch and the empty-candidate settlement this requirement reuses.
- [FR-005](./FR-005-dispatch-independent-protocol-claims.md), [FR-029](./FR-029-return-one-disposition-per-requested-claim.md) and [FR-030](./FR-030-bind-each-disposition-to-its-exact-premises.md) supply closed dispatch, independence and premise binding.
- FSL FR-215, FR-217 and FR-338 own the checker, its replay and its certificate check. FSL FR-636 names the item; FSL's engine manifests advertise no `global-conformance` kind, which is an FSL gap this requirement records and does not fill.
- FSpec FR-785 owns the capacity binding and its admission.
