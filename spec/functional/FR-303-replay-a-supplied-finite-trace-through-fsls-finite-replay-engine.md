---
id: FR-303
title: "Replay a supplied finite trace through FSL's finite-replay engine"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-005, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-029, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-030, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-150, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-153, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-207, type: references }
  - { target: ix://agent-ix/filament-protocol/FR-302, type: references }
  - { target: ix://agent-ix/filament-specification/FR-290, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-331, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-433, type: references }
  - { target: ix://agent-ix/filament-spec-language/FR-621, type: depends_on }
  - { target: ix://agent-ix/filament-spec-language/FR-636, type: depends_on }
---
# FR-303: Replay a supplied finite trace through FSL's finite-replay engine

## Description

When a `finite-replay` claim is requested for a protocol with a finite trace the caller supplies, and an accepted FSL analysis backend serves that claim kind, the dispatcher SHALL send the claim to that backend and carry its terminal record unchanged. The trace, its reading and its replay are FSL's (FSL FR-636, FSpec FR-433); this library reads no trace and replays no step.

## Inputs

One requested `finite-replay` claim with its admitted fragment, optional named backend, assumptions and source identities; FSL's `analyze` request for the claim, in FSL's own types, whose one item is a `finite-replay` item naming the supplied trace document and whose provisioned documents hold that document's bytes; and the FSL registry snapshot, from which `ix://agent-ix/filament-specification/FR-290` computes the claim's candidate set.

## Outputs

One disposition per requested claim (FR-029), carrying FSL's terminal record of the replay (FSpec FR-331) when FSL settled the trace, FSL's declined terminal record (its catalog code and its diagnostic detail, the step position or the document path) when FSL declined it, the typed `unsupported` path when FSL reports the item unsupported, or FSL's failure cause, bound to the exact fragment, backend, assumptions and source identities the claim was requested against (FR-030).

## Behavior

- The dispatcher shall take the candidate set of a `finite-replay` claim from FSL's registry snapshot under `ix://agent-ix/filament-specification/FR-290`, as for the other kinds, and settle the claim `unsupported` while no accepted backend serves it (FR-153).
- The dispatcher shall send the claim to the one candidate.
- The provider shall run FSL's `analyze` entry on the supplied request.
- The dispatcher shall carry a replay FSL settled (`proved`, or `refuted` once FSL emits it) with FSL's terminal record unchanged and the truth and settlement basis FSL's outcome axes give that record (`OutcomeAxes::of_record`, FSL FR-622): `proved` is `satisfied` on `closed-scope`, `refuted` is `violated` on `decisive-counterexample`. The dispatcher adds no truth or basis rule of its own.
- The dispatcher shall carry a trace FSL declined (a step not enabled at the replayed state, a post-state other than the recorded one, an unreadable or malformed document) as a refusal with FSL's `invalid_runtime_input` code and FSL's declined terminal record unchanged, so the position and the path stay readable, and derive no verdict.
- The provider shall settle an item FSL reports unsupported (including one disposed `requires-bound`, whose terminal record holds no domain) on the typed `unsupported` path of FR-153 with FSL's diagnostic, and a value FSL settles otherwise (`failed`, `incomplete`, `inconclusive`) as a failure carrying FSL's diagnostic.
- The dispatcher shall require the terminal record. A `finite-replay` result that carries no terminal record settles `unsupported` with a warning naming the required capability, and a result whose record is not a `proved` or `refuted` one, or whose truth or basis state other than FSL's outcome axes give that record, is refused with `ReplayWithoutItsRecord` (the carriage rule of FR-302 applied to the replay record).
- The dispatcher shall refuse a `finite-replay` result that carries a separating witness, because a replay has none.
- The dispatcher shall refuse the retained result when the claim retains no source identity.
- The provider shall refuse a request whose item is not one `finite-replay` item, and a request of another kind than the claim's, with FSL's `invalid_capability`/`inapplicable-kind` refusal.
- When the item's claim is none of the dispatched claim's source identities in the package the provider is bound to, the provider shall run nothing and fail with `ReplayNotTheBoundClaim`.
- The provider shall carry a refusal of FSL's `analyze` entry to admit the request as `AnalyzeRefused` with the `Display` text of FSL's refusal, and a limit, a cancellation or an internal fault that stops it as `AnalyzeStopped` with FSL's own value, an incomplete run and not a refusal.
- The library shall build no mapping from observation records to role, channel, join or loop facts; that remains FR-207's blocked half.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-303-AC-1 | Given FSL's linked engine registered and a trace the protocol replays step by step, the `finite-replay` claim settles truth `satisfied` on basis `closed-scope`, the selected backend is `protocol-analysis`, and the disposition carries FSL's `proved` terminal record, whose basis is the trace's length and which equals the record FSL's `analyze` returns for the same request. | Test (TC-178) |
| FR-303-AC-2 | Given the same registration and a trace whose step is not enabled at the replayed state, the claim is refused with FSL's `invalid_runtime_input`/`invalid-value` code and FSL's declined record, whose detail names the step position; a trace naming an initial state the subject lacks has the same code and a detail naming `/initial_state` and no position. The claim holds neither truth `satisfied` nor `violated` and carries no result. | Test (TC-178) |
| FR-303-AC-3 | Given a trace document with a wire defect (a `send` step without its `send_record`), the claim is refused with FSL's `invalid_runtime_input`/`missing-member` code and a detail naming `/prefix/0/send_record` and position 0, before any step replays. | Test (TC-178) |
| FR-303-AC-4 | Given FSL's linked engine registered, a `finite-replay` claim for which the supplied request is a `realizability` request, and a `realizability` claim for which the supplied request is a `finite-replay` request, are each refused with FSL's `invalid_capability`/`inapplicable-kind` code and carry no result. | Test (TC-178) |
| FR-303-AC-5 | Given an empty registry, a `finite-replay` claim settles `unsupported` with the warning naming the claim kind and no backend is called. | Test (TC-178) |
| FR-303-AC-6 | Given a stand-in serving `finite-replay` whose result carries no terminal record, the claim settles `unsupported` with a warning naming the kind; given one whose record is not a `proved` or `refuted` one, one whose truth or basis is not the one FSL's outcome axes give its record, and one that carries a separating witness, each is refused and carried as no result. | Test (TC-178) |
| FR-303-AC-7 | Given a `finite-replay` item whose disposition is `unsupported`, the claim settles on the typed `unsupported` path (truth and basis `unavailable`) and not as a failure. | Test (TC-178) |
| FR-303-AC-8 | The truth and basis of a replay record are FSL's outcome axes: `proved` gives `satisfied` on `closed-scope`, `refuted` gives `violated` on `decisive-counterexample`, and a value that settles nothing gives `unavailable`. FSL emits no `refuted` replay record yet, so the `refuted` row is asserted on FSL's axes over the value alone. | Test (TC-178) |
| FR-303-AC-9 | Given a replay request whose item is for another protocol's clause than the dispatched claim's, the engine runs nothing and the claim fails with `ReplayNotTheBoundClaim`. | Test (TC-178) |
| FR-303-AC-10 | Given a request FSL's `analyze` refuses to admit (an unprovisioned source), the claim fails with `AnalyzeRefused` holding the `Display` text of FSL's refusal. | Test (TC-178) |

## Dependencies

- [ADR-010](../adr/ADR-010-consume-fsl-protocol-analysis-own-assessment-and-dispatch.md) records that FSL owns the decision procedure and this library the dispatch.
- [FR-150](./FR-150-dispatch-projection-and-realizability-to-fsl.md) supplies the dispatch and the engine provider this requirement extends with a kind.
- [FR-153](./FR-153-settle-unsupported-while-no-fsl-backend-serves-the-kind.md) supplies the `unsupported` settlement while no backend serves the kind.
- [FR-302](./FR-302-carry-trace-position-witness.md) supplies the record carriage rule.
- [FR-207](./FR-207-replay-a-serialized-observed-history.md) keeps the serialized observed-history half, which waits on QSL-2210.
