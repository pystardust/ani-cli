#!/bin/sh
# Architectural invariant: the upstream `ani-cli` script is POSIX sh and
# must never use awk (per upstream's own CI gate `check-no-awk`). Our
# carried changes (just the __ANI_CLI_LIB__ source-guard) must respect
# that constraint.
#
# This script is a thin mirror of upstream's `! grep awk "./ani-cli"`
# check in .github/workflows/ani-cli.yml — duplicated locally so
# `bash tests/arch/run-all.sh` catches a regression before CI does.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -f ani-cli ]; then
    printf 'arch/bash_portability: ani-cli script not present — skipping\n'
    exit 0
fi

failed=0

# 1. No awk in the script.
if grep -q '\bawk\b' ani-cli; then
    matches=$(grep -n '\bawk\b' ani-cli)
    printf 'arch/bash_portability FAIL: ani-cli contains awk:\n%s\n' "$matches" >&2
    failed=1
fi

# 2. Shebang must be /bin/sh.
first_line=$(head -n1 ani-cli)
if [ "$first_line" != "#!/bin/sh" ]; then
    printf 'arch/bash_portability FAIL: ani-cli shebang is %s (expected #!/bin/sh)\n' "$first_line" >&2
    failed=1
fi

# 3. Diff against upstream master should differ by no more than one line
# (the carried __ANI_CLI_LIB__ guard). Skip if no upstream remote is
# configured.
if git remote get-url upstream >/dev/null 2>&1; then
    if git fetch upstream master --quiet 2>/dev/null; then
        diff_lines=$(git diff upstream/master -- ani-cli | grep -cE '^[+-][^+-]' || true)
        # Each carried line shows as one + in the diff. Allow up to 4
        # lines (the guard + a comment + spacing) — anything more is
        # suspicious.
        if [ "$diff_lines" -gt 4 ]; then
            printf 'arch/bash_portability FAIL: ani-cli diverges from upstream by %d lines (max 4)\n' "$diff_lines" >&2
            failed=1
        fi
    fi
fi

if [ "$failed" -eq 0 ]; then
    printf 'arch/bash_portability PASS\n'
fi
exit "$failed"
