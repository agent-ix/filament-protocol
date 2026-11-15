---
id: FR-202
title: "Carry refinement results without reinterpretation"
type: FR
relationships:
  - { target: ix://agent-ix/filament-protocol/US-001, type: implements }
  - { target: ix://agent-ix/filament-protocol/ADR-010, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-008, type: references }
  - { target: ix://agent-ix/filament-protocol/FR-030, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-200, type: depends_on }
  - { target: ix://agent-ix/filament-protocol/FR-301, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-241, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-242, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-177, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-489, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-360, type: depends_on }
  - { target: ix://agent-ix/filament-specification/FR-243, type: depends_on }
  - { target: ix://agent-ix/filament-spec-language/FR-148, type: references }
  - { target: ix://agent-ix/filament-spec-language/FR-530, type: depends_on }
---
# FR-202: Carry refinement results without reinterpretation

## Description

When a refinement backend returns a result, the library SHALL carry the paired counterexample, certificate, witness or cause in the disposition byte-equal, with the same carry-unchanged semantics FR-301-AC-6 states, deriving no outcome value from what it carries.

## Inputs

A returned refinement result (FR-301): a certificate, a paired counterexample, a witness, or a refusal, `unsupported` or other non-success cause.

## Outputs

The disposition carrying every returned artifact byte-equal.

## Behavior

- The library shall carry each returned certificate, counterexample and witness byte-equal to what the backend returned, as FR-301-AC-6 requires.
- When a refinement backend returns a paired counterexample, the library shall carry both of its halves, the implementation side and the specification side, byte-equal.
- The library shall reconstruct no counterexample half that the backend did not return.
- The library shall carry a returned refusal or non-success cause unchanged and without mapping it to another cause.
- The library shall derive no execution, truth or settlement-basis value for a refinement result, because `ix://agent-ix/filament-specification/FR-177` has no owner table into `ix://agent-ix/filament-specification/FR-241`, `FR-242` and `FR-243`. The missing mapping is an open question for the owner of `filament-specification`, not a rule invented here.
- The library shall derive no truth value from the presence, absence or content of a certificate, counterexample or witness.
- The library shall set the assessment-execution value `refused` on no refinement disposition on its own, because that value states that the evaluator refused the request.
- When FSL's backend returns a refusal of the refinement item, the library shall carry that refusal unchanged.
- The library shall carry a non-success result in no form that presents it as satisfied.

A non-success result includes an unsupported dynamic behaviour or a missing correspondence (`ix://agent-ix/filament-specification/FR-177`), and a backend-reported strengthened assumption (FSL FR-148's `protocol-assumption-not-established`). Each is carried by its own name. A refusal the library itself raises on a disposition under FR-030 is a separate act from carrying a backend's value, and it sets no execution value. A composition is decided here and its reading into the axes is FR-200's.

## Acceptance Criteria

| ID | Criteria | Verification |
| --- | --- | --- |
| FR-202-AC-1 | Given a stand-in refinement result carrying certificate and witness bytes, the disposition carries them byte-equal to the sentinels. | Test (TC-202) |
| FR-202-AC-2 | Given a stand-in refinement counterexample with an implementation half and a specification half, and a stand-in simulation certificate, the disposition carries both halves and the certificate byte-equal. | Test (TC-202) |
| FR-202-AC-3 | Given a stand-in counterexample with only one half, the disposition carries that half and no second half appears. | Test (TC-202) |
| FR-202-AC-4 | Given stand-in results carrying `unsupported-requested-capability`, `protocol-assumption-not-established` and a cause name the library does not recognise, each cause appears in its disposition unchanged, none is mapped to another, and none carries a success value. | Test (TC-202) |
| FR-202-AC-5 | A refinement result of any label receives no derived execution, truth or settlement-basis value, including a label carrying certificate-shaped bytes. | Test (TC-202) |
| FR-202-AC-6 | Over every stand-in outcome in this table, no refinement disposition carries execution `refused` unless the stand-in returned a refusal of the item; a stand-in `malformed-declaration` refusal is carried unchanged. | Test (TC-202) |
| FR-202-AC-7 | Given a stand-in result for unsupported dynamic behaviour or a missing correspondence, no disposition presents `satisfied`, and the cause is carried unchanged. | Test (TC-202) |

## Dependencies

- [FR-301](./FR-301-admit-protocol-refinement-dispatch.md) supplies FR-301-AC-6, whose carry-unchanged semantics this requirement extends to the paired counterexample's halves.
- [FR-030](./FR-030-bind-each-disposition-to-its-exact-premises.md) supplies the byte-equality rule for producer-authored components (FR-030-AC-6).
- [FR-008](./FR-008-return-typed-refusals.md) supplies the unknown-source rule for a cause the library does not recognise.
- `ix://agent-ix/filament-specification/FR-241`, `ix://agent-ix/filament-specification/FR-242` and `ix://agent-ix/filament-specification/FR-243` own the axis vocabularies; `ix://agent-ix/filament-specification/FR-177` and FSL FR-148 own the results carried.
