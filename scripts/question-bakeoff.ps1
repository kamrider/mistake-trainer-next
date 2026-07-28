[CmdletBinding(PositionalBinding = $false)]
param(
    [switch]$InstallDependencies,
    [switch]$SelfCheck,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$LabArguments
)

$ErrorActionPreference = 'Stop'

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$labRoot = Join-Path $repositoryRoot 'labs\question-region-bakeoff'
$dependencyRoot = Join-Path $repositoryRoot '.tools\question-bakeoff-python'
$requirements = Join-Path $labRoot 'requirements.txt'

function Resolve-QuestionBakeoffPython {
    if ($env:QUESTION_BAKEOFF_PYTHON) {
        if (-not (Test-Path -LiteralPath $env:QUESTION_BAKEOFF_PYTHON -PathType Leaf)) {
            throw 'QUESTION_BAKEOFF_PYTHON does not point to a Python executable.'
        }
        return [pscustomobject]@{ Command = $env:QUESTION_BAKEOFF_PYTHON; Prefix = @() }
    }

    $launcher = Get-Command py.exe -ErrorAction SilentlyContinue
    if ($launcher) {
        return [pscustomobject]@{ Command = $launcher.Source; Prefix = @('-3.12') }
    }

    $python = Get-Command python.exe -ErrorAction SilentlyContinue
    if ($python) {
        return [pscustomobject]@{ Command = $python.Source; Prefix = @() }
    }

    throw 'Python 3.12 was not found. Install it or set QUESTION_BAKEOFF_PYTHON to python.exe.'
}

$runtime = Resolve-QuestionBakeoffPython

if ($InstallDependencies) {
    New-Item -ItemType Directory -Force -Path $dependencyRoot | Out-Null
    & $runtime.Command @($runtime.Prefix) -m pip install --disable-pip-version-check --upgrade --target $dependencyRoot -r $requirements
    if ($LASTEXITCODE -ne 0) {
        throw "Question bake-off dependencies failed to install (exit $LASTEXITCODE)."
    }
}

$requiresInferenceRuntime = $SelfCheck -or (
    $LabArguments -and
    $LabArguments.Count -gt 0 -and
    $LabArguments[0] -eq 'run'
)
if ($requiresInferenceRuntime) {
    $requiredModules = @('cv2', 'numpy', 'PIL', 'rapidocr', 'onnxruntime')
    foreach ($module in $requiredModules) {
        if (-not (Test-Path -LiteralPath (Join-Path $dependencyRoot $module))) {
            throw 'The isolated question bake-off runtime is incomplete. Run this script once with -InstallDependencies.'
        }
    }
}

$pathSeparator = [IO.Path]::PathSeparator
$pythonPaths = @($labRoot, $dependencyRoot)
if ($env:PYTHONPATH) {
    $pythonPaths += $env:PYTHONPATH
}
$env:PYTHONPATH = $pythonPaths -join $pathSeparator

if ($SelfCheck) {
    & $runtime.Command @($runtime.Prefix) -m question_bakeoff.cli self-check
    exit $LASTEXITCODE
}

if (-not $LabArguments -or $LabArguments.Count -eq 0) {
    throw 'Pass annotate <data>, validate <manifest>, or run <manifest> --output <directory>. Use -SelfCheck to inspect the lab runtime.'
}

& $runtime.Command @($runtime.Prefix) -m question_bakeoff.cli @LabArguments
exit $LASTEXITCODE
