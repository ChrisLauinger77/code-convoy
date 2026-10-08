# Part 3.2 completion and validation

Implemented against merged Part 3.1 (`94fbd3c`), on branch
`codex/review-apply-discard`. No dependency or application version change.

## Completion report

1. **Review UI/model:** compact convoy grid includes every launched job, selection,
   execution status, mode, availability/resolution and totals, in the existing two panes.
2. **Statistics:** Git numstat of effective working files versus the saved isolated
   base, with nonignored untracked files included using a disposable index. Binary
   files have no fabricated line counts; rename is two changed paths (delete/add).
   Divergent staged versions have a separate alternative count and block Apply.
   Fresh index entries exclude cached timestamps so same-size edits cannot be missed.
3. **Direct versus Isolated:** Direct is explicitly the current mutable working tree;
   only isolated results have Apply/Discard. Cached observations are refreshed off-thread.
4. **Apply preconditions:** terminal result, current registration, matching canonical
   source/common Git identity, original base present, full Part 3.1 ownership verification,
   clean destination at that base, representable changes and exclusive repository lease.
   A changed result tree must be reviewed again.
5. **Mechanism:** full-index binary Git patch, preflight, repeated checks, working-tree-only
   Git apply and post-verification. Neither real index is edited. Temporary index writes
   disable index hooks; unreachable Git objects can be created, but no commits or refs.
6. **Supported changes:** tracked modification/addition/deletion, rename as delete/add,
   binary data, nonignored new files, tracked ignored additions, executable changes and
   symlinks where Git/platform settings can preserve them.
7. **Blocked cases:** submodules/nested repositories, sparse/unmerged/assume-unchanged
   entries, divergent staged alternatives, conversion attributes (filter/encoding/text/
   eol/ident), autocrlf conversion, unsupported mode/symlink settings and oversized output.
   Custom-filter statistics are unavailable; ordinary pre-existing Git inspection retains
   Part 3.1's Git configuration semantics. No unsupported transfer is silently omitted.
8. **Preflight:** `git apply --check`, then source-image, ownership and destination
   revalidation under the same lease. A preflight refusal leaves the destination alone.
9. **Apply failure:** every error after a write attempt is uncertain; no success claim,
   automatic retry or reset/clean/stash recovery. The isolated result survives.
10. **Applied lifecycle:** separate persisted Applied resolution and timestamp. Stable
    isolated Diff remains until explicit confirmed retained-copy cleanup. Applied is not
    classified as unresolved solely because its copy exists.
11. **Discard:** safe-default confirmation, Escape cancellation, exact verified removal,
    Discarded history and explicit unavailable Diff. No source-working-tree modifications.
12. **Cleanup integration:** same Store intent/outcome transaction, manager lease, ownership
    verifier and Git removal/journal as Part 3.1; no new deletion implementation.
13. **Locks:** Apply/Discard exclude Direct and isolated jobs, maintenance and other actions
    for the same common Git repository or nested source. Other repositories continue.
    No agent slot is consumed; busy operations fail with retry guidance. Peer copies remain.
14. **History/restart:** additive defaults load old state. Applied/Discarded survive restart;
    unresolved/uncertain results and copies awaiting cleanup retain ownership/history
    protection. Saved discard intent plus completed cleanup recovers as Discarded. Interrupted
    Apply remains uncertain and never replays. History removal remains metadata-only.
15. **Activity/UI:** application lifecycle messages use `[CodeConvoy]`; Raw is untouched.
    Existing Activity/Diff/Raw/Task tabs, keyboard buttons, visible focus and themes remain.
    Quit waits for an active result operation's final state merge.
16. **Validation:** required formatting, all-target/all-feature clippy, all-feature tests,
    release build and packaging tests pass locally. See details below.
17. **Native scenarios:** A–D passed with disposable native macOS fixtures, including
    24-row Review, real successful/failed fixture processes, keyboard Discard and restart.
18. **Cross-platform:** existing Linux/macOS/Windows CI matrix is retained; platform mode
    support is explicitly gated. Unix-only symlink/permission tests are conditional.
19. **Limitations:** external tools are outside CodeConvoy leases. Filesystem writes are not
    crash-atomic; an OS failure can require manual destination inspection. A crash between
    Git success and persisted completion remains ApplyPending. Patches are capped at 32 MiB,
    rendered diffs at 2 MiB, and other Git inspection limits/timeouts still apply. No stored
    archival diff after cleanup; statistics unavailable after Discard. Non-Unicode temporary
    index paths are explicitly unsupported.
20. **Deferred to Part 3.3:** Retry, New Convoy from selected, and follow-up provenance.
    No automatic commits/staging/push/branches/PRs, merge/rebase, conflict resolver,
    pipelines, new backend or storage dashboard was added to the application.

## Checks

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`: 252 top-level tests pass; four existing optional
  installed/authenticated CLI probes remain ignored. Subprocess self-tests also pass.
- `cargo build --release`
- `python3 -m unittest discover -s packaging -p 'test_*.py'`: 16 pass.

The worktree/recovery suite now has 54 tests (38 Part 3.1 tests preserved plus
16 Part 3.2 scenarios, several parameterized). Coverage includes Git counts and
fixed-base independence; all transfer categories; failed/cancelled/interrupted
results; dirty/moved/missing/unregistered destinations; changed result/ownership;
ignored-file collision preflight with no partial write; actual Unix write refusal;
uncertain completion persistence; active/peer exclusion; index-hook suppression;
same-size cached-stat result/destination edits; mode capability refusal;
Discard retries, source/peer preservation and restart.
UI tests add 24-job rendering without Git work, safe default Enter/Escape handling,
result-operation quit handling and truthful diagnostic classification. Legacy
version-1 fixture assertions explicitly cover additive resolution defaults.
Existing scheduler tests verify exclusive maintenance, admission blocking,
unrelated progress, release and quarantine. No new backend protocol was introduced.

## Native macOS scenarios

Helper: `cargo build --features test-support --example v03_review_validation
--bin codeconvoy-test-agent`, then `v03_review_validation --prepare <new-directory>`.
Opening the helper without arguments uses the persisted disposable workspace.
It runs 24 successful isolated fixture jobs plus one genuinely failing fixture
(exit 7 after editing), through the real runner, before opening native egui.
No installed coding agent, authentication or ordinary application state is used.

Workspace: `/private/tmp/codeconvoy-part32-native-20261007`.

- **A — Apply:** Review showed 24/24 measured results, +24/−24. Selected first job,
  opened its fixed-base Diff (`native committed base` → `review`), explicitly Applied.
  Source received exactly `review`; HEAD still equaled the saved base and the real
  index remained clean. The copy stayed present and history showed Applied.
- **B — destination changed:** second source contained `user destination edit`.
  Apply remained Unresolved and displayed “Apply blocked: Registered working tree
  has local changes.” Original destination bytes and isolated result were preserved.
- **C — Discard:** third job's modal visibly focused Cancel; Escape dismissed it.
  Reopened, used Tab and Return to confirm. Only its retained copy disappeared;
  source stayed `native committed base`, history said Discarded, and Diff explicitly
  reported that the result was discarded. Peer isolated copies stayed intact.
- **D — failed agent:** convoy #2 showed Failed + one changed file (+1/−1), with Apply
  enabled. Explicit Apply copied `failed` into its clean source while retaining Failed
  agent status, unchanged HEAD/index and Applied resolution.
- **Restart:** quit/reopened and verified Applied and Discarded independently of
  physical availability. Rechecked corrected blocked and cleanup messages. The
  validation window was quit afterward; disposable resources remain for inspection.

A separate filesystem assertion checked source contents, unchanged HEADs, clean
indexes, expected retained/removal paths and the peer result. No commits, pushes,
ordinary repositories, attachment contents or provider credentials were touched by
these native result actions. Linux/Windows native UI was not exercised locally;
authenticated provider runs were not performed.
