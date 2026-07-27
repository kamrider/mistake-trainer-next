[CmdletBinding()]
param(
    [int]$Port = 4179
)

$ErrorActionPreference = 'Stop'

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$vite = Join-Path $repositoryRoot 'node_modules\.bin\vite.cmd'
$labRoot = Join-Path $repositoryRoot 'labs\capture-workbench-perf'

if (-not (Test-Path -LiteralPath $vite -PathType Leaf)) {
    throw 'Install the repository dependencies before starting the performance lab.'
}

Write-Output "Capture performance lab: http://127.0.0.1:$Port/"
& $vite $labRoot --host 127.0.0.1 --port $Port --strictPort
exit $LASTEXITCODE
