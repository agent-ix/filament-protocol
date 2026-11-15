---
id: FR-200
title: "Decide composition claims in this library"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-002, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-005, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-029, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-030, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-043, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-044, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-301, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-176, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-290, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-489, type: depends_on }
  - { target: ix://agent-ix/filament-spec-language/FR-530, type: references }
---
# FR-200: Decide composition claims in this library

## Description

When a `composition` claim is requested, the dispatcher SHALL decide it with the library's own assume-guarantee procedure over the request's declared guarantees and four premise lists, with no backend call.

## Inputs

One requested `composition` claim (an `ix://agent-ix/filament-specification/FR-290` label) with its admitted fragment, assumptions and source identities. The component subjects, which are checked-package references (FR-043, FR-044). The component guarantees: each guarantee's checked node, its form (safety or liveness), FSL's label for its own atomic or temporal obligation, and the nodes its proof used. The four premise lists of `ix://agent-ix/filament-specification/FR-489`, each entry naming the node that must discharge it.

## Outputs

One source-bound composition disposition retaining its exact admitted fragment, assumptions, premise lists and source identities as FR-030 requires of every disposition, together with the premise discharge record, the undischarged premises, the dependency cycles and the counterexamples the procedure found; or one refusal of that disposition under FR-030.

## Behavior

- The dispatcher shall decide each requested `composition` claim by the procedure FR-201, FR-203 and FR-204 state, with no backend call.
- The dispatcher shall refuse a `composition` claim that names a backend, because no backend decides it.
- The dispatcher shall settle a `composition` claim `proved` only when every premise is discharged, no guarantee is on a dependency cycle and no relied-on proof rests on an assumption the request did not offer.
- The dispatcher shall take FSL's label for each guarantee as given, because FSL owns atomic and temporal obligations.
- The dispatcher shall evaluate no temporal formula and no model-level refinement.
- The dispatcher shall name the verdict by the precedence circular, then incompatible, then unproved, then proved, whatever the order of the declared guarantees and of the nodes each proof used, retaining every finding beside it.
- The dispatcher shall let only a guarantee a premise relies on block success through an unproved premise or a counterexample.
- The dispatcher shall refuse a dependency cycle among any declared guarantees as circular (FR-203), a cycle no premise relies on included, because FSL FR-176 builds the whole dependency graph.
- The dispatcher shall refuse a request of any kind other than `composition` that reaches the composition decision.
- The dispatcher shall refuse a `refinement` request that carries declared guarantees or premise entries.
- The dispatcher shall not settle a `composition` claim `proved` when any relied-on guarantee carries a label other than `proved`, `unsupported` included.
- The dispatcher shall read a decided composition into the assessment-execution, truth and settlement-basis values of `ix://agent-ix/filament-specification/FR-241`, `FR-242` and `FR-243`: `proved` as `completed`, `satisfied`, `decisive-witness`; incompatible as `completed`, `violated`, `decisive-witness`; unproved as `completed`, `pending`, `unsettled`; circular as `refused`, `unavailable`, `unavailable`.
- When a composition claim settles `proved` or incompatible without a source identity, the dispatcher shall refuse that disposition (FR-030).
- When a composition claim is refused, the dispatcher shall leave the independent global conformance result and every other claim's disposition unchanged (FR-029).
- The dispatcher shall not carry the complete-global-conformance premise set as evidence for a composition disposition's own success, as `ix://agent-ix/filament-specification/FR-290` requires of every non-global claim.

This requirement reverses the dispatch FR-200 stated before ADR-010 Decision 6: FSL's planner (FSL FR-530) settles an FSL `composition-decl` and decides no composition of this library's component protocols. FR-301 states the dispatch of `refinement`, which is unchanged.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-200-AC-1 | Given a composition claim whose premises are all discharged by proved guarantees and offered assumptions, with a recording stand-in registered for the composition capability, the disposition is `proved` with a discharge record naming each premise's discharger, and the stand-in receives no call. | Test (TC-200) |
| FR-200-AC-2 | Given a composition claim and a boundary serving no capability, the claim is decided and is never `unsupported-requested-capability`; given a composition claim that names a backend, the disposition is refused naming that backend. | Test (TC-200) |
| FR-200-AC-3 | Given a refused composition claim requested beside finite global conformance and a refinement claim, the conformance disposition and the refinement disposition equal those returned without the composition claim. | Test (TC-200) |
| FR-200-AC-4 | A composition disposition never carries the complete-global-conformance premise set as evidence for its own success, and a successful global conformance result does not change a composition disposition. | Test (TC-200) |
| FR-200-AC-5 | Removing or substituting the composition disposition's admitted fragment, assumption identities, source identity or a guarantee's node one at a time refuses only that disposition (FR-030) and leaves every independent disposition unchanged. | Test (TC-200) |
| FR-200-AC-6 | Changing one guarantee's FSL label from `proved` to `unsupported` or `inconclusive` changes a `proved` composition to an unproved one naming the premise and the label, and the composition is never `proved` while a relied-on guarantee is not `proved`. | Test (TC-200) |
| FR-200-AC-7 | The four verdicts read into the execution, truth and settlement-basis values the Behavior section states, and only `proved` reads as `satisfied`. | Test (TC-200) |
| FR-200-AC-8 | The composition decision refuses a request whose kind is not `composition`, and a `refinement` request carrying declared guarantees or premise entries is refused, each with a typed refusal. | Test (TC-200) |
| FR-200-AC-9 | A guarantee no premise relies on that is not `proved` leaves an otherwise discharged composition `proved`, while a cycle among guarantees no premise relies on makes it `circular`. | Test (TC-200) |

## Dependencies

- [ADR-010](../adr/ADR-010-consume-fsl-protocol-analysis-own-assessment-and-dispatch.md) Decision 6 fixes that composition is decided here and atomic and temporal obligations stay FSL's.
- [FR-301](./FR-301-admit-protocol-refinement-dispatch.md) states the dispatch of the `refinement` kind.
- [FR-005](./FR-005-dispatch-independent-protocol-claims.md), [FR-029](./FR-029-return-one-disposition-per-requested-claim.md) and [FR-030](./FR-030-bind-each-disposition-to-its-exact-premises.md) supply closed dispatch, independence and premise binding; [FR-040](./FR-040-emit-no-success-beside-fatal-refusal.md) supplies no success beside a refusal.
- [FR-043](./FR-043-admit-the-checked-package-handoff.md) and [FR-044](./FR-044-link-from-a-typed-read-view-of-the-checked-package.md) supply the checked-package references, and [FR-002](./FR-002-link-finite-protocol-representation.md) the linked graph.
- `ix://agent-ix/filament-specification/FR-176` and `ix://agent-ix/filament-specification/FR-489` own the composition contract and the four premise lists. FSL FR-530 owns the FSL planner this procedure does not call.
- `ix://agent-ix/filament-specification/FR-290` supplies the claim kind.
