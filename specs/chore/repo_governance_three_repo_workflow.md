# Three-Repo Workflow

> Status: execution staging

## Purpose

- lock the current three-repo governance consensus into a repo-side hot document
- keep day-to-day V1 work out of `pub`
- make review, promotion, and archive sync readable at the MR boundary rather than through commit archaeology

## Repo Roles

### `entrance-private`

- the only active day-to-day development truth source
- all ongoing V1 work starts here
- internal iteration, bounded review branches, and active repo-governance updates land here first

### `entrance-archive`

- stage archive and historical continuity lane
- keeps synchronized accepted baselines rather than becoming the live development trunk
- should be updated after meaningful accepted rounds or phase boundaries, not for every scratch move

### `entrance-pub`

- curated public mirror
- should change only when a human intentionally promotes a public-facing version or larger product milestone
- is not a day-to-day development repo and should not be dirtied by routine internal iteration

## Review Policy

- humans review Merge Requests, not raw commit streams
- `private` may contain implementation-oriented commits as long as the MR slice stays readable
- `pub` commits should stay close to release or promotion semantics
- `archive` MRs should explain carry-forward, sync, or preservation meaning

## Promotion Cadence

### Daily work

- happens only in `entrance-private`
- the default assumption is that `pub` does not move

### Accepted round or stage sync

- may be carried into `entrance-archive`
- archive sync is about preserving accepted historical state, not mirroring every local fluctuation

### Public release or milestone promotion

- may be promoted from `private` into `pub`
- after `pub` is prepared and reviewed, GitHub release publication may happen from that public lane

## Remote Policy

- GitLab on `9311` is the working review surface
- `private` points at the private GitLab repo
- `archive` points at the archive GitLab repo
- `pub` uses GitLab as the working `origin` and may keep a separate GitHub remote for release publication

## CI And Pipeline Policy

- default GitLab Auto DevOps is disabled for this workflow
- CI should not be assumed by default just because a repo exists
- if CI is introduced later, it should be explicit, minimal, and intentional rather than inherited from GitLab defaults

## Immediate Operating Rule

- after this governance cut, all active V1 work should proceed from `entrance-private`
- `entrance-pub` stays dormant until the next intentional public promotion
- `entrance-archive` stays aligned to accepted phase truth rather than active day-to-day churn
