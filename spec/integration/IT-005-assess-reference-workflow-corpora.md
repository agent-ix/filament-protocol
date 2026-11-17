---
id: IT-005
title: "Assess the reference workflow corpora over the Protocol-owned cells"
type: IT
relationships:
  - target: ix://agent-ix/filament-protocol/FR-019
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-021
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-022
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-023
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-050
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-051
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-052
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-053
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-054
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-055
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-056
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-100
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-020
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-303
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-205
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-206
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-211
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-210
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-209
    type: verifies
  - target: ix://agent-ix/filament-protocol/FR-208
    type: verifies
---
# IT-005: Assess the reference workflow corpora over the Protocol-owned cells

## Objective

Show, for each corpus the Protocol-owned V1 cells name, which living tests run it and what each asserts, so every cell has attributed evidence or a stated gap. This library assesses an observed history; FSL owns the exploration engines (ADR-010).

## Target Integration

FSL compiles the choreographies and owns the exploration engines; this library admits, links and assesses an observed history against the authored declarations (ADR-010). It adds no engine.

## Preconditions

FSL's writer and Contract IR's reader are available in process. FSL's linked engines serve `local-projection` and `realizability` only; `composition` and `refinement` are decided by this library (ADR-010 Decisions 6, 7).

## Inputs

The recipe model's dynamic-roles choreography, and finite observed histories of it: a conforming run and a faulty run. The corpora below name the evidence for each cell.

## Test Procedure

Run each living test named in the table. Each corpus row names the cells it backs and the assertion made.

## Corpora

| Corpus | Cells | Run by | What is asserted |
| --- | --- | --- | --- |
| Finite replay | V1-PROT-025, V1-RUN-003 | TC-167; TC-178; TC-008 | A finite observed history is assessed on the role, queue, join and loop axes, and a history rebuilt over a second compilation gives the same result (TC-167). A supplied finite protocol trace, taken from FSL's own protocol system over a handoff-compiled protocol, is replayed by FSL's real `finite-replay` engine: the whole trace settles `satisfied` on `closed-scope` with FSL's `proved` record carried unchanged, a stale or malformed trace is refused with FSL's `invalid_runtime_input` code (TC-178, FR-303, FR-207-AC-3). TC-008 separately types the reproduction record. No test replays a serialized observed history: its reader and assessor (FSL FR-737) wait on QSL-2210, and no owner defines how an observation record becomes a role, channel, join or loop fact (FR-207-AC-1, FR-207-AC-2, ADR-010 G10). |
| Timed refund | V1-RUN-011, V1-PROT-019 | TC-154; TC-170 | Inclusive deadline branches settle only on complete progress; captures bind once, in order. TC-170 runs a refund-shaped run on the generic deadline fixture: a timeout starts compensation and a response starts none; a failed attempt then a successful retry give one effect and no duplicate. |
| Fan-out and join | V1-RUN-012, V1-PROT-010 | TC-053; TC-167; TC-170 | Each join policy, threshold and `outstanding` rule is judged on an observed history; a run that breaks one axis fails on that axis only (TC-167). |
| Retry | V1-PROT-021, V1-PROT-023 | TC-003 (FR-022, FR-023) | The initial attempt counts, the authored retry bound admits exactly and one over refuses; attempts, effects and outcomes stay distinct. |
| Compensation | V1-PROT-020, V1-PROT-022, V1-PROT-024 | TC-003 (FR-019..FR-021); TC-169; TC-176 | Registration needs its matching effect and is replay-idempotent; commit and recovery orderings follow only the authored causal order; a declared population of related effects holds distinct members that are not duplicates (TC-176). |
| Dynamic roles | V1-PROT-012, V1-PROT-013 (Partial) | TC-051, TC-052, TC-167, TC-209 | Lifetime and population bound of a replicated role; queue capacity and overflow; a faulty run is judged on each axis separately. |
| Global conformance (Partial, tied to QSL-2209: the `refuted` settlement is untested) | V1-PROT-026 | TC-205, TC-208 | FSL's linked EN-1 checker decides whether every behaviour of a handoff-compiled protocol is deadlock-free, and the library carries its record unchanged (FR-205): a deadlock-free protocol is proved on `closed-scope`, a deadlock whose counterexample FSL cannot replay is never read as `violated`, and the recipe model as FSL ships it, with its unbounded `Total` and `Amount`, is refused by the real checker as requiring a bound and settles the typed requires-bound disposition with FSL's outcome, domain kinds and keys unchanged (FR-205-AC-5, FR-058; V1-PROT-014). TC-209 binds a `symbolic(<name>)` capacity in the protocol subject: unbound it is the typed requires-bound disposition with FSL's list unchanged, bound FSL decides with the value, and a refused binding is FSL's `invalid_runtime_input` unchanged (V1-PROT-013, Partial: FSL's realizability profile and the refinement and finite-replay requests take no bound capacity yet). TC-208 decides the history half over the choreography TC-167 uses: the same assessment in every permitted order and a reported one in a forbidden order (FR-208), each fault non-conformant on its own axis and an undeclared subject unexpected (FR-209), complete global success only with a conformant history, five established closure axes and a completed execution (FR-210), and batch and prefix-by-prefix assessment agreeing (FR-211). |
| Projection | V1-PROT-027 | TC-164, TC-165 (IT-004) | FSL's linked engine decides it. |
| Realizability | V1-PROT-028 | TC-166, TC-172, TC-173 | FSL's linked engine decides it over a subject built from a checked-handoff directory by FSL's `protocol_subject`; the witness is carried unchanged (FR-302). TC-173 runs the reference workflow family as a table: satisfied, hidden choice and incompatible merge, the witness carried unchanged. |
| Composition, refinement | V1-PROT-029, V1-PROT-030 | TC-200..TC-204, TC-147, TC-173 | Composition and refinement are decided in `src/composition.rs` and `src/refinement.rs` (TC-200..TC-204, TC-147), the refinement over transition systems built from FSL's protocol step view (FSL FR-578). TC-173 runs refinement over the reference workflow family (refines, extra trace, deadlock, divergence, unsupported, incomplete) through `decide` and `dispatch_claims`, and composition over three separately compiled protocols with guarantee labels taken from FSL's engine results (proved, unproved premise, circular, incompatible with a replayable counterexample, order independence). |

## Expected Results

Every corpus row passes. A cell with no row is a stated gap in the V1-PROT inventory of `spec/tests.md`, not an unnamed omission.

## Dependencies

The result-publication cells (V1-RUN-005, V1-RUN-006) are backed: Quoin's intake takes a scenario record, which FR-206 composes from the live dispositions of real engine claims and Quoin's `write_scenario_evidence` accepts (TC-179). The related-instance cell (V1-RUN-009) is backed by filament-observation's relationship tests (the owner resolves the relationships) and TC-154's test, which takes the payment-attempt and refund endpoints of an order from the same static bundle and shows they are other instances than the order; no code of this library consumes a `Relationship` itself. Replay of a supplied finite protocol trace (V1-PROT-025) is FR-303 over FSL's `finite-replay` engine; replay of a serialized observed history stays blocked on QSL-2210 (FR-207, G10). Global conformance (V1-PROT-026) is FR-205 and FR-208..FR-211: FSL advertises no `global-conformance` kind, so it selects on `temporal-satisfaction`, and an EN-1 counterexample cannot be replayed in process (G8, G9). Composition and refinement are decided by this library; refinement over a linked protocol is built from FSL's step view. The `v1-draft.2` witness carrier (FR-302) is landed for realizability and refinement.
