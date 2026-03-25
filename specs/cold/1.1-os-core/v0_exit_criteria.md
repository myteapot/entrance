# V0 Exit Criteria

> Scope: define when V0 post-consolidation sharpening is complete enough to stop top-level drift

## Exit gates

- `human_round` exists as live runtime truth rather than prose only
- `acceptance` and `fully_settled` both have strict machine predicates
- hot root remains bounded to six files and is rebuildable from DB
- cold docs are canonicalized in DB and may be exported to files
- anti-Zeno is both visible and enforceable
- projection freshness and dirty repair are visible runtime truth
- invariant checking exists and can surface repairable violations
- host and worktree ownership are explicit runtime truth
- recovery is permanently lowered to import-only status
- the top-level constitutional docs are frozen and no longer ambiguous

## Non-exit signals

- more product ideas exist
- more UI could be built
- more integrations could be added
- more role-local convenience could be invented

Those may all be true while V0 sharpening is still complete.
