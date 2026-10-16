# CodeConvoy 1.0.0 — proposed final Windows licensing decision

Prepared 2026-10-16 for maintainer review; **not adopted**. This completes the
two decision questions identified in [the risk review](license-blocker-evidence.md#9-final-risk-based-windows-review)
using its existing evidence. It does not reopen historical NSIS provenance.

**Recommendation: do not close `windows-crt` on the present record.** SDK linked
output has an express conditional basis. Ordinary linked VC startup output has
a plausible licensing interpretation, but the reviewed documents do not
establish the necessary inclusion in the controlling grant. The proposed
[recipient terms](windows-microsoft-terms-proposed.txt) address the current
notice's substantive gap; their adoption and a legally effective agreement
arrangement remain necessary. These are two bounded legal/contractual decisions,
not reasons to rerun technical audits or request blanket Microsoft approval.

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
