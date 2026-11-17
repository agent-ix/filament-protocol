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

One disposition per requested claim (FR-029), carrying FSL's terminal record of the replay (FSpec FR-331) when FSL proved the trace, FSL's refusal with its catalog code when FSL declined it, or FSL's failure cause, bound to the exact fragment, backend, assumptions and source identities the claim was requested against (FR-030).

## Behavior

- The dispatcher shall take the candidate set of a `finite-replay` claim from FSL's registry snapshot under `ix://agent-ix/filament-specification/FR-290`, as for the other kinds, and settle the claim `unsupported` while no accepted backend serves it (FR-153).
- The dispatcher shall send the claim to the one candidate.
- The provider shall run FSL's `analyze` entry on the supplied request.
- The dispatcher shall carry a replay FSL proved as truth `satisfied` on basis `closed-scope` with FSL's `proved` terminal record unchanged, the replay depth being the record's basis.
- The dispatcher shall carry a trace FSL declined (a step not enabled at the replayed state, a post-state other than the recorded one, an unreadable or malformed document) as FSL's refusal with FSL's `invalid_runtime_input` code unchanged, and derive no verdict.
- The dispatcher shall require the terminal record. A `finite-replay` result that carries no terminal record settles `unsupported` with a warning naming the required capability, and a result whose record is not FSL's `proved` one, or whose truth or basis state another settlement, is refused with `ReplayWithoutItsRecord` (the carriage rule of FR-302 applied to the replay record).
- The dispatcher shall refuse a `finite-replay` result that carries a separating witness, because a replay has none.
- The dispatcher shall refuse the retained result when the claim retains no source identity.
- The provider shall refuse a request whose item is not one `finite-replay` item, and a request of another kind than the claim's, with FSL's `invalid_capability`/`inapplicable-kind` refusal.
- The provider shall carry a failure of FSL's `analyze` entry to admit the request as a typed `AnalyzeRefused` cause.
- The library shall build no mapping from observation records to role, channel, join or loop facts; that remains FR-207's blocked half.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-303-AC-1 | Given FSL's linked engine registered and a trace the protocol replays step by step, the `finite-replay` claim settles truth `satisfied` on basis `closed-scope`, the selected backend is `protocol-analysis`, and the disposition carries FSL's `proved` terminal record, whose basis is the trace's length and which equals the record FSL's `analyze` returns for the same request. | Test (TC-178) |
| FR-303-AC-2 | Given the same registration and a trace whose step is not enabled at the replayed state, the claim is refused with FSL's `invalid_runtime_input`/`invalid-value` code, holds neither truth `satisfied` nor `violated`, and carries no result. | Test (TC-178) |
| FR-303-AC-3 | Given a trace document with a wire defect (a `send` step without its `send_record`), the claim is refused with FSL's `invalid_runtime_input`/`missing-member` code before any step replays. | Test (TC-178) |
| FR-303-AC-4 | Given FSL's linked engine registered, a `finite-replay` claim for which the supplied request is a `realizability` request, and a `realizability` claim for which the supplied request is a `finite-replay` request, are each refused with FSL's `invalid_capability`/`inapplicable-kind` code and carry no result. | Test (TC-178) |
| FR-303-AC-5 | Given an empty registry, a `finite-replay` claim settles `unsupported` with the warning naming the claim kind and no backend is called. | Test (TC-178) |
| FR-303-AC-6 | Given a stand-in serving `finite-replay` whose result carries no terminal record, the claim settles `unsupported` with a warning naming the kind; given one whose record is not `proved`, one whose truth is not `satisfied`, one whose basis is not `closed-scope`, and one that carries a separating witness, each is refused and carried as no result. | Test (TC-178) |

## Dependencies

- [ADR-010](../adr/ADR-010-consume-fsl-protocol-analysis-own-assessment-and-dispatch.md) records that FSL owns the decision procedure and this library the dispatch.
- [FR-150](./FR-150-dispatch-projection-and-realizability-to-fsl.md) supplies the dispatch and the engine provider this requirement extends with a kind.
- [FR-153](./FR-153-settle-unsupported-while-no-fsl-backend-serves-the-kind.md) supplies the `unsupported` settlement while no backend serves the kind.
- [FR-302](./FR-302-carry-trace-position-witness.md) supplies the record carriage rule.
- [FR-207](./FR-207-replay-a-serialized-observed-history.md) keeps the serialized observed-history half, which waits on QSL-2210.
