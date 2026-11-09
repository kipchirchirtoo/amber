# Makes render.env: the settings to paste into Render (Environment > "Add from .env").
# It reads your private .env and writes render.env next to it. render.env holds secrets:
# paste it into Render, then delete it. It is ignored by git.
#
#   .\deploy\make-render-env.ps1
#   .\deploy\make-render-env.ps1 -WebUrl https://amber-demo.vercel.app

param([string]$WebUrl = '')

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$envFile = Join-Path $root '.env'
if (-not (Test-Path $envFile)) { throw "No .env file in $root" }

$v = @{}
foreach ($line in Get-Content $envFile) {
    if ($line -match '^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+?)\s*$' -and -not $line.TrimStart().StartsWith('#')) { $v[$Matches[1]] = $Matches[2] }
}
if (-not $v['AMBER_DB_PASSWORD']) { throw 'AMBER_DB_PASSWORD is empty in .env' }

$pw = [uri]::EscapeDataString($v['AMBER_DB_PASSWORD'])
# Session pooler (port 5432) with full certificate checking; the certificate is inside the image.
$db = "postgres://postgres.yqmlergkirervkoyefdy:$pw@aws-0-eu-west-2.pooler.supabase.com:5432/postgres?sslmode=verify-full&sslrootcert=%2Fetc%2Famber%2Fsupabase-ca.crt"

$out = @("DATABASE_URL=$db", 'AMBER_DB_POOL=5')
if ($WebUrl) { $out += "AMBER_CORS_ORIGIN=$WebUrl" }
foreach ($k in 'OPENAI_API_KEY', 'GEMINI_API_KEY', 'GROQ_API_KEY', 'ANTHROPIC_API_KEY') {
    if ($v[$k]) { $out += "$k=$($v[$k])" }
}
Set-Content -Path (Join-Path $root 'render.env') -Value $out -Encoding ascii
Write-Host "Wrote render.env with: $((($out | ForEach-Object { ($_ -split '=')[0] }) -join ', '))"
if (-not $WebUrl) { Write-Host 'No web address yet: after the Vercel deploy, run this again with -WebUrl https://your-app.vercel.app and set AMBER_CORS_ORIGIN on Render.' }
Write-Host 'Paste it into Render, then delete the file.'
