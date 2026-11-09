# Starts the Amber server against the Supabase database.
#
#   cd C:\Users\user\OneDrive\Desktop\amber
#   .\run-server.ps1
#
# It asks for the database password each time and never writes it to disk.
# (To skip the question, set the AMBER_DB_PASSWORD environment variable yourself.)
#
# One-off setup: download Supabase's SSL certificate (Supabase dashboard ->
# Project Settings -> Database -> SSL Configuration -> "Download certificate")
# and save it next to this script as  supabase-ca.crt . The certificate is public,
# not a secret. It lets the server check it is really talking to your Supabase
# database and not an impostor.
#
# First time only, to create the first administrator:
#   $env:AMBER_BOOTSTRAP_ADMIN = "yourusername:a-long-password"; .\run-server.ps1
# (ignored once any user exists)

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$ca = Join-Path $PSScriptRoot 'supabase-ca.crt'
if (-not (Test-Path $ca)) {
    throw "Missing supabase-ca.crt next to this script. Download it from the Supabase dashboard (Project Settings -> Database -> SSL Configuration)."
}

# Settings from the .env file next to this script (KEY=value lines; blank values are ignored).
$envFile = Join-Path $PSScriptRoot '.env'
if (Test-Path $envFile) {
    foreach ($line in Get-Content $envFile) {
        if ($line -match '^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+?)\s*$' -and -not $line.TrimStart().StartsWith('#')) {
            Set-Item -Path "Env:$($Matches[1])" -Value $Matches[2]
        }
    }
}

if (-not $env:AMBER_DB_PASSWORD) {
    $secure = Read-Host 'Supabase database password' -AsSecureString
    $env:AMBER_DB_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))
}

$password = [uri]::EscapeDataString($env:AMBER_DB_PASSWORD)
$caUrl = [uri]::EscapeDataString(($ca -replace '\\', '/'))

# "Session" pooler (port 5432): the right choice for a server that stays running.
# If the certificate check complains about the host name, change verify-full to verify-ca.
$env:DATABASE_URL = "postgres://postgres.yqmlergkirervkoyefdy:$password@aws-0-eu-west-2.pooler.supabase.com:5432/postgres?sslmode=verify-full&sslrootcert=$caUrl"

Remove-Item Env:AMBER_DEV_MEMORY -ErrorAction SilentlyContinue
cargo run -p amber_server
