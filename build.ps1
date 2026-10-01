param(
    [string]$RbeSha = $env:RBE_SHA,
    [switch]$NoRbeRefresh,
    [switch]$NoSdkUpdate,
    [switch]$CheckOnly,
    [switch]$AllowHostToolchain,
    [switch]$Clean,
    [string]$SdkVersion = $(if ($env:RBE_SDK_VERSION) { $env:RBE_SDK_VERSION } else { 'latest' })
)

$ErrorActionPreference = 'Stop'
$RepoRoot = $PSScriptRoot
$RbeRepository = if ($env:RBE_REPOSITORY) { $env:RBE_REPOSITORY } else { 'https://github.com/Kate-alt-69/RBE.git' }
$RbeApi = if ($env:RBE_API) { $env:RBE_API.TrimEnd('/') } else { 'https://api.github.com/repos/Kate-alt-69/RBE' }
$RbeSource = Join-Path $RepoRoot '.cache\rbe\upstream'
$ManifestPath = Join-Path $RepoRoot 'package.rbe.toml'

if (-not (Test-Path $ManifestPath)) {
    throw "package.rbe.toml is missing from $RepoRoot"
}
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    throw 'git is required to build mail.'
}

function Invoke-GitHubJson([string]$Uri) {
    $headers = @{
        Accept = 'application/vnd.github+json'
        'User-Agent' = 'rbe-mail-build'
    }
    if ($env:GITHUB_TOKEN) {
        $headers.Authorization = "Bearer $($env:GITHUB_TOKEN)"
    }
    return Invoke-RestMethod -Uri $Uri -Headers $headers -Method Get
}

function Resolve-LatestGreenRbe {
    Write-Host 'Resolving latest green RBE main CI...' -ForegroundColor Cyan
    $response = Invoke-GitHubJson "$RbeApi/actions/runs?branch=main&status=success&event=push&per_page=20"
    $run = $response.workflow_runs |
        Where-Object {
            $_.name -eq 'CI' -and
            $_.head_branch -eq 'main' -and
            $_.event -eq 'push' -and
            $_.status -eq 'completed' -and
            $_.conclusion -eq 'success'
        } |
        Select-Object -First 1

    if (-not $run) {
        throw 'No successful RBE main CI run was found.'
    }
    return [string]$run.head_sha
}

$RbeParent = Split-Path -Parent $RbeSource
New-Item -ItemType Directory -Force -Path $RbeParent | Out-Null

if (-not (Test-Path (Join-Path $RbeSource '.git'))) {
    Write-Host 'Cloning RBE from GitHub...' -ForegroundColor Cyan
    git clone $RbeRepository $RbeSource
    if ($LASTEXITCODE -ne 0) { throw 'Failed to clone RBE.' }
}

if (-not $NoRbeRefresh) {
    Write-Host 'Fetching RBE main...' -ForegroundColor Cyan
    git -C $RbeSource fetch --prune origin main
    if ($LASTEXITCODE -ne 0) { throw 'Failed to fetch RBE main.' }

    if ([string]::IsNullOrWhiteSpace($RbeSha)) {
        $RbeSha = Resolve-LatestGreenRbe
    }
}
elseif ([string]::IsNullOrWhiteSpace($RbeSha)) {
    $RbeSha = (git -C $RbeSource rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not resolve cached RBE HEAD.' }
}

if ($RbeSha -notmatch '^[0-9a-fA-F]{40}$') {
    throw "Invalid RBE commit SHA: $RbeSha"
}

git -C $RbeSource cat-file -e "$RbeSha`^{commit}" 2>$null
if ($LASTEXITCODE -ne 0) {
    git -C $RbeSource fetch origin $RbeSha
    if ($LASTEXITCODE -ne 0) { throw "Could not fetch RBE commit $RbeSha." }
}

git -C $RbeSource checkout --detach $RbeSha
if ($LASTEXITCODE -ne 0) { throw "Failed to check out RBE commit $RbeSha." }

git -C $RbeSource reset --hard $RbeSha
if ($LASTEXITCODE -ne 0) { throw 'Failed to reset the RBE checkout.' }

Write-Host "Selected RBE green commit: $RbeSha" -ForegroundColor Green

Write-Host 'Building RBE backend only...' -ForegroundColor Cyan
Push-Location $RbeSource
try {
    & .\build.ps1 --only-backend --build-win --arch-x64
    if ($LASTEXITCODE -ne 0) {
        throw "RBE backend build failed with exit code $LASTEXITCODE."
    }
}
finally {
    Pop-Location
}

$RbeBackend = Join-Path $RbeSource 'dist\x86_64-pc-windows-msvc\backend.exe'
if (-not (Test-Path $RbeBackend)) {
    throw "Freshly-built RBE backend was not found: $RbeBackend"
}

$SdkBackend = Join-Path $RepoRoot '.rbe\bin\backend.exe'
$Rpx = Join-Path $RepoRoot '.rbe\bin\rpx.exe'

if (-not $NoSdkUpdate) {
    Write-Host "Installing/updating verified RBE Rust SDK: sdk.$SdkVersion" -ForegroundColor Cyan

    # IMPORTANT: backend install currently expects option/value as separate
    # argv tokens. Do NOT use "-path=<value>" here.
    & $RbeBackend install "sdk.$SdkVersion" '-path' $RepoRoot '-language' 'rust'
    if ($LASTEXITCODE -ne 0) {
        throw "RBE SDK install/update failed with exit code $LASTEXITCODE."
    }
}

if (-not (Test-Path $SdkBackend)) {
    throw "Project-local SDK backend is missing: $SdkBackend"
}
if (-not (Test-Path $Rpx)) {
    throw "Project-local RPX is missing: $Rpx"
}

Write-Host 'RBE SDK status:' -ForegroundColor DarkGray
& $SdkBackend sdk status '-path' $RepoRoot
if ($LASTEXITCODE -ne 0) {
    throw "RBE SDK status failed with exit code $LASTEXITCODE."
}

if ($Clean) {
    $buildCache = Join-Path $RepoRoot '.cache\rbe\build'
    if (Test-Path $buildCache) {
        Remove-Item $buildCache -Recurse -Force
    }
}

$RpxArgs = @()
if ($AllowHostToolchain) {
    $RpxArgs += '--allow-host-toolchain'
}

Push-Location $RepoRoot
try {
    Write-Host ''
    Write-Host '==> rpx check' -ForegroundColor Cyan
    & $Rpx check .
    if ($LASTEXITCODE -ne 0) { throw "rpx check failed with exit code $LASTEXITCODE." }

    if ($CheckOnly) {
        Write-Host ''
        Write-Host 'mail package validation passed.' -ForegroundColor Green
        return
    }

    Write-Host ''
    Write-Host '==> rpx compile' -ForegroundColor Cyan
    & $Rpx compile . @RpxArgs
    if ($LASTEXITCODE -ne 0) { throw "rpx compile failed with exit code $LASTEXITCODE." }

    Write-Host ''
    Write-Host '==> rpx compile.package' -ForegroundColor Cyan
    & $Rpx compile.package . @RpxArgs
    if ($LASTEXITCODE -ne 0) { throw "rpx compile.package failed with exit code $LASTEXITCODE." }

    Write-Host ''
    Write-Host 'mail build complete.' -ForegroundColor Green
    Write-Host "RBE source commit: $RbeSha" -ForegroundColor DarkGray
}
finally {
    Pop-Location
}
