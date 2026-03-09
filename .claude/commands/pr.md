Create a PR for the current branch following the rira project conventions.

## Steps

1. Run `git status` and `git diff main...HEAD` to understand all changes
2. Determine appropriate labels from this list (create if missing):
   - Phase labels: `phase-1` ~ `phase-11`
   - Crate labels: `crate:app`, `crate:editor`, `crate:keymap`, `crate:theme`, `crate:renderer`, `crate:ui`, `crate:highlight`, `crate:git`, `crate:terminal-core`, `crate:terminal-pty`
   - Type labels: `feature`, `bugfix`, `refactor`, `docs`, `ci`, `tests`, `infra`
   - Priority labels: `core`, `nice-to-have`
3. Push the current branch to origin with `-u` flag
4. Create the PR with `gh pr create` using this format:

```
gh pr create --title "<concise title>" --label "<labels>" --assignee "ohah" --body "$(cat <<'EOF'
## Summary
<1-3 bullet points describing what changed and why>

## Changes
<detailed list of changes per crate/file>

## Test Plan
- [ ] `cargo test --workspace --exclude rira-app --exclude rira-renderer` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] <specific test instructions for this PR>

## How to Verify
```bash
<commands to build/run/test the changes>
```

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

5. If any labels don't exist yet, create them first with `gh label create`
6. Return the PR URL
