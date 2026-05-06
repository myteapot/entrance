# Entrance Remote Truth Source

> This document records the corrected truth-source model after recovery discussion.
> It supersedes the weaker idea that the standalone local recovery DB should remain the long-term truth source.

## 1. Canonical Truth Location

The final truth source should live inside Entrance, not in the standalone local recovery DB.

That means:

- `.agents` and its recovered memory substrate are a recovery carrier
- Entrance is the long-term canonical system
- the recovery carrier should eventually be copied into Entrance, not treated as the final destination

## 2. Migration Rule

Migration from `.agents` into Entrance must be copy-first and non-destructive.

Practical rule:

- copy files and memory artifacts into Entrance
- verify the merged result
- keep the original recovery substrate intact until the new Entrance-backed truth is confirmed

No raw deletion should occur during this merge path.

## 3. Protected Remote Model

The canonical remote should use a protected default branch.

Desired properties:

- branch protection enabled
- force-push disabled
- deletion disabled
- only merge and normal commits allowed under policy
- bot account used for automation rather than an admin account

The bot account should not have the authority to bypass branch protection or delete the repository.

## 4. Snapshot Branch

In addition to the protected default branch, a separate protected snapshot branch can hold backup-oriented snapshots of the memory substrate and related recovery artifacts.

This does not replace the canonical branch. It adds a second auditable layer inside the same remote.

## 5. Offsite Snapshots

Later, the remote itself should be periodically snapshotted and uploaded to independent storage, ideally as encrypted archives.

This is a later hardening layer, not the immediate first step.

## 6. Practical Hierarchy

The corrected hierarchy is:

1. Entrance repository as canonical truth source
2. protected default branch as authoritative history
3. protected snapshot branch as in-repo backup lane
4. encrypted offsite snapshots as disaster fallback
5. `.agents` recovery substrate as transitional source and migration staging area

## 7. Key Boundary

The standalone local `store.db` should not be described as the final truth source.

It remains valuable during recovery, but its role is transitional:

- recovery substrate
- staging area
- import/export bridge
- source for copy-first migration into Entrance
