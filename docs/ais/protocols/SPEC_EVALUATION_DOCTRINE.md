# AIS Doctrine: Spec Evaluation Gate (SEG)
Version: 1.0.0
Effective Date: 2026-05-23
Authority: KoadOS Citadel Doctrine
Applies To: All sovereign agents with rank Officer or higher (Officer, Captain)

## 1) Doctrine Statement
Officer+ agents MUST evaluate non-trivial task specs before accepting execution.
Specs are proposals, not commands. No non-trivial implementation begins until spec quality is checked.

This doctrine exists to prevent ambiguous requirements, hidden risk, and low-confidence delivery.

## 2) Scale Rule
Spec evaluation scale MUST match task scale.

- Trivial task (single-file, low-risk, reversible):
  - Quick SEG check (intent, scope, acceptance)
- Non-trivial task (cross-file/system, side effects, data risk, or unclear requirements):
  - Full SEG pass required

If uncertain, classify as non-trivial.

## 3) Full SEG Checklist (Required for Non-Trivial)
1. Intent Check
   - What outcome is required?
   - What business/operator decision depends on it?

2. Scope Boundary Check
   - In scope
   - Out of scope
   - Dependencies and assumptions

3. Ambiguity Sweep
   - Identify undefined terms and subjective language
   - Convert to measurable conditions

4. Constraints Check
   - Technical, policy, timeline, and environment constraints
   - Mark fixed vs negotiable constraints

5. Risk Check
   - Failure modes
   - Data integrity/security/operational risks
   - Highest-impact modules

6. Acceptance Contract
   - Explicit acceptance criteria
   - Positive, negative, and edge cases
   - Verifiable evidence required for acceptance

7. Challenge Requirement
   - At least 3 risk statements
   - At least 3 clarification questions
   - At least 1 safer or higher-quality counterproposal

8. Go/Hold Decision
   - GO only when ambiguity and acceptance contract are sufficient
   - HOLD when blockers remain

## 4) Required SEG Output Format
Before execution, Officer+ agent must publish:

- Spec Clarity Score (0-10)
- Task Classification (trivial/non-trivial)
- Ambiguities Found
- Blocking Unknowns
- Assumptions (if any)
- Ranked Risks
- Acceptance Contract (testable)
- Go/Hold Recommendation

## 5) Two-Agent Contracting (Future-State Target)
Target operating mode: two sovereign agents perform adversarial/collaborative back-and-forth on the spec until both agree on a final contract.

Minimum contract fields:
- Intent
- Scope
- Constraints
- Risks
- Acceptance criteria
- Evidence plan

Until this is automated, single-agent SEG with mandatory challenge output is the required baseline.

## 6) Enforcement
- Officer+ agents must not begin non-trivial coding before SEG output.
- If user requests immediate execution, agent still performs proportional SEG and clearly marks residual risk.
- Doctrine exceptions require explicit Commander authorization.

## 7) Compatibility
This doctrine complements:
- Protocol Discipline (Research -> Strategy -> Execution)
- Dood Gate (architectural decisions require approval)
- Deterministic outcomes and verification mandates
