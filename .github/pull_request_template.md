## Summary

<!--
The smallest view that makes the point: pseudocode, a call tree, a file
tree, a Mermaid diagram or a diff sketch, each beside one or two lines of
why. The diff already says what; this says why and its shape.
-->

Closes #

## Evidence

<!--
A before and an after: a failing then passing test, a CLI run's output, the
TUI over `make demo`. `make check` is the gate; show what you looked at
beyond it.
-->

- **Before:**
  **After:**

## Merge Danger

<!--
A two-way door is cheap to walk back; a one-way door is not: a shipped
Cache migration, a change written to the user's server, a config key whose
meaning changes.
-->

**Door:** <!-- one-way or two-way -->

**Blast Radius:** <!-- one word: what could break, and for whom -->

## Before merging

- [ ] `make check` passes locally.
- [ ] Tests cover the new behaviour through the fake servers.
- [ ] One logical change per commit, Conventional Commits, no co-author
      trailer.
- [ ] Docs and ADRs that the change contradicts are updated.
- [ ] This pull request is one thing.
