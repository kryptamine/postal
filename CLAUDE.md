@AGENTS.md

## Tooling

- `pr`: write every pull request body with it, before `gh pr create` or
  `gh pr edit --body`. It fills the template AGENTS.md makes mandatory.

## macOS

BSD `sed -i` differs from GNU sed. Use `perl -pi -e 's/old/new/g' file` for
in-place substitution.
