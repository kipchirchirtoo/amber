# Builds the Flutter web app pointing at your Render server and publishes it on Vercel.
#
#   .\deploy\vercel\deploy.ps1 -ApiUrl https://amber-server.onrender.com
#
# First run: Vercel asks you to log in (a browser opens) and to create/link a
# project. Later runs update the same project. Needs Node.js (for npx).
param([Parameter(Mandatory = $true)][string]$ApiUrl)

$ErrorActionPreference = 'Stop'
$ApiUrl = $ApiUrl.TrimEnd('/')
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$site = Join-Path $PSScriptRoot 'site'

Write-Host "Building the web app for $ApiUrl ..."
Push-Location (Join-Path $root 'app')
try {
    # flutter prints progress notes to stderr; only its exit code decides success
    $ErrorActionPreference = 'Continue'
    flutter build web --release --dart-define=API_URL=$ApiUrl 2>&1 | ForEach-Object { "$_" }
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($code -ne 0) { throw 'flutter build web failed' }
} finally { Pop-Location }

# Copy into a fixed folder so Vercel's project link (.vercel) survives rebuilds.
New-Item -ItemType Directory -Force $site | Out-Null
robocopy (Join-Path $root 'app\build\web') $site /MIR /XD .vercel /XF vercel.json /NFL /NDL /NJH /NJS | Out-Null
if ($LASTEXITCODE -ge 8) { throw 'copying the build failed' }
Copy-Item (Join-Path $PSScriptRoot 'vercel.json') $site -Force

Push-Location $site
try {
    npx --yes vercel@latest deploy --prod
} finally { Pop-Location }
