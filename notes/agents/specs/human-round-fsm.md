# Human Round State Machine

> Scope: define the canonical lifecycle for one human interaction round

## Canonical states

- `opened`
  a round exists but is not yet checkpointed
- `checkpointed`
  continuity is anchored by a current checkpoint
- `accepted`
  the round has formally passed and acceptance is recorded
- `settling`
  accepted truth exists, but next-step, projection, or repair debt remains
- `fully_settled`
  acceptance is current and no follow-on debt remains
- `superseded`
  a later round now carries continuation

## Detail states

- `uncheckpointed`
  detail projection of canonical `opened`
- `checkpointed_pending_acceptance`
  detail projection of canonical `checkpointed`
- `accepted_waiting_carry_forward`
  detail projection of canonical `accepted`
- `accepted_followup_open`
  detail projection of canonical `settling`
- `fully_settled`
  detail projection of canonical `fully_settled`

## Canonical transitions

- `opened -> checkpointed`
- `checkpointed -> accepted`
- `accepted -> settling`
- `accepted -> fully_settled`
- `settling -> fully_settled`
- `fully_settled -> superseded`

## Illegal transitions

- `opened -> fully_settled` without checkpoint and acceptance
- `checkpointed -> fully_settled` without acceptance
- `superseded -> current`
- any transition that leaves multiple current rounds for one runtime scope

## Derived obligations

- a current round should have at most one current checkpoint
- a current accepted round should have at most one current acceptance bundle
- a fully settled round should have no current `next_step`
- a fully settled round should not leave dirty required projections behind
- repair may reopen settling work, but it should not erase prior acceptance truth
- handout and wake-request bridge objects should mirror both canonical and detail state for the current round
