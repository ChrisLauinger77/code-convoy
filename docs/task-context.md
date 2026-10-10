# Groups, templates and attachments

These features prepare an ordinary convoy in **NEW CONVOY**. A launched run
still contains one immutable task and one job per explicitly selected repository.

## Repository groups

Use **Manage groups…** to create, rename, edit membership or delete a group.
A repository can belong to several groups. Groups refer to registered canonical
repository paths; they do not contain copies of repositories or Git settings.

Selecting a group adds its currently available members to the explicit selection.
Repeated or overlapping selections produce only one selected path. Individual
checkboxes can then add or remove paths. Group checkboxes are derived from that
selection, without a separate remembered group state. A partial group is shown
as indeterminate; selecting it adds its remaining available members.

**Deselecting a group removes all its current members from the explicit
selection**, including paths selected individually or through an overlapping
group. Other paths remain selected. Editing or deleting a group does not change
current selections, registrations, repository contents or run snapshots.

Unavailable/unregistered members are indicated and retained. Refresh repository
state after restoring a folder, register its original canonical path again, or
remove/replace its membership in Manage groups. Group selection skips members
known to be unavailable; normal Git preflight remains mandatory for every
selected repository. It catches failures even if the cached state was stale.

### Bulk validation configuration

**Assign Validation Preset…** opens a separate repository/group selection. Select
one or several groups and refine individual targets; overlapping canonical paths
are updated once. This selection never changes NEW CONVOY or memberships. Unlike
convoy admission, configuration does not need Git or an available folder.
Unregistered members are reported explicitly. Existing commands are preserved by
default; replacing them requires the overwrite option and a confirmation showing
the count. Each repository owns its saved command independently, with no group
inheritance. See [validation configuration](validation.md#assign-to-several-repositories).

## Task templates

The **Templates** menu saves current task text, loads a template, or opens its
editor for changes, renaming and deletion. Templates contain only a name and task
text. Loading changes only the draft text and focuses it for editing. It keeps
the backend, settings, attachments and repository selections, never launches
work, and never changes a historical task.

## Attachments

Use **Add files…** for the native multi-file picker, or drop local files onto
the attachment area. Remove detaches a reference; it does not delete the file.
Supported extensions are `.md`, `.txt`, `.json`, `.yaml`, `.yml`, `.png`, `.jpg`,
`.jpeg` and `.webp`, case-insensitively. Text must be UTF-8 without NUL bytes;
images must have a matching file signature. Full image decoding and model
compatibility remain the selected CLI's responsibility.

CodeConvoy permits 32 files, at most 10 MiB each and 16 MiB combined. These are
product memory/payload guards for context repeated across repository jobs, not
universal model limits. Backend/provider limits can be lower. Large text may
also exceed a model's context window. Claude additionally rejects an image
whose base64 representation exceeds 10 MB; partner-hosted limits can be lower.

Each attachment records its canonical native path, selected display filename,
type, byte size and SHA-256 digest. Spaces, Unicode and shell metacharacters are
passed as data. Paths must be representable as Unicode for JSON history; an
unrepresentable path produces an explicit error rather than lossy transport.
Aliases to the same canonical file are deduplicated.

File inspection runs off the UI thread. Files are checked when added, during
preflight, and after scheduling admission immediately before command preparation.
Missing, unreadable, oversized or changed files prevent execution. A size and
digest match protects against same-size changes. Restore the original file, or
remove and re-add it to accept updated context. Native image flags refer to
files that the CLI subsequently reads, so an external edit after the last check
can still race with that read; CodeConvoy does not lock arbitrary documents.

Every selected job receives the same task references. The backend owns transport:

| Backend | Text/context files | Images |
| --- | --- | --- |
| Codex | JSON-escaped filename/content blocks in stdin prompt | Repeated native `--image PATH` arguments |
| Copilot | Same explicit text context through stdin | Repeated native `--attachment=PATH` arguments |
| OpenCode | Same explicit text context through stdin | Repeated native `--file=PATH` arguments |
| Claude Code | Text stdin, or a text block when images are present | Base64 image blocks in a documented `stream-json` stdin user message |

Text inclusion avoids requiring tool access to external documents. Image models
must support vision. Preflight checks image flag availability for Codex, Copilot
and OpenCode; a missing interface reports the affected files and blocks launch.
Codex treats commas in image paths as separators, so those image references are
rejected with a rename/move explanation. Claude documents some flags that do not
appear in help; its core compatibility check and runtime errors remain explicit.
Files are never silently ignored or copied into repositories, and adding an
attachment never adds filesystem directories or broader approvals to a command.

Launch snapshots save references and metadata, not file contents. **Task &
settings** can show those references even after files disappear. Successful
launch clears draft attachments along with task text and repository selection.
**Reuse convoy** restores all original references and marks missing, unreadable
or changed ones in the draft. They are never silently dropped. Run remains
disabled during asynchronous validation and while any marked reference remains.
Restore the file, then Remove and re-add it; or explicitly Remove its reference
to run without that context. Adding another file does not clear this requirement.
Reuse always requires a new review. Templates do not store attachments.

Attachment contents are transient CLI input; CodeConvoy does not persist them,
copy them into application data, or echo them in its diagnostics. Paths and
digests are saved metadata. Agent responses may themselves quote supplied
context, and Raw output shows what the CLI emits. Each CLI's provider, logging
and data policy still apply: local orchestration does not make the provider local.

## Activity and raw output

Results offer **Activity | Diff | Raw output | Task & settings**. Codex,
OpenCode and Claude summarize their actual structured events, tool names, file
paths, commands, responses and reported usage. Unknown event details stay in
Raw output. Copilot's actual plain text is used for both views. No synthetic
reading/editing stages or estimated provider progress are generated.

Both views retain the latest 512 KiB per job/view and share a 32 MiB session
budget. Oldest job logs are evicted first. Bounded channels report dropped
display output; backend completion checks continue independently. Raw stdout
keeps original text/JSON, stderr chunks are labeled, and invalid UTF-8 is replaced
for display. Ordering reflects chunk arrival across the two streams. Both views
use visible-line layout and are never restored from saved history.

## CLI evidence and validation limits

Research date: **2026-10-06**. Only Codex was installed on this development Mac
(`0.160.1`); its public exec/resume help was inspected without a provider request.
The other interfaces are checked against authoritative documentation/source and
deterministic fixture processes. Prior Codex/Copilot authenticated E2E evidence
does not establish attachment E2E for these new transports. Authenticated
attachment tests for all four remain outstanding. Part 2.3 adds a deliberate,
ignored Codex provider probe and verifies text/image receipt with fixture
processes for all four transports. Its native macOS picker, combined workflows,
history reuse and result checks are recorded in the
[completion report](v0.2-completion.md). Windows native-picker runtime checks
and Linux desktop drag-and-drop checks remain outstanding.

- Codex documents exec image input and ephemeral/session behavior in the
  [CLI reference](https://developers.openai.com/codex/cli/reference/). Its model
  controls the accepted image payload; no universal CLI image-size limit is
  asserted here.
- GitHub documents repeatable file attachments and programmatic stdin input in
  [programmatic CLI reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-programmatic-reference)
  and [programmatic usage](https://docs.github.com/en/copilot/how-tos/copilot-cli/automate-copilot-cli/run-cli-programmatically).
  [Image usage](https://docs.github.com/en/copilot/how-tos/copilot-cli/use-copilot-cli/overview)
  requires an image-capable model. No fixed provider size limit is assumed.
- OpenCode documents `--file` in [run options](https://opencode.ai/docs/cli/#run).
  Released [v1.18.34 run source](https://github.com/anomalyco/opencode/blob/v1.18.34/packages/opencode/src/cli/cmd/run.ts)
  constructs local file parts; [prompt source](https://github.com/anomalyco/opencode/blob/v1.18.34/packages/opencode/src/session/prompt.ts)
  resolves explicitly attached images through its read path, including outside
  cwd. The run source's remote-attach 10 MiB limit is not treated as a local
  universal limit; CodeConvoy uses local invocation.
- Claude documents streaming image blocks in
  [streaming input](https://code.claude.com/docs/en/agent-sdk/streaming-vs-single-mode),
  with CLI framing visible in the
  [official SDK transport](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/_internal/transport/subprocess_cli.py).
  [Vision limits](https://platform.claude.com/docs/en/build-with-claude/vision)
  include a 10 MB base64 limit per image on the direct API, lower limits on
  partner services, request limits and dimension limits. CodeConvoy does not
  identify the configured provider or predecode dimensions; the CLI reports
  such rejection truthfully. [CLI flags](https://code.claude.com/docs/en/cli-reference)
  specify `--input-format stream-json` with print mode.

For a manual attachment smoke test, use trusted disposable Git repositories and
an already authenticated CLI. Attach a small UTF-8 specification outside the
repositories and a PNG screenshot, request an accurate description without
editing files, and check both repository jobs' responses and Raw output. Test
removing/changing a file while a second job waits, and reuse after removal.
Record the actual CLI/model/platform and outcome before claiming provider E2E.
