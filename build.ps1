#Requires -Version 5.1
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

function Throw-MailBuildError([string]$Code, [string]$Message, [string]$Fix) {
    throw "${Code}: $Message`nFix: $Fix"
}
$RepoRoot = $PSScriptRoot
$RbeRepository = if ($env:RBE_REPOSITORY) { $env:RBE_REPOSITORY } else { 'https://github.com/Kate-alt-69/RBE.git' }
$RbeApi = if ($env:RBE_API) { $env:RBE_API.TrimEnd('/') } else { 'https://api.github.com/repos/Kate-alt-69/RBE' }
$RbeSource = Join-Path $RepoRoot '.cache\rbe\upstream'
$ManifestPath = Join-Path $RepoRoot 'package.rbe.toml'

if (-not (Test-Path $ManifestPath)) {
    Throw-MailBuildError 'MAIL5001' "package.rbe.toml is missing from $RepoRoot" 'Run build.ps1 from the rbe-mail repository and restore package.rbe.toml.'
}
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Throw-MailBuildError 'MAIL5001' 'git is required to fetch/build RBE.' 'Install Git and ensure git.exe is on PATH.'
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
    try {
        $response = Invoke-GitHubJson "$RbeApi/actions/runs?branch=main&status=success&event=push&per_page=20"
    }
    catch {
        Throw-MailBuildError 'MAIL5002' "Failed to query GitHub Actions for a green RBE main build: $($_.Exception.Message)" 'Check GitHub connectivity/API rate limits, set GITHUB_TOKEN, or pass -RbeSha <known-green-sha>.'
    }
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
        Throw-MailBuildError 'MAIL5002' 'No successful RBE main CI run was found in the queried window.' 'Pass -RbeSha <known-green-sha> or verify the RBE CI workflow is completing successfully.'
    }
    return [string]$run.head_sha
}

$RbeParent = Split-Path -Parent $RbeSource
New-Item -ItemType Directory -Force -Path $RbeParent | Out-Null

if (-not (Test-Path (Join-Path $RbeSource '.git'))) {
    Write-Host 'Cloning RBE from GitHub...' -ForegroundColor Cyan
    git clone $RbeRepository $RbeSource
    if ($LASTEXITCODE -ne 0) { Throw-MailBuildError 'MAIL5003' 'Failed to clone RBE.' 'Check GitHub access and delete a partial .cache\rbe\upstream before retrying.' }
}

if (-not $NoRbeRefresh) {
    Write-Host 'Fetching RBE main...' -ForegroundColor Cyan
    git -C $RbeSource fetch --prune origin main
    if ($LASTEXITCODE -ne 0) { Throw-MailBuildError 'MAIL5003' 'Failed to fetch RBE origin/main.' 'Check GitHub connectivity and the cached RBE checkout.' }

    if ([string]::IsNullOrWhiteSpace($RbeSha)) {
        $RbeSha = Resolve-LatestGreenRbe
    }
}
elseif ([string]::IsNullOrWhiteSpace($RbeSha)) {
    $RbeSha = (git -C $RbeSource rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { Throw-MailBuildError 'MAIL5003' 'Could not resolve cached RBE HEAD.' 'Delete/refresh .cache\rbe\upstream or run without -NoRbeRefresh.' }
}

if ($RbeSha -notmatch '^[0-9a-fA-F]{40}$') {
    Throw-MailBuildError 'MAIL5003' "Invalid RBE commit SHA: $RbeSha" 'Use a full 40-character SHA or let the helper resolve the latest green RBE CI commit.'
}

git -C $RbeSource cat-file -e "$RbeSha`^{commit}" 2>$null
if ($LASTEXITCODE -ne 0) {
    git -C $RbeSource fetch origin $RbeSha
    if ($LASTEXITCODE -ne 0) { Throw-MailBuildError 'MAIL5003' "Could not fetch RBE commit $RbeSha." 'Confirm the SHA exists in Kate-alt-69/RBE and GitHub is reachable.' }
}

git -C $RbeSource checkout --detach $RbeSha
if ($LASTEXITCODE -ne 0) { Throw-MailBuildError 'MAIL5003' "Failed to check out RBE commit $RbeSha." 'Delete/refresh .cache\rbe\upstream and retry.' }

git -C $RbeSource reset --hard $RbeSha
if ($LASTEXITCODE -ne 0) { Throw-MailBuildError 'MAIL5003' 'Failed to reset the cached RBE checkout.' 'Delete .cache\rbe\upstream and retry.' }

Write-Host "Selected RBE green commit: $RbeSha" -ForegroundColor Green

Write-Host 'Building RBE backend only...' -ForegroundColor Cyan
Push-Location $RbeSource
try {
    & .\build.ps1 --only-backend --build-win --arch-x64
    if ($LASTEXITCODE -ne 0) {
        Throw-MailBuildError 'MAIL5003' "RBE backend build failed with exit code $LASTEXITCODE at $RbeSha." 'Read the RBE compiler output above; install missing build prerequisites or select another known-green SHA if the local toolchain is incompatible.'
    }
}
finally {
    Pop-Location
}

$RbeBackend = Join-Path $RbeSource 'dist\x86_64-pc-windows-msvc\backend.exe'
if (-not (Test-Path $RbeBackend)) {
    Throw-MailBuildError 'MAIL5003' "Freshly-built RBE backend was not found: $RbeBackend" 'Inspect the selected RBE dist target/layout and report the selected SHA if its output layout changed.'
}

$SdkBackend = Join-Path $RepoRoot '.rbe\bin\backend.exe'
$Rpx = Join-Path $RepoRoot '.rbe\bin\rpx.exe'

if (-not $NoSdkUpdate) {
    Write-Host "Installing/updating verified RBE Rust SDK: sdk.$SdkVersion" -ForegroundColor Cyan

    # IMPORTANT: backend install currently expects option/value as separate
    # argv tokens. Do NOT use "-path=<value>" here.
    & $RbeBackend install "sdk.$SdkVersion" '-path' $RepoRoot '-language' 'rust'
    if ($LASTEXITCODE -ne 0) {
        Throw-MailBuildError 'MAIL5004' "RBE SDK install/update failed with exit code $LASTEXITCODE." 'Inspect the installer output; run the freshly-built backend install help if the CLI contract changed.'
    }
}

if (-not (Test-Path $SdkBackend)) {
    Throw-MailBuildError 'MAIL5004' "Project-local SDK backend is missing: $SdkBackend" 'Re-run without -NoSdkUpdate so the verified Rust SDK is installed.'
}
if (-not (Test-Path $Rpx)) {
    Throw-MailBuildError 'MAIL5004' "Project-local RPX is missing: $Rpx" 'Reinstall/repair the project-local RBE SDK.'
}

Write-Host 'RBE SDK status:' -ForegroundColor DarkGray
& $SdkBackend sdk status '-path' $RepoRoot
if ($LASTEXITCODE -ne 0) {
    Throw-MailBuildError 'MAIL5004' "RBE SDK status failed with exit code $LASTEXITCODE." 'Repair/reinstall the project-local SDK and verify its managed Rust toolchain.'
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
    if ($LASTEXITCODE -ne 0) { Throw-MailBuildError 'MAIL5005' "rpx check failed with exit code $LASTEXITCODE." 'Fix the package/component error above and use its MAIL/RPX/RBE diagnostic code for the next step.' }

    if ($CheckOnly) {
        Write-Host ''
        Write-Host 'mail package validation passed.' -ForegroundColor Green
        return
    }

    Write-Host ''
    Write-Host '==> rpx compile' -ForegroundColor Cyan
    & $Rpx compile . @RpxArgs
    if ($LASTEXITCODE -ne 0) { Throw-MailBuildError 'MAIL5005' "rpx compile failed with exit code $LASTEXITCODE." 'Fix the Rust/RBE compiler error above; use -AllowHostToolchain only for deliberate local development when no managed toolchain exists.' }

    Write-Host ''
    Write-Host '==> rpx compile.package' -ForegroundColor Cyan
    & $Rpx compile.package . @RpxArgs
    if ($LASTEXITCODE -ne 0) { Throw-MailBuildError 'MAIL5005' "rpx compile.package failed with exit code $LASTEXITCODE." 'Fix the compiler/archive error before publishing mail.' }

    Write-Host ''
    Write-Host 'mail build complete.' -ForegroundColor Green
    Write-Host "RBE source commit: $RbeSha" -ForegroundColor DarkGray
}
finally {
    Pop-Location
}
