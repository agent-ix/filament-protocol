---
id: ADR-010
title: "Consume FSL's protocol analysis; own only linking, assessment and dispatch"
type: ADR
status: accepted
relationships:
  - target: ix://agent-ix/filament-protocol/AD-001
    type: relates_to
  - target: ix://agent-ix/filament-protocol/ADR-007
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-002
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-004
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-005
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-029
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-030
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-301
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-302
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-205
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-206
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-207
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-208
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-209
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-210
    type: relates_to
  - target: ix://agent-ix/filament-protocol/FR-211
    type: relates_to
  - target: ix://agent-ix/filament-spec-language/ADR-017
    type: depends_on
  - target: ix://agent-ix/filament-spec-language/ADR-020
    type: depends_on
  - target: ix://agent-ix/filament-spec-language/ADR-027
    type: depends_on
---
# ADR-010: Consume FSL's protocol analysis; own only linking, assessment and dispatch

## Status

Accepted; Decision 6 reverses the composition assumption on 2026-11-15 and Decision 7 the protocol refinement assumption on 2026-11-16. Decision 8 (2026-11-16) amends the assumptions that FR-004, FR-006, FR-007 and FR-017 stay in force: they were retired with the evaluator (FSL-1549), and FR-205 to FR-211 succeed them. This record fixes the split between this library and FSL's
`fsl-analyze` for the dynamic, projection, realizability, composition and
refinement extension. It authors no requirement; it reserves requirement-id
blocks and names, per capability, what this library owns and what it consumes.
Capabilities FSL does not yet provide are listed in
[FSL gaps](#fsl-gaps-for-the-fsl-code-lead) and are not worked around here.

## Context

The extension tickets TL-6, TL-8, TL-9, TL-10, TL-11 and TL-12 were written
against a FSpec revision in which this library was to implement dynamic roles,
bounded queues, joins, loops, projection, realizability, assume-guarantee
composition and refinement itself. Since then FSL has taken over parts of that
work, and comments of 2026-11-10 on three of those tickets say that
realizability, projection and refinement "now live in FSL". That statement was
checked against the code instead of adopted.

### What this library contains

`src/` holds linking (`link.rs`, `checked_package.rs`), compensation
assessment (`compensation/`), closure and evidence (`closure.rs`,
`evidence.rs`), refusal, bounds, digest and version-lock code. A search of
`src/` for projection, realizability, refinement, quorum and capacity finds no
protocol-analysis code. `link.rs` exposes the checked package's queue nodes as
read-only view data; no queue is simulated. The library has no dispatch module
for the non-conformance claim kinds yet; FR-005, FR-029, FR-030 and FR-301
specify one. It depends on FSL for the checked package and for temporal
evidence (FR-043, FR-044, FR-046).

### What FSL provides, by capability

Read from FSL `main` on 2026-11-10.

| Capability | What the code has | Where |
| --- | --- | --- |
| Dynamic roles | Replicated roles with `spawn`, `retire`, lifetimes `workflow`, `scope`, `until(p)`, a `max` bound, `activation on each` with `max_live_instances` (ADR-027 RR-1..4, ST-13..15) | `fsl-eval/src/simulation/protocol/roles.rs` |
| Bounded queues | `send`, `receive`, `duplicate`, `lose` steps; literal capacity; overflow `block`, `reject`, `loss head/tail/new`; `fifo` per key and `unordered` (ADR-027 ST-3..5, PS-7) | `fsl-eval/src/simulation/protocol/channel.rs` |
| Joins | `parallel` fork/join with `all`, `any`, `quorum(n)` and `outstanding continue/cancel` (ADR-027 ST-6..8, PS-4) | `fsl-eval/src/simulation/protocol/program.rs`, `steps.rs` |
| Loops | `repeat` with iteration counters and a progress check on the body graph (ADR-027 PS-6); `invariant`/`variant` blocks are parsed and lowered into the checked package | `fsl-eval/.../program.rs`; `fsl-forms/src/protocol_control.rs`; `fsl-semantics/src/check/lowering/protocol/controls.rs` |
| Compensation in a model subject | Registration, activation, attempts, commit cutoff, `cend` (ADR-027 CP-1..5) | `fsl-eval/src/simulation/protocol/compensation.rs` |
| Missing bounds | A requires-bound pre-check before exploration | `fsl-analyze/src/model_check/check.rs` |
| Refinement (model-level) | Safety half on the explicit-state product, liveness half under abstract fairness, a protocol as the abstract side, counterexample replay, simulation certificate (ADR-020; FSL FR-142..149) | `fsl-analyze/src/model_check/refine.rs`, `fsl-replay/src/refinement/protocol_system.rs` |
| Assume-guarantee composition | Planner that discharges `rely` and `liveness-dependency` slots from proved guarantees and fairness implication, and writes the discharge record (FSL FR-530) | `fsl-replay/src/planner/compose.rs` |
| Local projection, realizability | Claim-kind vocabulary (`local-projection`, `realizability`), the protocol-family applicability gate and the routing key only. No engine. A request for either kind that no engine serves settles `unsupported` with `unsupported_projection` / `unsupported-requested-capability` | `fsl-semantics/src/check/capability.rs`, `fsl-route/src/lib.rs`, `fsl-analyze/src/analyze.rs` |

The `Capability::LocalProjection`, `Refinement` and `Realizability` entries in
`fsl-analyze/src/analyze.rs` that the planner comments cite sit in the test
`protocol_kinds_apply_only_to_protocol_claims`. That test proves that the
seven protocol kinds apply to the protocol family and to no other. It is
evidence that the kinds exist and are gated, not that an engine decides them.

### Verdict on the 2026-11-10 staleness comments

- "This library has no projection, queue or refinement code": true.
- "Refinement lives in FSL": true.
- "Realizability and projection live in FSL": true of the claim vocabulary
  and routing, false of any decision procedure. FSL has none. TL-10 is
  therefore not closable as "provided by FSL"; the capability is a FSL gap.
- "Queues, joins, dynamic roles and bounded loops live in FSL": true as an
  exploration semantics over a model subject (FSL decides what a protocol can
  do), with the gaps listed below. It is not an assessment of an observed run,
  which stays this library's job (FR-004).

FSL explores a protocol as a transition system and decides claims about every
behaviour. This library assesses one admitted observation history against the
linked protocol (FR-004) and dispatches claims to the backends that decide
them (FR-005). Those are different questions, and the split follows them.

## Decision

1. **The library implements no projection, realizability or model-level
   refinement decision procedure, and no queue, role, join or loop
   exploration semantics.** It consumes FSL's. It builds no product
   construction, strategy game or fixed-point engine. The protocol-level
   assume-guarantee composition procedure (Decision 6) and the
   protocol-level refinement procedure, whose product is built over FSL's
   step view (Decision 7), are the exceptions. This is ADR-007 and
   AD-001's boundary table applied to the extension. The protocol and
   frame boundaries FSL fixes for the checked package, which this library
   reads and does not redefine, are FSL ADR-017.

2. **The library owns exactly four things for these capabilities.**
   - *Linking:* retaining each authored role, population bound, channel
     premise, capacity, overflow policy, join policy, loop declaration and
     control identity from the checked package without minting identity
     (FR-002, FR-044).
   - *Assessment:* judging an observed history against the authored role
     lifetimes, queue bounds and premises, join thresholds, loop bounds and
     compensation relations, with the progress, closure and completeness axes
     kept independent (FR-053..FR-056, FR-019..025, FR-208..FR-211; FR-004,
     FR-026..028 and FR-041 are retired, Decision 8).
   - *Dispatch:* one disposition per requested claim kind, bound to its exact
     fragment, backend, assumptions and premises, with explicit `unsupported`
     where no FSL engine serves the kind (FR-005, FR-029, FR-030).
   - *Carrying:* carrying FSL's result, refusal, counterexample and witness
     unchanged, joined to the linked graph's source identities (FR-302,
     FR-205; FR-006 is retired and FR-206 succeeds it, Decision 8).

3. **Unsupported FSL constructs are carried, not repaired.** A FSL
   `Unsupported` cause (symbolic capacity, join predicate, a join with no
   `outstanding` rule, and the others listed below), a requires-bound outcome or
   an unserved kind becomes a typed non-success disposition that names the
   construct. The library never truncates a bound to finish, never substitutes
   a local approximation and never reports `unsupported` as false, satisfied or
   successful.

4. **FR-301's "explicit-state product construction" was assumed to be FSL's.**
   This decision is superseded by Decision 7: the protocol-level product is
   built in this library. FSL's `check_refinement` (ADR-020 RE-1) stays the
   model-level check.

5. **Requirement-id blocks.** Later waves allocate local FR ids in these
   blocks, one per capability family. Ids are allocated within a block
   and never across blocks.

   | Block | Capability family | Ticket |
   | --- | --- | --- |
   | FR-050 to FR-099 | Dynamic roles, bounded queues, joins, loops | TL-11 |
   | FR-100 to FR-149 | Core roles, messages, channels, control, compensation, replay | TL-6 |
   | FR-150 to FR-199 | Projection, realizability | TL-10 |
   | FR-200 to FR-249 | Assume-guarantee composition, refinement | TL-9 |

   FR-301 gains FR-301-AC-6 (result carried unchanged). FR-301 and FR-302 stay where they are. The FR-050 to FR-054 ids that
   artifacts cite as "source requirements" are FSpec ids, written
   `ix://agent-ix/filament-specification/FR-0nn`; they are not local ids and do
   not collide with these blocks.

6. **Reversal for composition (2026-11-15): this library decides the
   protocol-level assume-guarantee composition procedure.** The table above
   and Decision 1 assumed FSL serves composition. FSL does not: FSL FR-530's
   planner settles an FSL `composition-decl` from FSL result entries and
   writes the discharge record, FSL FR-530 adds no TL-9 algorithm, FSL FR-147
   never kinds a model-refines-protocol check `refinement`, and
   `open_systems_inner` refuses concrete protocol sides. No FSL API decides a
   composition of this library's component protocols. The library therefore
   owns, in `composition`: cycle detection over the declared guarantee
   dependencies (strongly connected components); discharge of a premise only
   from a proved guarantee outside every cycle, of the matching form; and a
   paired, replayable counterexample when a relied-on proof rests on an
   assumption the request did not offer. FSL still owns every atomic and
   temporal obligation: the requester carries FSL's label for each guarantee,
   and the library evaluates no temporal formula and no model-level
   refinement. The library computes no fixed point; a cycle is refused as
   circular reasoning (FSL FR-176).

7. **Reversal for protocol refinement (2026-11-16): this library decides the
   protocol-level refinement procedure.** Decision 4 assumed FSL's
   `check_refinement` serves a `refinement` claim over two protocols. It
   does not: FSL's refinement is a model-level check over a model subject
   (FSL FR-142, FR-147: a model that refines a protocol is never kind
   `refinement`), and no FSL API relates two protocols. The library therefore
   owns, in `refinement`: observable trace inclusion by subset construction
   over the specification's internal-step closure, with a paired replayable
   counterexample for an extra trace; deadlock; divergence; the
   stronger-assumption finding (FR-204); and `unsupported` for a replicated
   role, a symbolic queue capacity or a construct a system's builder could not
   step, which never refines. The precedence is unsupported, refuted,
   incomplete, refines. The transition system is this library's own small
   type, derived from FSL's read-only protocol step view (FSL FR-578, S3), with
   the observable steps defined by FR-301. Atomic and
   temporal obligations and model-level refinement stay FSL's. FR-301 and
   FR-202 state the procedure; the dispatch of a `refinement` claim to a
   backend is removed with the `Claim`/`Answer` plumbing it needed.

8. **Amendment for the retired evaluator (2026-11-16): global conformance,
   the scenario record and serialized replay have successors.** Decisions 1
   and 2 and the context above assumed this library assesses finite global
   conformance (FR-004) and produces the canonical result and scenario record
   (FR-006, FR-007, FR-017). FSL-1549 retired all of them with the evaluator,
   and the TL-8 cells they backed (V1-PROT-025, V1-PROT-026, V1-RUN-005,
   V1-RUN-006) are not dropped.
   - *Global conformance (V1-PROT-026, FR-205).* FSL does decide a question it
     calls `global-conformance` (FSL FR-636): its explicit-state checker EN-1
     explores every behaviour of a protocol subject (`check_protocol`) and
     settles the outcome with a certificate check and a counterexample replay
     (`settle_protocol_check`). The library dispatches that check and carries
     FSL's record unchanged. It decides no deadlock, evaluates no temporal
     formula and adds no refinement (Decision 1; FSL FR-530 and FR-147 stand).
     The check answers whether every behaviour is deadlock-free. The history
     questions of retired FR-004 are this library's, decided in `history` over
     the live assessment axes of `dynamics` and the closure algebra of
     `closure`, as composition and refinement are (Decisions 6, 7): order
     invariance (FR-208), adverse observations named by axis (FR-209), the five
     closure axes kept independent of the history's truth (FR-210), and batch
     against incremental agreement (FR-211). They need no FSL engine: FSL
     explores a protocol, and these judge one admitted history (see Context).
   - *Scenario record (V1-RUN-005, V1-RUN-006, FR-206).* The shape is Quoin's
     (`quoin.protocol-scenario-intake/v1`, FR-130 in `quoin-evidence`); this
     library composes the value from its live dispositions and declares none
     of the shape. That needs `quoin-evidence` as a dependency, which cannot
     resolve beside `filament-observation` today (gap G11).
   - *Serialized replay (V1-PROT-025, FR-207).* No owner publishes a
     serialization of an observed history; the library declares none (gap
     G10).

## Per-ticket ownership

| Ticket | Provided by FSL | Remains Protocol-owned | Planner action |
| --- | --- | --- | --- |
| TL-11 dynamic roles, bounded queues, joins, proved loops | Exploration semantics for all of them (table above), except FSL gaps G3 to G5 | FR-050 to FR-099: link the role, queue, join and loop declarations; assess an observed history against role lifetime and population bound, queue capacity and overflow outcome, join settlement per policy, loop bound; map FSL `Unsupported` and requires-bound outcomes to typed dispositions (Decision 3) | Rewrite as assessment and carrying work over FSL's semantics; drop "implement exploration" and "loop invariants and variants" from this repository |
| TL-6 core roles, messages, channels, control, compensation, replay | Compensation and channel steps for model subjects | FR-100 to FR-149: only the delta over the existing FR-002, FR-003, FR-004, FR-019 to FR-028 and FR-041 coverage. Each new requirement names the V1-PROT capability it closes and the existing requirement it extends; a capability those FRs already cover gets no new FR | Rewrite as a delta ticket; close the rows the existing FRs already cover |
| TL-10 projection and realizability | Claim vocabulary and routing only. No engine (gaps G1, G2) | FR-150 to FR-199: dispatch of both kinds to FSL's backend once one exists; premise binding for communication and visibility premises; carrying a FSL witness with trace-position correspondence to the linked graph (FR-302); `unsupported` while no backend exists, with global conformance preserved | Rewrite to dispatch and carrying only; the decision procedures move to a ticket for the FSL code lead (G1, G2); keep blocked by that ticket |
| TL-9 composition and refinement | Model-level refinement engine, protocol abstract side, replay, certificate; atomic and temporal obligations. FSL provides no protocol-level composition or refinement procedure (Decisions 6, 7) | FR-200 to FR-249: the protocol-level composition decision (cycles, discharge, stronger assumptions; Decision 6) and refinement decision (traces, deadlock, divergence, stronger assumptions; Decision 7); premise binding; paired counterexamples | Both decided here (FR-200..FR-204, FR-301) |
| TL-8 reference workflows, integration, qualification | Backends for the corpora where FSL has them | Integration tests and evidence for each Protocol-owned cell, as IT and TC artifacts that cite the FR blocks above; and FR-205 to FR-211, the successors of the retired FR-004, FR-026..028, FR-041, FR-006, FR-007 and FR-017 (Decision 8) | Keep as the last layer; scope corpora to the capabilities that have backends. V1-PROT-025 and V1-RUN-005/006 stay blocked on G10 and G11 |
| TL-12 adopt the extension and delivery DAG | Not applicable | This record is the adoption: it fixes the split, reserves the blocks and names ownership. No further adoption artifact is owed | Close when this record merges |

## Consequences

- The extension shrinks from implementing five analyses to linking, assessing,
  dispatching and carrying. Wave sizing for FR-050 to FR-249 follows from that.
- Projection and realizability cannot ship in this library until FSL provides
  an engine. Until then the library returns `unsupported` for them, as FR-029
  and FR-060 (FSpec) already require, and global conformance remains available.
- The library keeps no copy of FSL's step semantics. Where a requirement here
  must state a threshold or overflow rule for assessment, it cites the FSpec
  clause (FR-170, FR-171) and the FSL record (ADR-027) and does not restate them.
- Composition is decided in `composition`, not dispatched (Decision 6);
  FR-200 to FR-204 state it.
- FR-301, spec.md section 2, AD-001 and TC-147 lose their wording about a
  library-built product construction. Their dispatch, independence and
  premise-binding obligations are unchanged.

## FSL gaps for the FSL code lead

Capabilities this extension needs that FSL `main` does not provide, as read on
2026-11-10. Each is a candidate ticket for the FSL code lead; this repository
does no FSL-side work.

| Id | Gap | Evidence | Needed by |
| --- | --- | --- | --- |
| G1 | No decision procedure for `local-projection`: role-local projection with send/receive correspondence, queue order and merge visibility | Only vocabulary, applicability and routing exist; an unserved request settles `unsupported_projection` / `unsupported-requested-capability` | TL-10 |
| G2 | No decision procedure for `realizability`: local-history strategy game, realizable/unrealizable witnesses, non-conclusive outcome on missing premises | Same as G1 | TL-10 |
| G3 | `join predicate(...)` was not stepped and a join with no `outstanding` rule was refused (both closed in FSL by QSL-1885 and QSL-1891; this library still does not assess a predicate join) | `fsl-eval/src/simulation/protocol/program.rs` | TL-11 |
| G4 | Symbolic channel capacity is not stepped (`Unsupported::SymbolicCapacity`); only literal capacity is | Same file | TL-11 |
| G5 | No discharge of a `repeat`'s `invariant` and `variant` (lexicographic variants included) was found. They parse and lower into the checked package, but a search of `fsl-analyze`, `fsl-eval`, `fsl-replay` and `fsl-route` found no engine that proves them | `fsl-forms/src/protocol_control.rs`, `fsl-semantics/.../controls.rs` | TL-11 |
| G6 | Declared least-fixed-point cycles: the composition planner computes no fixed point and discharges a cycle only from a `fixed-point` slot holding a proved result. No producer of that result was found. Closed here by Decision 6: this library computes no fixed point either and refuses a cycle as circular | `fsl-replay/src/planner/compose.rs` | TL-9 |
| G7 | Other protocol-system constructs that are not stepped and refuse today: `activation on each` with a compensation template, a protocol capture, an `unordered` channel with a `head` or `tail` loss selector, and a step binder of an unenumerable record type | `Unsupported` in `fsl-eval/src/simulation/protocol.rs` | TL-11, TL-6 |
| G8 | No engine advertises the `global-conformance` kind. FSL FR-636 names `global-conformance` for protocol model checking, but EN-1's manifest advertises only `temporal-satisfaction`, and `protocol-analysis` only `local-projection` and `realizability`. The library selects on `temporal-satisfaction` (FR-205). FSL should advertise the kind or state that the item is a `temporal-satisfaction` item | `fsl-analyze/src/model_check/manifest.rs`, `protocol_analysis/manifest.rs` | TL-8 |
| G9 | EN-1's protocol check cannot be settled `refuted` in process: `settle_protocol_check` takes a replay, and FSL's `replay_protocol_trace` recompiles from a `ReplayRequestWire` that holds source, which a checked handoff does not carry (realizability has an in-process `replay_counterexample`). The refusal map `model_refusal` is crate-private, so an EN-1 refusal reaches this library with no code | `fsl-replay/src/model_check/replay.rs`, `fsl-analyze/src/analyze/items.rs` | TL-8 |
| G10 | No owner publishes a serialized observed history. FSpec FR-433 spells a protocol counterexample trace (steps, not observations); FSL's `finite-replay` item (FSL FR-636) that would replay a supplied trace is not implemented and `fsl-replay` reads no such spelling; no owner defines how an observation record becomes a role, channel, join or loop fact | `fsl-replay/src/planner/key.rs` (kind only) | TL-8 |
| G11 | `quoin-evidence` (the owner of `ScenarioEvidence`) pins `serde =1.0.229`; `filament-observation` pins `serde =1.0.228`. Both are exact, so no manifest depends on both | `quoin/rust/Cargo.toml`, `filament-observation/Cargo.toml` | TL-8 |

G5 and G6 are findings from a negative search. The FSL code lead should confirm
or correct each before opening a ticket.

## Alternatives considered

**Implement projection, realizability and the rest in this library, as the
tickets were written.** Rejected. It duplicates the transition system FSL
owns (ADR-027), forks the identity and step semantics, and contradicts the
rule that this library has no semantic runtime of its own (AD-001).

**Close TL-10 as provided by FSL.** Rejected. The comment's premise is false
for projection and realizability: FSL has the claim vocabulary and no engine.
Closing would leave the capability with no owner.

**Wait for FSL before reserving blocks.** Rejected. The blocks cost nothing and
let later waves allocate ids independently of the FSL gaps.
