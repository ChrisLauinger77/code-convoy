"""Fail-closed GLIBC inspection of the final Linux release packages."""
import os
import re
import tempfile
from pathlib import Path

from release import checksum, run

MAX_GLIBC = (2, 39)


def required_glibc(output):
    """Read requirements, never exported definitions, from GNU readelf output.

    Check both the number of libraries and each library's auxiliary records so
    truncated/changed output cannot silently lower the measured requirement.
    """
    if output.strip() == "No version information found in this file.":
        return None  # Valid for the statically linked AppImage runtime.
    sections = re.split(r"(?m)^(?=Version .* section )", output)
    if sections[0].strip() or len(sections) == 1:
        raise ValueError("Malformed or incomplete ELF version information")
    versions = []
    seen = set()
    for section in sections[1:]:
        lines = section.strip().splitlines()
        header = re.fullmatch(r"Version (symbols|definition|needs) section '\S+' contains (\d+) entr(?:y|ies):", lines[0])
        if not header or header[1] in seen:
            raise ValueError("Malformed or duplicate ELF version section")
        seen.add(header[1])
        if header[1] != "needs":
            continue
        libraries = 0
        remaining = 0
        address = False
        for line in lines[1:]:
            line = line.strip()
            if not line:
                continue
            if re.fullmatch(r"Addr: 0x[0-9a-f]+\s+Offset: 0x[0-9a-f]+\s+Link: \d+ \(\S+\)", line):
                if address or libraries:
                    raise ValueError("Malformed ELF version needs address")
                address = True
                continue
            library = re.fullmatch(r"(?:0x)?[0-9a-f]+: Version: 1\s+File: \S+\s+Cnt: (\d+)", line)
            if library:
                if not address or remaining or int(library[1]) == 0:
                    raise ValueError("Incomplete ELF version needs records")
                libraries += 1
                remaining = int(library[1])
                continue
            name = re.fullmatch(r"(?:0x)?[0-9a-f]+:\s+Name: (\S+)\s+Flags: (?:none|WEAK)\s+Version: \d+", line)
            if not name or not remaining:
                raise ValueError(f"Malformed ELF version needs record: {line}")
            remaining -= 1
            if name[1] == "GLIBC" or name[1].startswith("GLIBC_"):
                if not re.fullmatch(r"GLIBC_[0-9]+(?:\.[0-9]+)+", name[1]):
                    raise ValueError(f"Unrecognized glibc ABI requirement: {name[1]}")
                versions.append(tuple(map(int, name[1].removeprefix("GLIBC_").split("."))))
        if not address or remaining or libraries != int(header[2]) or not libraries:
            raise ValueError("Incomplete ELF version needs table")
    return max(versions) if versions else None


def inspect_glibc(path):
    result = run("readelf", "--version-info", "--wide", path, capture_output=True,
                 text=True, env=dict(os.environ, LC_ALL="C"))
    # readelf can diagnose corrupt/truncated tables while returning success.
    if result.stderr.strip():
        raise ValueError(f"Cannot inspect ELF {path}: {result.stderr.strip()}")
    try:
        return required_glibc(result.stdout)
    except ValueError as error:
        raise ValueError(f"Cannot inspect ELF {path}: {error}") from error


def glibc_label(value):
    return ".".join(map(str, value)) if value else "none (no versioned GLIBC requirement)"


def is_elf(path):
    with path.open("rb") as source:
        return source.read(4) == b"\x7fELF"


def inspect_payload(root):
    main = root / "usr/bin/codeconvoy"
    if main.is_symlink() or not main.is_file() or not is_elf(main):
        raise ValueError("Final Linux package is missing its ELF executable")
    requirements = []
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            # An internal alias is covered by its regular target. Never follow
            # an external/broken link and accidentally inspect a host library.
            if not path.resolve(strict=True).is_relative_to(root.resolve()):
                raise ValueError(f"Package symlink escapes its payload: {path}")
            continue
        if path.is_file() and is_elf(path):
            requirements.append((str(path.relative_to(root)), inspect_glibc(path)))
    if dict(requirements)["usr/bin/codeconvoy"] is None:
        raise ValueError("No glibc requirements found in the final Linux executable")
    return requirements


def inspect_artifact(artifact, work):
    """Extract and read the finished bytes, independently of the staging tree."""
    artifact = artifact.resolve(strict=True)
    requirements = []
    with tempfile.TemporaryDirectory(prefix="final-abi-", dir=work) as temporary:
        directory = Path(temporary)
        payload = directory / "payload"
        if artifact.suffix == ".deb":
            run("dpkg-deb", "--extract", artifact, payload)
        elif artifact.suffix == ".rpm":
            archive = directory / "payload.cpio"
            with archive.open("wb") as output:
                run("rpm2cpio", artifact, stdout=output)
            payload.mkdir()
            with archive.open("rb") as source:
                run("cpio", "--extract", "--make-directories", "--no-absolute-filenames",
                    "--quiet", "--directory", payload, stdin=source)
        elif artifact.suffix == ".AppImage":
            runtime = inspect_glibc(artifact)
            requirements.append(("AppImage runtime", runtime))
            if runtime and runtime > MAX_GLIBC:
                raise ValueError(f"{artifact.name}: AppImage runtime requires GLIBC_{glibc_label(runtime)}, maximum is {glibc_label(MAX_GLIBC)}")
            # The pinned runtime reports the embedded filesystem's exact offset;
            # unsquashfs reads that final image without launching its application.
            offset = run(artifact, "--appimage-offset", capture_output=True, text=True).stdout.strip()
            if not re.fullmatch(r"[1-9][0-9]*", offset):
                raise ValueError(f"Invalid AppImage filesystem offset: {offset!r}")
            run("unsquashfs", "-no-progress", "-d", payload, "-o", offset, artifact)
        else:
            raise ValueError(f"Unsupported Linux artifact: {artifact.name}")
        requirements.extend(inspect_payload(payload))
    maximum = max(value for _, value in requirements if value)
    report = f"{artifact.name}: maximum required GLIBC {glibc_label(maximum)} (ceiling {glibc_label(MAX_GLIBC)})\n"
    report += f"SHA256 {checksum(artifact)}\n"
    report += "".join(f"  {glibc_label(value)}  {name}\n" for name, value in requirements)
    print(report, flush=True)
    if maximum > MAX_GLIBC:
        raise ValueError(f"Final artifact exceeds GLIBC {glibc_label(MAX_GLIBC)}:\n{report}")
    return report


def verify_artifacts(artifacts, work):
    report = "\n".join(inspect_artifact(artifact, work) for artifact in artifacts)
    (work / "glibc-requirements.txt").write_text(report, encoding="utf-8")
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(summary, "a", encoding="utf-8") as output:
            output.write(f"\n### Final Linux artifact ABI inspection\n\n```text\n{report}```\n")
