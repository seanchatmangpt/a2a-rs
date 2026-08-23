# Agent Operating Contract

Scope: repository-wide unless a deeper `AGENTS.md` narrows a subtree. Live tree evidence outranks stale prose.

Before edits, resolve repo/ref/base to an exact commit and read applicable root+nested doctrine, README/architecture, manifests, task runners, CI, generated-source policy, and release policy. Preserve interfaces, authority, receipts/replay, generated/manual boundaries, compatibility, and maximal reversible lawful options. Apply Chesterton's fence before deleting a boundary; one failed edge is topology, not graph failure.

Use `UNKNOWN | PARTIAL_ALIVE | ALIVE | BLOCKED | BUILD_BROKEN | UNSUPPORTED` plus typed `REFUSED_*`. `ALIVE` requires observed execution against the exact admitted subject. Track observed/admitted/executed/changed/verified/inferred/refused/blocked/unsupported separately; inspection is not execution.

`A = μ(O*)`; `R = receipt(A)`. Separate `SELECT`, `CONSTRUCT`, `DO`. Model/planner/generator/proof/hook output has no ambient execution authority; hooks manufacture intents, never actuate. Consequential `DO` uses the repository's admitted receipt-bearing boundary: zero unreceipted actuation.

Follow `parse → orient → resolve → materialize → read doctrine → inspect → admit/refuse → diagnose/repair → construct → actuate → receipt → replay → standing`. Prefer the existing lawful path and smallest coherent diff. Generated artifacts are projections: edit their owning source. Do not fabricate evidence, weaken tests, substitute unit proof for requested integration/e2e proof, or add unrelated refactors.

Acceptance precedence: exact user behavior/command → live documented repo command → narrowest equivalent. On failure preserve command/exit/diagnostic, form a new hypothesis, repair narrowly, and rerun the failed boundary. CI supplements local proof; it is not truth.

Unless explicitly instructed otherwise: purpose branch from exact base, intentional commit, non-force push, draft PR, no merge. Final receipt states repo/base/tree, transports/failures, changes/generated status, commands/exits, verification ladder, receipt/replay, branch/SHA/PR, scoped standing, and falsifiers.