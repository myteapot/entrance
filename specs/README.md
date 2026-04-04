# Specs Structure

> Status: hot index
> Rule: `top` keeps only confirmed content, references, and `TODO(fill)` markers.

## Structure

- `top/`
  active top-level design surface
- `cold/`
  repo-side cold staging for mounted subordinate docs
- `chore/`
  execution, migration, handout, and todo docs that should not occupy the hot top surface

## Top Documents

- [1.1 OS Core](top/1.1-os-core.md)
- [1.2 Hierarchical State Machine](top/1.2-hierarchical-state-machine.md)
- [1.3 Compiler And Action IR](top/1.3-compiler-action-ir.md)
- [2.1 OTP Supervisor Model](top/2.1-otp-supervisor-model.md)
- [2.2 Lead Model 3](top/2.2-lead-model-3.md)
- [2.3 Control Tree Node LTE 3](top/2.3-control-tree-node-lte-3.md)
- [3.1 Learning And Truth System](top/3.1-learning-and-truth-system.md)

## Notes

- The current `cold/` tree is a repo-side cold staging layer.
- Superseded or conflicting state-machine drafts should be recorded in runtime DB through `entrance hygiene spec-v0` with explicit relations before hot-side cleanup deletes context.
