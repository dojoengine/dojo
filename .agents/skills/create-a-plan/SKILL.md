---
name: create-a-plan
description: Conduct a focused technical planning interview to produce an implementable, parallelizable plan or spec with clear dependencies, risks, and open questions.
---

# Create a Plan Skill

This skill runs a structured technical interview to turn a rough idea or an existing spec into a detailed, implementable plan. The output is organized for parallel execution: foundations first, then independent workstreams, then merge and integration.

## Invocation

The user will provide one of:
- A path to a spec or plan file (for example: `SPEC.md`, `PLAN.md`, `RFC.md`)
- A rough description of what they want to build
- A feature request or problem statement

Output is always written to `PLAN.md` in the repo root.

## Process

### Phase 0: Preflight

1. If a file path is provided, read it first and note goals, non-goals, constraints, and gaps.
2. Confirm you will produce `PLAN.md` as the output in the repo root. If `PLAN.md` already exists, update it rather than creating a new file.

### Phase 1: Discovery

Summarize what is known, then identify missing details. Focus on:
- Goals and non-goals
- Constraints (time, budget, platform, dependencies)
- Success metrics and acceptance criteria

### Phase 2: Deep Interview

Use the `AskUserQuestion` (Claude) and/or `request_user_input` (Codex) tools in rounds. Ask 1-3 questions per round. Each round should go deeper and avoid repeating what is already known.

CRITICAL RULES:
1. Never ask obvious questions. If the codebase or spec already answers it, do not ask it again.
2. Ask about edge cases and failure modes.
3. Probe for hidden complexity (state transitions, migrations, concurrency).
4. Challenge assumptions when they create risk or ambiguity.
5. Identify parallelization boundaries and serial dependencies.
6. If the user is unsure, propose a default and ask for confirmation.

Question categories to cover as relevant:
- Technical architecture and data flow
- Data model and state management
- API contracts and versioning
- Caching and invalidation
- Background jobs, retries, and idempotency
- Error handling and recovery
- Observability and debugging
- Performance, scale, and SLAs
- Security, privacy, and compliance
- Integrations and external dependencies
- UX flows, accessibility, and responsiveness
- Rollout, migration, and rollback
- Testing strategy and validation

### Phase 3: Dependency Analysis

Identify:
1. Serial dependencies that must complete first
2. Parallel workstreams that can run independently
3. Merge points where work reconvenes

### Phase 4: Plan Generation

Write the final plan to `PLAN.md`. Ensure the plan includes concrete verification steps the agent can run end to end. If the user only wants a plan in chat, provide it inline and mention that it would be written to `PLAN.md`.

## Output Format

The generated plan MUST follow this structure:

```markdown
# [Feature Name] Implementation Plan

## Overview
[2-3 sentence summary of what this implements and why]

## Goals
- [Explicit goal 1]
- [Explicit goal 2]

## Non-Goals
- [What this explicitly does NOT do]

## Assumptions and Constraints
- [Known constraints or assumptions]

## Requirements

### Functional
- [Requirement]

### Non-Functional
- [Performance, reliability, security, compliance]

## Technical Design

### Data Model
[Schema changes, new entities, relationships]

### API Design
[New endpoints, request/response shapes, versioning]

### Architecture
[System diagram in text or mermaid, component interactions]

### UX Flow (if applicable)
[Key screens, loading states, error recovery]

---

## Implementation Plan

### Serial Dependencies (Must Complete First)

These tasks create foundations that other work depends on. Complete in order.

#### Phase 0: [Foundation Name]
**Prerequisite for:** All subsequent phases

| Task | Description | Output |
|------|-------------|--------|
| 0.1 | [Task description] | [Concrete deliverable] |
| 0.2 | [Task description] | [Concrete deliverable] |

---

### Parallel Workstreams

These workstreams can be executed independently after Phase 0.

#### Workstream A: [Name]
**Dependencies:** Phase 0
**Can parallelize with:** Workstreams B, C

| Task | Description | Output |
|------|-------------|--------|
| A.1 | [Task description] | [Concrete deliverable] |
| A.2 | [Task description] | [Concrete deliverable] |

#### Workstream B: [Name]
**Dependencies:** Phase 0
**Can parallelize with:** Workstreams A, C

| Task | Description | Output |
|------|-------------|--------|
| B.1 | [Task description] | [Concrete deliverable] |

---

### Merge Phase

After parallel workstreams complete, these tasks integrate the work.

#### Phase N: Integration
**Dependencies:** Workstreams A, B, C

| Task | Description | Output |
|------|-------------|--------|
| N.1 | [Integration task] | [Concrete deliverable] |

---

## Testing and Validation

- [Unit, integration, end-to-end coverage]
- [Manual test plan if needed]

## Rollout and Migration

- [Feature flags, staged rollout, migration steps]
- [Rollback plan]

## Verification Checklist

- [Exact commands or manual steps the agent can run to verify correctness]
- [Expected outputs or success criteria]

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| [Risk description] | Low/Med/High | Low/Med/High | [Strategy] |

## Open Questions

- [ ] [Question that still needs resolution]

## Decision Log

| Decision | Rationale | Alternatives Considered |
|----------|-----------|------------------------|
| [Decision made] | [Why] | [What else was considered] |
```

## Interview Flow Example

Round 1: High-Level Architecture
- "The spec mentions a sync engine. Is this push-based (webhooks), pull-based (polling), or event-driven (queue)?"
- "What is the expected data volume and throughput?"

Round 2: Edge Cases
- "If a batch fails mid-run, do we retry the whole batch or resume from a checkpoint?"
- "What happens when source data is deleted but still referenced downstream?"

Round 3: Parallelization
- "Can we process different categories independently, or are there cross-category dependencies?"
- "Is there a natural partition key that allows sharding?"

Round 4: Operational
- "What is the acceptable latency for sync or processing?"
- "How will operators debug failures and what visibility do they need?"

## Key Behaviors

1. Persist until the plan is implementable and verifiable by the agent, but avoid user fatigue by batching questions.
2. Challenge vague answers when they affect design decisions.
3. Identify hidden work and operational overhead.
4. Think about the merge and integration steps early.
5. Summarize understanding and confirm before writing the final plan.

## Completing the Interview

After sufficient rounds of questions:
1. Summarize your understanding back to the user
2. Confirm the parallelization strategy
3. Write the complete plan to the target file
4. Ask if any sections need refinement
