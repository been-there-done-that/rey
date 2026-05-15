# Rey — Parallel Agent Execution Guide

## Overview

This guide explains how to distribute Rey implementation tasks across multiple agents running in separate git worktrees. Each wave contains tasks that can run in parallel once their dependencies are satisfied.

## Dependency Graph

```
Wave 0: [Task 1 — Scaffold workspace]
              │
              ▼
Wave 1: [Task 2 — types]    [Task 3 — common]
              │                      │
              └──────────┬───────────┘
                         ▼
Wave 2: [Task 5 — crypto]    [Task 6 — image]
              │                      │
              ▼                      ▼
         [Task 7 — metadata]    [Task 8 — thumbnail]
              │                      │
              └──────────┬───────────┘
                         ▼
Wave 3: [Task 10 — local-db]  [Task 11 — zoo]  [Task 12 — zoo-client]  [Task 13 — sync]
              │                      │                    │                    │
              └──────────────────────┴────────────────────┴────────────────────┘
                                      ▼
Wave 4: [Task 15 — client-lib]    [Task 16 — zoo-wasm]
              │                              │
              └──────────────┬───────────────┘
                             ▼
Wave 5: [Task 18 — desktop]    [Task 19 — web]
```

## Parallelism Rules

1. **Wave 0 must complete first.** All other waves depend on the workspace scaffold.
2. **Within a wave, tasks can run in parallel** (different worktrees, different agents).
3. **A wave cannot start until ALL tasks in the prior wave are complete** and merged to master.
4. **Exceptions:** Within Wave 2, Task 7 (metadata) depends on Task 5 (crypto), and Task 8 (thumbnail) depends on Tasks 5, 6, and 7. So within Wave 2, the actual order is: Task 5 + Task 6 first (parallel), then Task 7, then Task 8.
5. **Within Wave 3**, Task 13 (sync) depends on Tasks 10, 5, 7, 8. Tasks 10, 11, 12 can start in parallel. Task 13 waits for 10 + all of Wave 2.

## Worktree Setup

After Wave 0 is merged to master, create worktrees for parallel waves:

```bash
# From the main repo directory
git worktree add ../rey-wave1-types -b wave1-types master
git worktree add ../rey-wave1-common -b wave1-common master

# After Wave 1 is merged
git worktree add ../rey-wave2-crypto -b wave2-crypto master
git worktree add ../rey-wave2-image -b wave2-image master

# After Wave 2 is merged
git worktree add ../rey-wave3-localdb -b wave3-localdb master
git worktree add ../rey-wave3-zoo -b wave3-zoo master
git worktree add ../rey-wave3-zooclient -b wave3-zooclient master

# After Wave 3 is merged
git worktree add ../rey-wave4-clientlib -b wave4-clientlib master
git worktree add ../rey-wave4-zoowasm -b wave4-zoowasm master

# After Wave 4 is merged
git worktree add ../rey-wave5-desktop -b wave5-desktop master
git worktree add ../rey-wave5-web -b wave5-web master
```

## Agent Assignment Per Wave

### Wave 0 (Sequential — 1 agent)
- **Agent A**: Task 1 (Scaffold workspace)

### Wave 1 (2 agents in parallel)
- **Agent A**: Task 2 (types crate) in `../rey-wave1-types`
- **Agent B**: Task 3 (common crate) in `../rey-wave1-common`

### Wave 2 (4 agents, but with internal dependencies)
- **Agent A**: Task 5 (crypto crate) in `../rey-wave2-crypto` — starts first
- **Agent B**: Task 6 (image crate) in `../rey-wave2-image` — starts first (parallel with Task 5)
- **Agent C**: Task 7 (metadata crate) — starts after Task 5 is done
- **Agent D**: Task 8 (thumbnail crate) — starts after Tasks 5, 6, 7 are done

### Wave 3 (4 agents in parallel)
- **Agent A**: Task 10 (local-db crate) in `../rey-wave3-localdb`
- **Agent B**: Task 11 (zoo server) in `../rey-wave3-zoo`
- **Agent C**: Task 12 (zoo-client) in `../rey-wave3-zooclient`
- **Agent D**: Task 13 (sync engine) — starts after Task 10 + all Wave 2 complete

### Wave 4 (2 agents in parallel)
- **Agent A**: Task 15 (client-lib) in `../rey-wave4-clientlib`
- **Agent B**: Task 16 (zoo-wasm) in `../rey-wave4-zoowasm`

### Wave 5 (2 agents in parallel)
- **Agent A**: Task 18 (desktop app) in `../rey-wave5-desktop`
- **Agent B**: Task 19 (web app) in `../rey-wave5-web`

## Merge Process

After each wave completes:
1. Each agent commits to their worktree branch
2. Review and merge each branch to master
3. Resolve any conflicts (should be minimal since tasks touch different crates)
4. Run `cargo test --workspace --all-features` to verify
5. Delete completed worktrees: `git worktree remove ../rey-waveX-xxx`
6. Create new worktrees for the next wave

## Verification Checklist Per Wave

- [ ] All tasks in the wave are complete
- [ ] `cargo check --workspace` passes
- [ ] `cargo test -p <crate>` passes for each modified crate
- [ ] `cargo clippy --workspace -- -D warnings` is clean
- [ ] `cargo fmt --check` passes
- [ ] Branches are merged to master
- [ ] Worktrees are cleaned up

## Task File Format

Each task file in `tasks/wave-X-*/TASK-XX.md` contains:
- **Task ID and name**
- **Dependencies** (what must be done first)
- **Design references** (which sections of design.md to follow)
- **Files to create/modify** (exact paths)
- **Implementation details** (step-by-step instructions)
- **Test requirements** (what tests to write)
- **Verification steps** (how to confirm completion)
- **Can run in parallel with** (which other tasks)

## Notes

- Checkpoints (Tasks 4, 9, 14, 17, 20) are merge gates — do not proceed to the next wave until the checkpoint passes.
- Tasks marked with `*` are optional (tests) and can be skipped for a faster MVP.
- The `zoo` crate (Task 11) is the largest single task — consider splitting it across multiple agents if needed.
- All code conventions: no comments unless asked, follow existing patterns, use `thiserror` for error types, `serde` for serialization.
