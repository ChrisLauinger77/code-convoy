## Change

Describe the problem and resulting behavior. Link related issues if applicable.

## Validation

Record commands/results, tested OS and build revision. Mark unperformed checks
explicitly; distinguish fixtures, authenticated agent runs and exact-package tests.

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --locked --all-targets --all-features -- -D warnings`
- [ ] `cargo test --locked --all-features`
- [ ] `cargo build --locked --release --bin codeconvoy`
- [ ] `python3 -m unittest discover -s packaging -p 'test_*.py'`

For UI/platform changes, include native checks and their limits. Markdown-only
changes skip automatic CI; record local validation instead of assuming a CI pass.

## Risks and scope

Describe compatibility, persistence, repository/worktree safety or dependency
license implications, if any. List remaining limitations and untested platforms.
Confirm documentation matches changed behavior. See
[CONTRIBUTING.md](https://github.com/ChrisLauinger77/code-convoy/blob/main/CONTRIBUTING.md).
