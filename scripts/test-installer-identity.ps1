# Compile the real installer identity guard with in-memory registry fixtures.
# The temporary test executable never accesses the registry or runs SwitchPlus.
param([string]$NsisRoot = "$env:LOCALAPPDATA\tauri\NSIS")
$ErrorActionPreference = 'Stop'
$taskRepo = Split-Path $PSScriptRoot -Parent
$taskTemplate = Get-Content (Join-Path $taskRepo 'src-tauri\nsis\installer.nsi') -Raw -Encoding UTF8
$taskConfig = Get-Content (Join-Path $taskRepo 'src-tauri\tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$taskGuard = [regex]::Match($taskTemplate, '(?ms)^Function RequireCurrentInstallIdentity\r?\n.*?^FunctionEnd').Value
if (-not $taskGuard) { throw 'Installer identity guard is missing' }
$taskGuard = $taskGuard.Replace('ReadRegStr $R0 SHCTX "${UNINSTKEY}" "UninstallString"', 'StrCpy $R0 $TestUninstaller')
$taskGuard = $taskGuard.Replace('ReadRegStr $R0 SHCTX "${MANUPRODUCTKEY}" "AppIdentifier"', 'StrCpy $R0 $TestIdentity')
if ($taskGuard -match 'ReadReg|WriteReg|DeleteReg') { throw 'Test guard still accesses the registry' }
$taskHeader = @'
Unicode true
RequestExecutionLevel user
SilentInstall silent
!include LogicLib.nsh
!include FileFunc.nsh
Var TestUninstaller
Var TestIdentity
Var TestWix
Function FindWixInstall
  StrCpy $R6 $TestWix
FunctionEnd
'@
$taskSection = @'
Section
  ${GetOptions} $CMDLINE "/installed=" $TestUninstaller
  ${GetOptions} $CMDLINE "/identity=" $TestIdentity
  ${GetOptions} $CMDLINE "/msi=" $TestWix
  Call RequireCurrentInstallIdentity
  SetErrorLevel 0
SectionEnd
'@
$taskDir = Join-Path ([IO.Path]::GetTempPath()) ('switchplus-identity-test-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $taskDir | Out-Null
try {
    $taskExe = Join-Path $taskDir 'identity-test.exe'
    $taskScript = Join-Path $taskDir 'identity-test.nsi'
    $taskProgram = $taskHeader + "`n!define BUNDLEID `"$($taskConfig.identifier)`"`nOutFile `"$taskExe`"`n" + $taskGuard + "`n" + $taskSection
    [IO.File]::WriteAllText($taskScript, $taskProgram, [Text.UTF8Encoding]::new($true))
    & (Join-Path $NsisRoot 'makensis.exe') /V2 $taskScript
    if ($LASTEXITCODE -ne 0) { throw 'Installer identity test did not compile' }
    $taskCases = @(
        @{ Name = 'fresh install'; Args = '/S'; Expected = 0 },
        @{ Name = 'old NSIS without marker'; Args = '/S /installed=yes'; Expected = 2 },
        @{ Name = 'old NSIS identifier'; Args = '/S /installed=yes /identity=io.github.saber570.switchplus'; Expected = 2 },
        @{ Name = 'old MSI without marker'; Args = '/S /msi=yes'; Expected = 2 },
        @{ Name = 'current NSIS upgrade'; Args = '/S /installed=yes /identity=com.chow.switchplus'; Expected = 0 },
        @{ Name = 'current MSI transition'; Args = '/S /msi=yes /identity=com.chow.switchplus'; Expected = 0 },
        @{ Name = 'leftover marker after uninstall'; Args = '/S /identity=com.chow.switchplus'; Expected = 0 }
    )
    foreach ($taskCase in $taskCases) {
        $taskProcess = Start-Process -FilePath $taskExe -ArgumentList $taskCase.Args -WindowStyle Hidden -Wait -PassThru
        if ($taskProcess.ExitCode -ne $taskCase.Expected) {
            throw "$($taskCase.Name): expected $($taskCase.Expected), got $($taskProcess.ExitCode)"
        }
    }
    Write-Output "Passed $($taskCases.Count) installer identity cases with in-memory fixtures."
} finally {
    $taskResolved = [IO.Path]::GetFullPath($taskDir)
    $taskTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if ($taskResolved.StartsWith($taskTemp, [StringComparison]::OrdinalIgnoreCase) -and (Split-Path $taskResolved -Leaf) -like 'switchplus-identity-test-*') {
        Remove-Item -LiteralPath $taskResolved -Recurse -Force
    }
}
