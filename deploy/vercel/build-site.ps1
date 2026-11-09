# Builds the Flutter web app for your Render server into the repository's
# vercel-site folder, so a Vercel project connected to GitHub can serve it
# (Vercel > Settings > General > Root Directory = vercel-site, Framework = Other).
#
#   .\deploy\vercel\build-site.ps1 -ApiUrl https://amber-gv4h.onrender.com
#   git add vercel-site; git commit -m "Update web app"; git push
#
# Vercel does not build Flutter itself, so the built files are committed.

param([Parameter(Mandatory = $true)][string]$ApiUrl)

$ErrorActionPreference = 'Stop'
$ApiUrl = $ApiUrl.TrimEnd('/')
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$site = Join-Path $root 'vercel-site'

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

New-Item -ItemType Directory -Force $site | Out-Null
robocopy (Join-Path $root 'app\build\web') $site /MIR /XF vercel.json /NFL /NDL /NJH /NJS | Out-Null
if ($LASTEXITCODE -ge 8) { throw 'copying the build failed' }
Copy-Item (Join-Path $PSScriptRoot 'vercel.json') $site -Force
Write-Host "Done: $site"
