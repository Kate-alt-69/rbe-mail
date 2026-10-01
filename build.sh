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
CLEAN=false

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
  5. Run rpx check, compile, and compile.package for mail.

Options:
  --rbe-sha <sha>          Use an explicit RBE commit instead of latest green CI.
  --no-rbe-refresh         Reuse the cached RBE checkout/commit without GitHub CI lookup.
  --no-sdk-update          Do not reinstall/update the project-local SDK.
  --check-only             Run RPX validation only.
  --allow-host-toolchain   Allow RPX host Rust tools when no managed toolchain exists.
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

command -v git >/dev/null 2>&1 || { echo "ERROR: git is required." >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "ERROR: python3 is required." >&2; exit 1; }
[ -f "$REPO_ROOT/package.rbe.toml" ] || { echo "ERROR: package.rbe.toml is missing." >&2; exit 1; }

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
  payload="$(github_json "$RBE_API/actions/runs?branch=main&status=success&event=push&per_page=20")"
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
raise SystemExit("no successful RBE main CI run was found")
' <<<"$payload"
}

mkdir -p "$(dirname "$RBE_SOURCE")"

if [ ! -d "$RBE_SOURCE/.git" ]; then
  echo "Cloning RBE from GitHub..."
  git clone "$RBE_REPOSITORY" "$RBE_SOURCE"
fi

if ! $NO_RBE_REFRESH; then
  echo "Fetching RBE main..."
  git -C "$RBE_SOURCE" fetch --prune origin main

  if [ -z "$RBE_SHA" ]; then
    echo "Resolving latest green RBE main CI..."
    RBE_SHA="$(resolve_latest_green)"
  fi
elif [ -z "$RBE_SHA" ]; then
  RBE_SHA="$(git -C "$RBE_SOURCE" rev-parse HEAD)"
fi

[[ "$RBE_SHA" =~ ^[0-9a-fA-F]{40}$ ]] || {
  echo "ERROR: invalid RBE commit SHA: $RBE_SHA" >&2
  exit 1
}

git -C "$RBE_SOURCE" cat-file -e "$RBE_SHA^{commit}" 2>/dev/null || {
  git -C "$RBE_SOURCE" fetch origin "$RBE_SHA"
}
git -C "$RBE_SOURCE" checkout --detach "$RBE_SHA"
git -C "$RBE_SOURCE" reset --hard "$RBE_SHA"

echo "Selected RBE green commit: $RBE_SHA"

case "$(uname -s)" in
  Linux*)
    RBE_TARGET="x86_64-unknown-linux-gnu"
    echo "Building RBE backend only..."
    (
      cd "$RBE_SOURCE"
      ./build.sh --only-backend --build-linux --arch-x64
    )
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
  echo "ERROR: freshly-built RBE backend not found: $RBE_BACKEND" >&2
  exit 1
}

SDK_BACKEND="$REPO_ROOT/.rbe/bin/backend"
RPX="$REPO_ROOT/.rbe/bin/rpx"

if ! $NO_SDK_UPDATE; then
  echo "Installing/updating verified RBE Rust SDK: sdk.$SDK_VERSION"
  "$RBE_BACKEND" install "sdk.$SDK_VERSION" -path "$REPO_ROOT" -language rust
fi

[ -x "$SDK_BACKEND" ] || { echo "ERROR: project-local SDK backend is missing: $SDK_BACKEND" >&2; exit 1; }
[ -x "$RPX" ] || { echo "ERROR: project-local RPX is missing: $RPX" >&2; exit 1; }

echo "RBE SDK status:"
"$SDK_BACKEND" sdk status -path "$REPO_ROOT"

if $CLEAN; then
  rm -rf -- "$REPO_ROOT/.cache/rbe/build"
fi

RPX_ARGS=()
if $ALLOW_HOST_TOOLCHAIN; then
  RPX_ARGS+=(--allow-host-toolchain)
fi

cd "$REPO_ROOT"

echo
echo "==> rpx check"
"$RPX" check .

if $CHECK_ONLY; then
  echo
  echo "mail package validation passed."
  exit 0
fi

echo
echo "==> rpx compile"
"$RPX" compile . "${RPX_ARGS[@]}"

echo
echo "==> rpx compile.package"
"$RPX" compile.package . "${RPX_ARGS[@]}"

echo
echo "mail build complete."
echo "RBE source commit: $RBE_SHA"
