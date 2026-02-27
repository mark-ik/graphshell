#!/usr/bin/env python3
import json
import re
import subprocess
import sys
from pathlib import Path

ALLOWED_LICENSE_IDS = {
    "MIT",
    "Apache-2.0",
    "BSD-1-Clause",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "MPL-2.0",
    "Zlib",
    "BSL-1.0",
    "CC0-1.0",
    "MIT-0",
    "NCSA",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "OFL-1.1",
    "Ubuntu-font-1.0",
    "CDLA-Permissive-2.0",
    "OpenSSL",
    "Unlicense",
    "0BSD",
}

BANNED_ONLY_IDS = {
    "GPL-2.0",
    "GPL-2.0-only",
    "GPL-2.0-or-later",
    "GPL-3.0",
    "GPL-3.0-only",
    "GPL-3.0-or-later",
    "AGPL-3.0",
    "AGPL-3.0-only",
    "AGPL-3.0-or-later",
    "LGPL-2.1-only",
    "LGPL-2.1-or-later",
    "LGPL-3.0-only",
    "LGPL-3.0-or-later",
}

ALLOWED_UNKNOWN = {
    "tokio-tungstenite-wasm": "temporary allowlist: upstream crate metadata omits license expression",
}

TOKEN_RE = re.compile(r"[A-Za-z0-9.+-]+")


def run_license_scan(output_path: Path) -> None:
    result = subprocess.run(
        ["cargo", "license", "--json"],
        capture_output=True,
        text=False,
        check=False,
    )
    if result.returncode != 0:
        print((result.stdout or b"").decode("utf-8", errors="replace"))
        print((result.stderr or b"").decode("utf-8", errors="replace"), file=sys.stderr)
        raise SystemExit(result.returncode)
    output_path.write_text((result.stdout or b"").decode("utf-8", errors="replace"), encoding="utf-8")


def extract_tokens(license_expr: str) -> set[str]:
    tokens = set(TOKEN_RE.findall(license_expr or ""))
    return {t for t in tokens if t not in {"AND", "OR", "WITH"}}


def is_acceptable_expression(license_expr: str) -> tuple[bool, str]:
    tokens = extract_tokens(license_expr)
    if not tokens:
        return False, "no SPDX tokens found"

    has_allowed = any(t in ALLOWED_LICENSE_IDS for t in tokens)
    has_banned = any(t in BANNED_ONLY_IDS for t in tokens)

    if not has_allowed:
        return False, f"no allowed license option in expression ({license_expr})"

    if has_banned and "OR" not in (license_expr or ""):
        return False, f"banned copyleft-only expression ({license_expr})"

    return True, "ok"


def main() -> int:
    report_path = Path("license-report.json")
    run_license_scan(report_path)
    data = json.loads(report_path.read_text(encoding="utf-8"))

    violations = []
    unknowns = []

    for crate in data:
        name = crate.get("name", "<unknown>")
        version = crate.get("version", "<unknown>")
        license_expr = (crate.get("license") or "").strip()

        if not license_expr:
            if name not in ALLOWED_UNKNOWN:
                unknowns.append((name, version, "missing license expression"))
            continue

        ok, reason = is_acceptable_expression(license_expr)
        if not ok:
            violations.append((name, version, license_expr, reason))

    if violations or unknowns:
        print("License gate failed.")
        if violations:
            print("\nDisallowed license findings:")
            for name, version, expr, reason in violations:
                print(f"- {name} {version}: {expr} ({reason})")
        if unknowns:
            print("\nUnknown/unlicensed findings (not allowlisted):")
            for name, version, reason in unknowns:
                print(f"- {name} {version}: {reason}")
        if ALLOWED_UNKNOWN:
            print("\nCurrent allowlisted unknowns:")
            for name, why in ALLOWED_UNKNOWN.items():
                print(f"- {name}: {why}")
        return 1

    print("License gate passed: all crates have acceptable license options and unknowns are allowlisted.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
