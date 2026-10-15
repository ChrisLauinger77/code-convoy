# Security

## Reporting a suspected vulnerability

Use GitHub's private vulnerability reporting to
[report a vulnerability privately](https://github.com/ChrisLauinger77/code-convoy/security/advisories/new).
You can also open the repository's **Security → Advisories → Report a vulnerability**
form while signed in to GitHub. Private reporting is enabled for this repository.

Do not publish exploit details, private repositories, credentials or unredacted
logs in a public issue. Keep vulnerability details and proof-of-concept material
in the private report.

A private report should include the affected version/revision, OS/package type,
backend CLI version where relevant, impact, and a minimal reproduction using
disposable data. Do not send live credentials or authentication files.

Security triage is best effort. No response-time guarantee, bounty or supported
long-term maintenance branch is offered. The latest release is the first target
for fixes; any backport depends on maintainer review. Keep older affected versions
in the report rather than assuming an upgrade proves the problem resolved.

## Security boundaries

CodeConvoy orchestrates local Git and installed coding-agent CLIs under your user
account. It is not an additional sandbox. Backend permissions differ, and a CLI
or configured validation command can modify files accessible to that account.
Provider authentication and data handling remain with the external CLI/provider.
CodeConvoy does not request or manage API keys.

Repository leases coordinate CodeConvoy operations; they do not lock out external
tools or hostile same-user processes. Git ownership checks and explicit cleanup
confirmation protect managed worktrees. Cancellation retains partial changes;
it cannot roll them back. Unix descendants that deliberately detach into another
session/group, abrupt termination and OS crashes have documented supervision
limits. Unconfirmed managed cleanup must remain protected, not reported as safe.

Tasks, templates and validation arguments can persist locally. Session output,
screenshots and exported reports can still contain sensitive text even where
structural log/path fields are omitted. Review before sharing. See
[data handling](README.md#repository-safety-and-data),
[export privacy](docs/history-export.md#privacy) and
[compatibility/recovery](docs/compatibility.md).
