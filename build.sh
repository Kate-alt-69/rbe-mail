#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RBE_REPOSITORY="${RBE_REPOSITORY:-https://github.com/Kate-alt-69/RBE.git}"
RBE_API="${RBE_API:-https://api.github.com/repos/Kate-alt-69/RBE}"
RBE_SOURCE="$REPO_ROOT/.cache/rbe/upstream"
SDK_VERSION="${RBE_SDK_VERSION:-latest}"
RBE_SHA="${RBE_SHA:-}"
NO_RBE_REFRESH=false
NO_SDK_UPDATE=false
CHECK_ONLY=false
ALLOW_HOST_TOOLCHAIN=false
MANAGED_TOOLCHAIN_ONLY=false
CLEAN=false

mail_die() {
  local code="$1"; shift
  local message="$1"; shift || true
  echo "$code: $message" >&2
  if [ "$#" -gt 0 ]; then echo "Fix: $*" >&2; fi
  exit 1
}

usage() {
  cat <<'EOF'
mail build helper

Usage:
  ./build.sh [options]

Default flow:
  1. Resolve the latest successful RBE main CI commit from GitHub.
  2. Clone/update Kate-alt-69/RBE under .cache/rbe/upstream.
  3. Build ONLY the RBE backend from that green commit.
  4. Use that freshly-built backend to install/update the project-local Rust SDK.
  5. Prefer the RBE-managed RPX compiler map; if absent, use host Cargo only for local authoring.
  6. Run rpx check, compile, and compile.package for mail.

Options:
  --rbe-sha <sha>          Use an explicit RBE commit instead of latest green CI.
  --no-rbe-refresh         Reuse the cached RBE checkout/commit without GitHub CI lookup.
  --no-sdk-update          Do not reinstall/update the project-local SDK.
  --check-only             Run RPX validation only.
  --allow-host-toolchain   Explicitly force RPX host Rust tools for local authoring.
  --managed-toolchain-only Require .rbe/rpx-toolchain.json; never fall back to host Cargo.
  --clean                  Remove package build cache before building.
  --sdk-version <version>  Install sdk.<version> instead of sdk.latest.
  -h, --help               Show this help.

Environment equivalents:
  RBE_SHA
  RBE_SDK_VERSION
  RBE_REPOSITORY
  RBE_API
  GITHUB_TOKEN             Optional; increases GitHub API rate limit.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --rbe-sha)
      shift
      [ "$#" -gt 0 ] || { echo "ERROR: --rbe-sha requires a SHA." >&2; exit 2; }
      RBE_SHA="$1"
      ;;
    --rbe-sha=*) RBE_SHA="${1#*=}" ;;
    --no-rbe-refresh) NO_RBE_REFRESH=true ;;
    --no-sdk-update) NO_SDK_UPDATE=true ;;
    --check-only) CHECK_ONLY=true ;;
    --allow-host-toolchain) ALLOW_HOST_TOOLCHAIN=true ;;
    --managed-toolchain-only) MANAGED_TOOLCHAIN_ONLY=true ;;
    --clean) CLEAN=true ;;
    --sdk-version)
      shift
      [ "$#" -gt 0 ] || { echo "ERROR: --sdk-version requires a value." >&2; exit 2; }
      SDK_VERSION="$1"
      ;;
    --sdk-version=*) SDK_VERSION="${1#*=}" ;;
    -h|--help) usage; exit 0 ;;
    *) echo "ERROR: unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

if $ALLOW_HOST_TOOLCHAIN && $MANAGED_TOOLCHAIN_ONLY; then
  mail_die MAIL5001 "--allow-host-toolchain and --managed-toolchain-only cannot be used together" "Choose explicit host authoring or strict managed-toolchain mode, not both."
fi

command -v git >/dev/null 2>&1 || mail_die MAIL5001 "git is required to fetch/build RBE" "Install Git and ensure git is on PATH."
command -v python3 >/dev/null 2>&1 || mail_die MAIL5001 "python3 is required to resolve the latest green RBE CI run" "Install Python 3 and ensure python3 is on PATH."
[ -f "$REPO_ROOT/package.rbe.toml" ] || mail_die MAIL5001 "package.rbe.toml is missing from the repository root" "Run build.sh from the rbe-mail repository and restore package.rbe.toml."

github_json() {
  local url="$1"
  python3 - "$url" <<'PY'
import json, os, sys, urllib.request
url = sys.argv[1]
headers = {
    "Accept": "application/vnd.github+json",
    "User-Agent": "rbe-mail-build",
}
token = os.environ.get("GITHUB_TOKEN")
if token:
    headers["Authorization"] = f"Bearer {token}"
req = urllib.request.Request(url, headers=headers)
with urllib.request.urlopen(req, timeout=30) as response:
    sys.stdout.write(response.read().decode("utf-8"))
PY
}

resolve_latest_green() {
  local payload
  if ! payload="$(github_json "$RBE_API/actions/runs?branch=main&status=success&event=push&per_page=20" 2>&1)"; then
    mail_die MAIL5002 "failed to query GitHub Actions for a green RBE main build: $payload" "Check GitHub connectivity/API rate limits or set GITHUB_TOKEN; alternatively pass --rbe-sha <known-green-sha>."
  fi
  python3 -c '
import json, sys
data=json.load(sys.stdin)
for run in data.get("workflow_runs", []):
    if (run.get("name") == "CI"
        and run.get("head_branch") == "main"
        and run.get("event") == "push"
        and run.get("status") == "completed"
        and run.get("conclusion") == "success"):
        print(run["head_sha"])
        raise SystemExit(0)
raise SystemExit("no successful RBE main CI run was found in the last 20 successful push runs")
' <<<"$payload"
}

mkdir -p "$(dirname "$RBE_SOURCE")"

if [ ! -d "$RBE_SOURCE/.git" ]; then
  echo "Cloning RBE from GitHub..."
  git clone "$RBE_REPOSITORY" "$RBE_SOURCE" || mail_die MAIL5003 "failed to clone RBE from $RBE_REPOSITORY" "Check Git/GitHub access and delete a partial .cache/rbe/upstream before retrying."
fi

if ! $NO_RBE_REFRESH; then
  echo "Fetching RBE main..."
  git -C "$RBE_SOURCE" fetch --prune origin main || mail_die MAIL5003 "failed to fetch RBE origin/main" "Check GitHub connectivity and the cached checkout under .cache/rbe/upstream."

  if [ -z "$RBE_SHA" ]; then
    echo "Resolving latest green RBE main CI..."
    if ! RBE_SHA="$(resolve_latest_green 2>&1)"; then
      mail_die MAIL5002 "could not resolve a green RBE main commit: $RBE_SHA" "Check GitHub Actions/API access or pass --rbe-sha <known-green-sha>."
    fi
  fi
elif [ -z "$RBE_SHA" ]; then
  RBE_SHA="$(git -C "$RBE_SOURCE" rev-parse HEAD)"
fi

[[ "$RBE_SHA" =~ ^[0-9a-fA-F]{40}$ ]] || {
  mail_die MAIL5003 "invalid RBE commit SHA: $RBE_SHA" "Use a full 40-character commit SHA or let the helper resolve the latest green CI commit."
}

git -C "$RBE_SOURCE" cat-file -e "$RBE_SHA^{commit}" 2>/dev/null || {
  git -C "$RBE_SOURCE" fetch origin "$RBE_SHA" || mail_die MAIL5003 "could not fetch RBE commit $RBE_SHA" "Confirm the SHA exists in Kate-alt-69/RBE and that GitHub is reachable."
}
git -C "$RBE_SOURCE" checkout --detach "$RBE_SHA" || mail_die MAIL5003 "failed to check out RBE commit $RBE_SHA" "Delete/refresh .cache/rbe/upstream and retry."
git -C "$RBE_SOURCE" reset --hard "$RBE_SHA" || mail_die MAIL5003 "failed to reset cached RBE checkout to $RBE_SHA" "Delete .cache/rbe/upstream and retry."

echo "Selected RBE green commit: $RBE_SHA"

case "$(uname -s)" in
  Linux*)
    RBE_TARGET="x86_64-unknown-linux-gnu"
    echo "Building RBE backend only..."
    (
      cd "$RBE_SOURCE"
      ./build.sh --only-backend --build-linux --arch-x64
    ) || mail_die MAIL5003 "RBE backend build failed at $RBE_SHA" "Read the RBE compiler output above; install missing build prerequisites or select another green SHA if the local toolchain is incompatible."
    RBE_BACKEND="$RBE_SOURCE/dist/$RBE_TARGET/backend"
    ;;
  Darwin*)
    echo "ERROR: RBE's current selective build helper does not define a macOS backend target in this package script yet." >&2
    exit 1
    ;;
  *)
    echo "ERROR: unsupported shell platform. On Windows use build.ps1." >&2
    exit 1
    ;;
esac

[ -x "$RBE_BACKEND" ] || {
  mail_die MAIL5003 "freshly-built RBE backend was not found at $RBE_BACKEND" "Inspect the selected RBE build output/target and report the selected SHA if dist layout changed."
}

SDK_BACKEND="$REPO_ROOT/.rbe/bin/backend"
RPX="$REPO_ROOT/.rbe/bin/rpx"

if ! $NO_SDK_UPDATE; then
  echo "Installing/updating verified RBE Rust SDK: sdk.$SDK_VERSION"
  "$RBE_BACKEND" install "sdk.$SDK_VERSION" -path "$REPO_ROOT" -language rust || mail_die MAIL5004 "RBE SDK install/update failed" "Run '$RBE_BACKEND install help' if CLI syntax changed; otherwise inspect the installer error above and retry."
fi

[ -x "$SDK_BACKEND" ] || mail_die MAIL5004 "project-local SDK backend is missing: $SDK_BACKEND" "Re-run without --no-sdk-update so the verified Rust SDK is installed."
[ -x "$RPX" ] || mail_die MAIL5004 "project-local RPX is missing: $RPX" "Reinstall/repair the project-local RBE SDK."

echo "RBE SDK status:"
"$SDK_BACKEND" sdk status -path "$REPO_ROOT" || mail_die MAIL5004 "RBE SDK status validation failed" "Repair/reinstall the project-local SDK and verify its managed Rust toolchain."

if $CLEAN; then
  rm -rf -- "$REPO_ROOT/.cache/rbe/build"
fi

MANAGED_TOOLCHAIN_MAP="$REPO_ROOT/.rbe/rpx-toolchain.json"
RPX_ARGS=()

if $ALLOW_HOST_TOOLCHAIN; then
  command -v cargo >/dev/null 2>&1 || mail_die MAIL5005 "explicit host-toolchain authoring was requested, but cargo was not found on PATH" "Install Rust/Cargo or remove --allow-host-toolchain and configure .rbe/rpx-toolchain.json."
  echo "WARNING: RPX local authoring: explicitly using host Cargo at $(command -v cargo)." >&2
  RPX_ARGS+=(--allow-host-toolchain)
elif [ -f "$MANAGED_TOOLCHAIN_MAP" ]; then
  echo "RPX compiler authority: managed toolchain ($MANAGED_TOOLCHAIN_MAP)"
elif $MANAGED_TOOLCHAIN_ONLY; then
  mail_die MAIL5005 "no RBE-managed RPX toolchain is configured for this project" "Hydrate/write .rbe/rpx-toolchain.json with pinned compiler SHA-256 identities, or rerun without --managed-toolchain-only for local authoring fallback."
elif command -v cargo >/dev/null 2>&1; then
  echo "WARNING: No RBE-managed RPX toolchain is configured; local package authoring will use host Cargo at $(command -v cargo) via RPX --allow-host-toolchain. Use --managed-toolchain-only to forbid this fallback." >&2
  RPX_ARGS+=(--allow-host-toolchain)
else
  mail_die MAIL5005 "no RBE-managed RPX toolchain is configured and cargo was not found on PATH" "Install Rust/Cargo for local authoring, or hydrate .rbe/rpx-toolchain.json with a pinned managed toolchain."
fi

cd "$REPO_ROOT"

echo
echo "==> rpx check"
"$RPX" check . || mail_die MAIL5005 "rpx check failed for package mail" "Fix the package/component error printed above; use the MAIL/RPX/RBE code in that output for the next diagnostic."

if $CHECK_ONLY; then
  echo
  echo "mail package validation passed."
  exit 0
fi

echo
echo "==> rpx compile"
"$RPX" compile . "${RPX_ARGS[@]}" || mail_die MAIL5005 "rpx compile failed for package mail" "Fix the first Rust/RBE compiler diagnostic above. The helper already prefers managed compiler authority and uses host Cargo only for local authoring fallback."

echo
echo "==> rpx compile.package"
"$RPX" compile.package . "${RPX_ARGS[@]}" || mail_die MAIL5005 "rpx compile.package failed for package mail" "Fix the compiler/archive error above before publishing the package."

echo
echo "mail build complete."
echo "RBE source commit: $RBE_SHA"
