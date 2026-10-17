# CodeConvoy — Windows licensing and maintainer release decisions

## Maintainer renewal — 2026-10-17

During preparation of CodeConvoy **1.1.0**, the maintainer explicitly authorized
carrying the existing Windows Microsoft-component licensing-risk acceptance
forward to this version, preserving the current component terms and every
enforcement check. The catalog records this version-scoped renewal under
`windows-crt`; it does not extend acceptance to future releases.

The canonical notice/terms bytes and their SHA-256 binding remain unchanged.
The existing conditions, distribution responsibilities and uncertainty remain
as recorded for 1.0.0 below. This renewal is a maintainer risk decision, not a
new Microsoft grant or a finding of legal compliance. A known unmet mandatory
condition remains a blocker. No check, notice, prerequisite, source obligation
or candidate-verification requirement is waived. This preparation authorizes
the local release commit; tagging, pushing and publication remain separate.

## 1.0.0 decision record

Licensing assessment prepared 2026-10-16. This completes the
two decision questions identified in [the risk review](license-blocker-evidence.md#9-final-risk-based-windows-review)
using its existing evidence. It does not reopen historical NSIS provenance.
The policy for that release is the implemented maintainer decision below. Earlier
assessments and the prior stop decision are retained as historical evidence;
their recommendations do not override this later explicit instruction.
Microsoft permission and recipient agreement are not conclusively established.

## Implemented maintainer risk decision — 2026-10-16

The maintainer explicitly instructs implementation of the CodeConvoy **1.0.0**
release decision. The additional internal standard of an express redistribution
grant for every linked VC startup object is no longer an absolute publication
condition. `windows-crt` is recorded as a **maintainer-accepted licensing risk**,
not as an established Microsoft grant, resolved legal uncertainty, or verified
finding of compliance. No tag or publication is authorized by this preparation.

### Interpretation and mandatory conditions

The accepted interpretation is that the applicable licensed development/build
arrangement permits ordinary application output incorporating the recorded VC
startup/support code, subject to Microsoft's distribution conditions. The
express SDK linked-output basis and the limitations of the VC path mapping in
sections 1–3 remain unchanged. CPython, yt-dlp and Firefox provide comparison
evidence for distribution arrangements, not rights for CodeConvoy. No separate
Microsoft approval letter or legal-opinion certificate is imposed.

The existing record identifies no express contractual prohibition against this
specific recorded output. That is not proof of permission. It does identify a
mandatory protective-recipient-agreement condition: M1 II.7(b), second bullet,
and M3 §2(a)(ii), recipient-agreement bullet. The previous informational notice
alone left an implementation gap, so this change does **not** merely delete the
blocker. It incorporates the previously proposed, Microsoft-only operative
terms into the [canonical notice](../third-party/MICROSOFT-DISTRIBUTABLE-CODE.txt)
and places the corresponding pre-use notice in the README download guidance
and release notes. Existing NSIS/ZIP resources deliver the exact same text.
The permission is expressly limited to the distributor's applicable rights.

The intended agreement basis is informed exercise of the component permissions
after conspicuous notice and a reasonable opportunity to review, where
applicable law recognizes that conduct, or an existing applicable agreement.
The release notes link the exact 1.0.0 tagged text; packaged copies identify
version 1.0.0 and the date. The distributing party offers the terms; managed
and downstream distributors must arrange agreement for their recipients,
including direct-link, mirror and unattended deployments. Silent installation
and notice delivery do not themselves prove agreement. No installer click-through
is required by the cited clauses or introduced here.

This implements the known missing operative wording and planned delivery;
**the legal sufficiency and effectiveness of that arrangement remain an
accepted interpretation risk**, not an automated finding that recipients have
agreed. The SDK-derived USD 5 limitation is retained as the proposed protective
choice for both Microsoft portions, with mandatory-law and direct-warranty
exceptions. Its enforceability is not certified. The distributor must still
meet the agreement condition in actual distribution; a specifically identified
failure to do so must be treated as an unmet obligation, not covered by this
risk acceptance. The maintainer's instruction does not waive a known prohibition
or mandatory condition. No new evidence of either is asserted by this change.

All Microsoft ownership notices, distribution restrictions, defense/indemnity
undertakings (including the differing VS/SDK scope), and trade obligations
remain. CodeConvoy's MIT license and independent component rights are unchanged.
No assurance of legal compliance is inferred from the maintainer's willingness
to release. The questions for qualified legal determination remain the precise
VC grant coverage and sufficiency/formation of the recipient agreement; such
an opinion is optional assurance, not a newly invented Microsoft condition.

### Publication policy and evidence

The catalog moves only `windows-crt` from `release_blockers` to
`accepted_licensing_risks` and changes the Microsoft component reference to
`accepted_risk`. The decision is limited to version 1.0.0 and bound to the
canonical notice/terms SHA-256. The checker rejects a missing or stale decision,
changed terms binding, and conflicts between accepted risk and active blockers.
Acceptance cannot excuse missing notices or untracked obligations. The publisher
still calls the unchanged `require_release_ready()` enforcement function before
creating even a draft. There is no bypass flag or blanket obligation waiver.
The generated inventory and aggregate notice expose the accepted risk separately.

Historical native captures, source archives, linker evidence and their hashes
are unchanged, including their point-in-time unresolved assessments. The current
policy record supersedes those assessments only as a publication-policy choice.
Changed Microsoft terms, toolchain/channel, native payload, recipient terms or
distribution method require affected-scope review; a concrete contradictory fact
must be recorded as a blocker. Other license and package checks are unchanged.

The Linux runner fix is present on `main` at
`c749a5ec9a38b85976ed497dbd957e0be737d355`; its
[controlled reproduction and correction](appimage-runtime-validation.md#candidate-38936028334-ubuntu-sandbox-failure-and-correction)
are separate from this decision. Candidate 38936028334 failed and remains
ineligible. A fresh complete, successful non-publishing release candidate from
the final committed source must validate the Linux fix and changed release
materials. Passing `release-ready` alone cannot qualify a candidate or authorize
publication. This work stops as local changes for maintainer review.

### Validation of the implemented decision

- `python3 packaging/licenses.py check`: passed; 457 locked crates, zero
  unresolved publication obligations and one maintainer-accepted licensing risk.
- `python3 packaging/licenses.py release-ready`: **exit 0**. This is the actual
  internal publication-policy result, not verified legal compliance.
- All **110 packaging tests** passed, including notice/resource/ZIP integrity,
  accepted-risk binding and rejection of unrelated obligations. A removed
  Microsoft notice now fails the ordinary missing-notice gate earlier than its
  specialized binding check; the regression expectation was updated accordingly.
- Local formatting, strict Clippy, Rust tests (621 passed, 10 intentionally
  ignored) and the release build passed with installed Rust 1.96.1. These
  checks do not certify native Windows packages.
- All other component/text records, source archives, tool pins and native
  provenance remain identical. The source ZIP remains SHA-256
  `eb61566a9f3e26f7c5d29482cabb22828a86cd6b985480e48faf196e77ba7b8f`.
- Canonical Microsoft notice/terms SHA-256 is
  `ec0484b390241e0fbc0e9608ca8c4e4f605f1acd6cb918db61d1756a287846d2`.
  It occurs exactly once in the generated aggregate, which passed the existing
  directory verifier. Actual final NSIS/ZIP validation remains part of the
  fresh candidate; no new release workflow was dispatched in this step.

## Historical release-intent decision (superseded policy)

The following records the earlier instruction and its actual checks. Its
recommendation to retain G1/G2 as blockers is superseded by the explicit risk
decision above; the factual evidence and limitations remain relevant.

## Maintainer release decision — 2026-10-16

The maintainer explicitly decided to proceed with CodeConvoy **1.0.0** and
declared the technical and licensing investigations complete. That decision
is recorded as release intent, with the documented Microsoft interpretation,
conditions and uncertainty preserved. It is not a verified finding that the VC
output grant (G1) or recipient agreement (G2) has been established. The same
instruction prohibits silently clearing unresolved obligations and requires
stopping if publication would override an unresolved material requirement.
**That stop condition applies. No publication override is made.**

### Release-material review

The existing `MICROSOFT-DISTRIBUTABLE-CODE.txt` is present exactly once and
unchanged in the generated `THIRD-PARTY-NOTICES.txt`, which uses the existing
installer/ZIP delivery paths. This is a check of release materials, not a new
claim about completed candidate packages. The notice communicates the recorded
Microsoft conditions and separates them from MIT; its summary status remains
explicit.

The proposed terms remain a review draft outside those materials. In particular,
their section 1 offers permissions whose upstream basis is G1; their section 4
applies the SDK's USD 5 ceiling to both Microsoft portions as a proposed choice;
and their sections 1/3 depend on an effective recipient-agreement arrangement.
The release instruction supplies no new determination of those points. Simply
copying the draft into packages would not establish them, and it must not be
presented as verified required terms or effective agreement. **Final operative
recipient terms/delivery remain an unresolved release-material requirement
under G2.** The proposed scoped text and delivery plan below remain available
for the maintainer's review; no new click-through or packaging change is made.

### Exact publication-guard consequence

`packaging/publish.py` calls `require_release_ready()` before creating a release.
That function in `packaging/licenses.py` checks the catalog and rejects every
remaining `release_blockers` entry. The catalog also requires every component's
`unresolved` reference to name an existing blocker. There is no maintainer-risk
override setting.

After supported resolution, the mechanical change would be to remove only
`windows-crt` from `third-party/catalog.json`'s `release_blockers` and remove the
Microsoft component's corresponding `unresolved` marker, updating the decision
record and adopted notice/text/hash bindings consistently. The regression that
currently asserts this active blocker would need its expectation updated while
preserving notice-integrity and blocked-state coverage. No change to the
enforcement function or two-stage workflow is needed for a genuine resolution.
Deleting only the blocker fails the unresolved-reference check; deleting only
the component marker still leaves publication blocked.

**With G1/G2 still recorded as material and unresolved, removing both markers
would suppress those obligations; bypassing the function would defeat the
check. Neither is a justified readiness change under this instruction.**

### Source, candidate and actual readiness result

At the initial check, local HEAD and remote `main` both identified
`56dfe5548a1a4594e9abe45b0ed83290b48507f4`; five licensing-review files were
staged. During this request they were committed and pushed outside this work.
The final source check found local HEAD and remote `main` at
`eec04e9ffee471d55b6062d5f1494bc0eb2961eb` (`docs(licensing): narrow Windows CRT
publication blockers`). This step neither staged nor committed those changes.
The new release-decision documentation remains uncommitted, so there is
**no committed final source containing this decision**.
The exact-commit Release workflow lookup returned no run for `eec04e9` at the
time of this check.

The existing [candidate run 38936028334](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38936028334)
was independently verified through GitHub's run metadata: repository
`ChrisLauinger77/code-convoy`, `.github/workflows/release.yml`,
`workflow_dispatch`, branch `main`, attempt **1**, and source
`56dfe5548a1a4594e9abe45b0ed83290b48507f4` — **not current HEAD**.
Correction after inspecting individual jobs: the [Linux job](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38936028334/job/117091962627)
had already **failed** at 17:09:55 UTC in “Package and verify DEB, RPM, AppImage
and host tools.” The overall run still reported `in_progress` because Windows
was running; that overall status must not obscure the failed Linux job. macOS
passed. The run is not an eligible candidate, regardless of the remaining job.

Linux formatting, Clippy, Rust tests, application build and probe had passed.
Failure occurred while preparing the AppImage runtime build environment: the
`bwrap ... /sbin/apk add --no-network --no-chown ...` command exited 1. Its
actual diagnostic output was redirected to
`target/packaging/linux/runtime-build/build.log`, which the workflow did not
print or retain. The available job log therefore identifies the failing command
but does not establish whether bubblewrap setup or Alpine package installation
caused it. Do not infer a particular root cause from the wrapper traceback.
Subsequent Linux verification/upload steps were skipped. The artifact listing
contained `packages-macos` but no aggregate `release-assets` artifact.
This is a separate technical packaging failure from `windows-crt`; no complete
six-package artifact/checksum provenance is certified. No workflow was
dispatched or runtime/build code changed by this inspection.

That run contains the committed informational notice, not the subsequently
committed review or proposed operative terms. The two-stage workflow requires
candidate and tagged source commits to match exactly. Any later commit changing the catalog or
release materials requires a fresh candidate from that commit; this run cannot
be relabelled as validation of later source, even for documentation-only changes.

`python3 packaging/licenses.py release-ready` was executed against the current
working tree and returned **exit 1**:

```text
ValueError: Third-party license release blockers remain: windows-crt
```

Only this focused material/guard/provenance check and documentation checks were
performed. Earlier audits, builds and tests were not repeated. Application,
CRT, helpers, packaged notices, catalog/guard and workflow are unchanged by
this step. No commit, push, merge, tag or publication was performed. Work stops
for maintainer review with the material blocker and incomplete final-source/
candidate status explicitly retained.

## 1. VC redistribution rights

### Express provisions and the actual components

The reference agreement is Visual Studio Enterprise/Professional 2026,
EULAID `VS2026_EntPro_ENU.1033`, updated 2025-10-01. Its **II.6(g)(i)** gives
accompanying Microsoft component agreements precedence. **II.7(a)(i)** permits
object-code distribution for code identified by the Distributable List;
**II.7(a)(iii)** permits further distribution within the application. The
sample/template grant in **II.7(a)(ii)** does not identify these inputs.
**II.2** and **II.6(c)** authorize specified development/build uses; they are
not express grants to distribute every toolchain file or its contents.
**II.10** reserves ungranted rights. See [M1–M2](#microsoft-references).

| Actual CodeConvoy component | Recorded Microsoft input | Applicable grant and conclusion |
| --- | --- | --- |
| `CodeConvoy.exe`, x64, installer and ZIP | MSVC 14.51.36231 `msvcrt.lib`: 34 selected member names; `vcruntime.lib` import members | M1 II.7(a)(i), subject to II.6(g)(i). M2 lists runtime merge modules and the `VC\redist` directory, not the observed `VC\Tools\MSVC\14.51.36231\lib\x64` inputs. **Coverage of this linked VC output is an interpretation, not an express path match.** |
| `codeconvoy_utils.dll`, x86, installer | Same toolset, `msvcrt.lib`: 24 selected member names; `vcruntime.lib` import members | Same issue for the toolset's `lib\x86` inputs. Dynamic imports do not eliminate the startup/support code. |
| Both preceding binaries | SDK 10.0.26100.0 `ucrt.lib` and recorded SDK system import/support libraries | M3 §2(a)(i), §2(a)(ii), and M4 expressly cover linked output from public SDK `um`/`ucrt` directories for these architectures, subject to distribution conditions. |
| `CodeConvoyAppId.dll`, x86, installer | SDK `kernel32`/`ole32` imports and UUID data; no CRT | Same SDK directory grant; no VC startup grant is needed for this helper. |
| Unchanged official NSIS stub, uninstaller and plugins | Existing provenance and residual historical uncertainty | Keep the section 9 risk disposition. Neither assert that these are CRT-free nor invent a separate VC grant. No specific new contrary fact makes historical reconstruction necessary. |
| External VC/UCRT DLLs and redistributable installer | User/OS supplied, not packaged | Outside this distribution. Their runtime end-user license cannot substitute for the embedded-code grant. |

The member counts describe selection, not proof that every selected object
survives optimization. Exact paths, hashes and final validation are already in
[the native record](nsis-helper-validation.md#retained-dynamic-linker-reference).
The unrelated `windows.0.52.0` import archive keeps its existing license decision.

### Interpretation and final answer to G1

A reasonable argument is that a licensed development tool's ordinary output
permission encompasses the startup code required by its documented dynamic-CRT
linking model. [M5](#microsoft-references) confirms that identified distributable
libraries may be distributed as linked output. However, its identification
condition remains; [M6](#microsoft-references) explains linkage, not licensing.
The argument therefore needs a contract interpretation that includes the
observed VC code despite its absence from M2's listed paths, or applicable
component terms that cover it. A redistributable DLL entry does not expressly
license a different startup implementation. Absence from those paths also does
not establish infringement.

**Answer:** the documents provide a supportable express basis for the SDK
output, and support a plausible ordinary-output interpretation for VC. They do
**not, without that interpretive determination, establish a sufficiently
supported VC redistribution basis to recommend closing G1**. This is a limit
of the present grant mapping, not a conclusion that normal MSVC output is
unredistributable. No separate approval letter is a condition of II.7.

The hosted-image record supports ordinary authorized build use; it does not
identify which agreement supplies this distributor's VC output rights. Treat
that as a premise of the same grant determination, not an independent demand
for a purchased subscription or licensing certificate. The question for a
qualified licensing reviewer is: **under the agreement applicable to this
recorded hosted Enterprise build, do II.7 and any controlling II.6(g)(i) terms
authorize the selected `msvcrt.lib` startup/support and `vcruntime.lib` output?**
A reasoned yes must identify the contractual basis. No new binary evidence is
needed to answer it.

## 2. Recipient and downstream terms

M1 **II.7(b), second bullet**, and M3 **§2(a)(ii), recipient-agreement bullet**,
require agreement to sufficiently protective terms. The VS requirements are
unnumbered bullets in the reviewed document; earlier references to
“II.7(b)(ii)/(iii)” mean its second/third bullets, not separately numbered clauses.

The existing notice accurately distinguishes ownership and conditions, but
expressly remains a summary. It does not offer a settled component license or
establish assent. It is therefore insufficient as the sole basis for G2.
The linked draft supplies operative, Microsoft-only permission conditions,
downstream agreement duties, notice preservation, use restrictions, and
Microsoft/supplier protections with mandatory-law exceptions. CodeConvoy's MIT
license remains unchanged. It grants no rights until adopted following G1.

The scope/use protections draw on M1 II.10 and II.15 and M3 §§7–8. The proposed
warranty/liability provisions draw on M3 §§13–15 and preserve nonwaivable rights
and any direct Microsoft warranty. The USD 5 ceiling follows the SDK provision;
applying it to both identified Microsoft portions is a **proposed protective
choice**, not a claim that the full-use VS agreement sets that ceiling. A
qualified reviewer must assess whether these selected protections are enough
under both pass-through clauses and enforceable in the intended distribution.
The draft does not import IDE-seat, telemetry, product-transfer or forum terms
into CodeConvoy usage. It does not impose Microsoft's developer indemnity on
ordinary end users. The maintainer's own undertakings remain under M1 II.7(b),
third bullet, and M3 §2(a)(ii); the VS exception for claims solely about
Distributable Code is absent from the SDK undertaking.

### Smallest proposed delivery arrangement

After approval, replace the summary's operative portion with the reviewed terms
through the existing aggregate-notice mechanism, preserving component scope
and updating its existing text/hash bindings. This is a later content change;
no packaging implementation or notice binding is changed by this proposal.

Before the Windows download links, present this text with a link to the exact
versioned terms:

> Windows binaries contain Microsoft startup and SDK code under separate
> Microsoft-component terms. Review those terms before use or redistribution.
> Permission to use or redistribute the Microsoft portions is offered only on
> agreement to those terms.
> They do not change CodeConvoy's MIT source license. If you do not agree, do
> not use or redistribute the Microsoft portions of these binaries.

Keep the same terms in the installer payload and ZIP through the existing
notice file. For silent/managed deployment, the deploying distributor must
arrange agreement before use, directly or through an authorized organizational
agreement; running a silent installer is not evidence that every user agreed.
Downstream distributors must supply the terms and require equivalent agreement,
including when their recipients bypass the original download page.

**Assent model:** deliberate use/copying/redistribution after conspicuous notice
and a reasonable opportunity to review, where applicable law recognizes that
conduct as agreement, or an existing applicable agreement. Neither a buried
file nor the sentence “by using you agree” proves formation by itself. Identify
the responsible distributor and fix the terms version in the publication record.
An installer checkbox is not prescribed by either distribution clause and is
not proposed. Notice on the download page alone does not solve direct-link,
mirror or unattended delivery.

**Answer to G2:** this is a reasonable, limited candidate arrangement for
satisfying the requirement, provided a qualified determination supports both
its substantive protection and agreement formation for the intended channels.
It is not yet fulfilled merely because a draft exists or a file is delivered.
The remaining question is **whether these component-only terms and this
notice/conduct or existing-agreement model satisfy the two pass-through clauses
for CodeConvoy's distributor and intended recipients, including mandatory
consumer protections**. If they do not, identify the deficient protection or
channel and fix only that deficiency; do not assume a click-through is needed.

## 3. Requirements and proposed maintainer disposition

| Requirement | Fulfilment or decision needed |
| --- | --- |
| Authorized VC output and further-distribution grant | G1 remains open as described above; maintain the guard. |
| SDK listed inputs distributed only as linked output | Supported by existing paths and package inventory; no standalone `.lib` distribution. |
| Substantial independent functionality, valid application copyright | Existing application and MIT copyright notice supply these elements. |
| Released toolchain/components and unmodified authorized Microsoft code | Existing non-prerelease toolchain/input evidence supports these elements; no new binary modifications. No right to bundle debug/preview code is inferred. |
| Preserve notices, distinguish Microsoft ownership, no misleading branding | Existing notice/package evidence and proposed terms §§1–2; MIT applies independently. |
| SDK Windows-platform and lawful-program restrictions; no excluded licensing of Microsoft source | Existing Windows-only payload, independent functionality and source licensing support these elements; proposed §2 passes conditions onward. |
| No separate runtime redistribution | Fulfilled by the current package contents; users supply prerequisites. |
| Protective terms and downstream/end-user agreement | Proposed §§1–5 and the delivery plan; **not yet adopted or established as effective** (G2). |
| Distributor defense/indemnity and trade compliance | Continuing duties of the actual distributor under the applicable agreements, not tests or an end-user indemnity. Maintainer adoption must account for them. |
| SDK setup/WPT-specific conditions | No SDK setup package or WPT code is identified in this payload. The NSIS installer is not, merely by being an installer, the SDK-supplied setup program described in that clause. |

**Proposed decision for adoption:** retain `windows-crt` until G1's contractual
interpretation and G2's terms/agreement arrangement are supported and adopted.
Once both are resolved, record the reasoning and adopted text/delivery version
and authorize removal separately. Do not demand an individual Microsoft letter,
complete historical NSIS reconstruction or another unchanged-input audit.
The change triggers and automated checks in section 9 continue to apply.

This is the final technical/compliance handoff for these questions, not
authoritative legal advice. The two qualified determinations above can be
handled together using these documents and the existing evidence; this proposal
does not start another investigation, contact anyone or bind the maintainer.
Qualified review is recommended because the grant interpretation and agreement
formation remain unsettled, not because Microsoft expressly requires a lawyer's
opinion or individual approval letter.

## Microsoft references

| ID | Exact source and relevant provisions |
| --- | --- |
| M1 | [VS 2026 Enterprise/Professional terms](https://visualstudio.microsoft.com/license-terms/vs2026-ga-pro-enterprise/), [DOCX](https://visualstudio.microsoft.com/wp-content/uploads/2025/10/Visual_Studio_2026-License-Enterprise_Professional_ENU.docx): II.2; II.6(c), II.6(g)(i); II.7(a)–(c); II.10; II.15; II.19 and Limited Warranty. Recorded SHA-256 `dd2ab92a7c2bb90dd509e16572707a34b13bc0bc09b9199d115f25594ac39053`. |
| M2 | [VS 2026 Distributable List](https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution#visual-c-runtime-files), “Distributable Code Files” and “Visual C++ Runtime Files”: licensed-copy condition, listed runtime files/directories, preview/debug exclusions. |
| M3 | [Windows SDK terms](https://aka.ms/WinSDKLicenseURL), [resolved RTF](https://download.microsoft.com/download/0/F/F/0FF2B061-47DD-4F55-89B6-FD1D8C44F14D/sdk_license.rtf): §2(a)(i)–(iii), §7 Scope of License, §8 Export Restrictions, §13 Legal Effect, §14 Disclaimer of Warranty, §15 Limitation on and Exclusion of Remedies and Damages. EULAID `WIN10SDK.RTM.AUG_2018_en-US`; SHA-256 `dd07eb178e00c6bba4148457fc00ff77cd4887eb521d504186fe59c9ec8bbe62`. |
| M4 | [SDK REDIST static LIB files](https://learn.microsoft.com/en-us/legal/windows-sdk/redist#static-lib-files): public SDK `um`/`ucrt` architecture-directory coverage, unmodified linked output. |
| M5 | [Microsoft licensing guidance](https://www.microsoft.com/licensing/guidance/Visual-Studio#how-certain-software-can-be-distributed-to-others-within-your-applications): identification by REDIST; distribution of output from identified distributable `.lib` inputs. Guidance, not a replacement grant. |
| M6 | [CRT library documentation](https://learn.microsoft.com/en-us/cpp/c-runtime-library/crt-library-features): `msvcrt.lib` static startup for dynamic CRT use. Technical interpretation of already captured inputs, not permission. |

The published list/guidance references were checked on 2026-10-16; the retained
agreement texts above were reused. No source or binary audit was repeated.

## Review boundary

Only documentation is prepared in this step. The packaged notice, its bindings,
MIT license, catalog/publication guard, application, helpers and packaging code
are unchanged from the start of this request. Earlier uncommitted review edits
remain intact. Documentation links/whitespace and those unchanged-file hashes
were checked; compilation and technical audits were not repeated. No commit,
push, release action or external enquiry is authorized by this proposal.
