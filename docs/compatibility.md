# Compatibility for the 1.x series

This is the intended compatibility approach for CodeConvoy 1.0 and later 1.x
releases. It does not announce a 1.0 release; the current application version
remains defined by `Cargo.toml`. Compatibility covers user data and documented
behavior, not a frozen internal implementation or an unrestricted platform promise.

## Settings and saved history

The intent is to preserve supported existing user configuration, task text,
repository registrations/groups, backend preferences and retained history through
upgrades. Application version, internal state version and export schema version
are independent. Current state remains `version: 1`; no reset or migration is
required merely to label an application release 1.0.

New optional fields use safe defaults. Unsupported versions, invalid records and
unknown enum values fail explicitly and preserve the original file rather than
silently resetting data. Older completion observations remain Unknown when they
were never recorded. Unfinished agents/validations recover as interrupted or
cancelled; no process is resumed. Saved worktree observations never replace fresh
ownership and Git checks. See [data-format details](history.md#data-format-compatibility).

Internal JSON fields, worktree ownership manifests, journals and location bindings
are application-managed recovery data. They may evolve with compatible loading or
explicit migrations; they are not public plugin/storage APIs. Editing these files
by hand is not a supported way to change permissions or ownership. Task templates
contain task text, not backend settings or workflows.

Back up the complete application data directory and any custom retained worktree
locations before upgrading. Exports and `state.json` alone cannot restore owned
results. Downgrades are not guaranteed: unknown additive fields may be lost on
save, and a newer enum or state version can prevent an older build from loading.
Use a compatible application and keep the original backup.

## Export consumers

[JSON history export schema 1](history-export.md#json-schema-1) is the public report
contract. Preserve its documented field meanings and distinguish `null` from zero.
Consumers should check `schema_version`, tolerate additive fields and reject an
unsupported schema explicitly. Incompatible report changes should use a new
schema version. Markdown is a human-readable summary whose layout can evolve;
use JSON for structured consumers. Neither format is a backup or import format.

## Operating systems and packages

The [README platform table](../README.md#downloads-and-platform-support) lists
artifact architectures, installation requirements and signing limits. Linux's
intended minimum binary ABI is glibc 2.39; every final packaged ELF is checked.
That ceiling does not promise every glibc-based distribution, graphics driver,
portal, filesystem or desktop will work. Windows and macOS likewise need native
checks beyond compilation and packaging. Only the
[verification matrix](platform-validation.md) establishes performed tests for a
particular candidate/environment. Historical passes do not certify new bytes.

Platform baseline changes should be documented in release notes and installation
requirements. There is no promise of permanent support for an OS version, an
untested architecture or a separate long-term support branch.

## External agent CLIs

Each backend owns its CLI detection, settings, arguments, output and completion
rules. A CLI passing **Check CLI** does not establish authentication, model access
or compatibility with every future CLI release. Supported behavior is documented
in the [backend contracts](../README.md#codex-contract) and their linked evidence.
Providers can change modes, account eligibility and organization policy independently
of CodeConvoy. Unknown/unsupported settings must be reported, not silently mapped
to more permissive options. Permission models remain backend-specific.

CodeConvoy does not install/update agents or manage provider accounts. Record the
exact CLI version when reporting problems. Changes needed for real CLI behavior
may be shipped within 1.x with corresponding documentation and regression tests.
The core repository isolation, explicit destructive-action confirmation and honest
interruption/recovery semantics remain safety requirements throughout those changes.
