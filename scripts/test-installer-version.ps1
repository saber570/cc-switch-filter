# Exercise the real NSIS version-policy macro without installing, changing the
# registry, or running SwitchPlus. Only a temporary test executable is created.
param([string]$NsisRoot = "$env:LOCALAPPDATA\tauri\NSIS")
$ErrorActionPreference = 'Stop'
$taskRepo = Split-Path $PSScriptRoot -Parent
$taskDir = Join-Path ([IO.Path]::GetTempPath()) ('switchplus-version-test-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $taskDir | Out-Null
$taskPolicy = Join-Path $taskRepo 'src-tauri\nsis\version-policy.nsh'
$taskExe = Join-Path $taskDir 'version-test.exe'
$taskScript = Join-Path $taskDir 'version-test.nsi'
$taskHeader = @'
Unicode true
RequestExecutionLevel user
SilentInstall silent
!include LogicLib.nsh
!define VERSION "1.0.0"
'@
$taskProgram = $taskHeader + "`n!addplugindir `"$NsisRoot\Plugins\x86-unicode\additional`"`n!include `"$taskPolicy`"`nOutFile `"$taskExe`"`nSection`n"
$taskCases = @(
    @('4.0.28', 1), @('4.0.29', 1), @('4.0.28.0', 1), @('4.0.29.0', 1),
    @('1.0.0', 0), @('1.1.0', -1), @('1.0.1', -1), @('0.9.0', 1),
    @('1.0.0-beta.1', 1), @('4.0.30', -1), @('4.0.29-beta.1', -1)
)
for ($taskIndex = 0; $taskIndex -lt $taskCases.Count; $taskIndex++) {
    $taskVersion, $taskExpected = $taskCases[$taskIndex]
    $taskProgram += 'StrCpy $R0 "' + $taskVersion + '"' + "`n!insertmacro SwitchPlusCompareInstalledVersion`n"
    $taskProgram += '${If} $R0 != ' + $taskExpected + "`nSetErrorLevel " + ($taskIndex + 1) + "`nQuit`n" + '${EndIf}' + "`n"
}
$taskProgram += "SetErrorLevel 0`nSectionEnd`n"
[IO.File]::WriteAllText($taskScript, $taskProgram, (New-Object Text.UTF8Encoding($false)))
try {
    & (Join-Path $NsisRoot 'makensis.exe') /V2 $taskScript
    if ($LASTEXITCODE -ne 0) { throw 'Version policy test did not compile' }
    $taskProcess = Start-Process -FilePath $taskExe -Wait -PassThru -WindowStyle Hidden
    if ($taskProcess.ExitCode -ne 0) { throw "Version policy case $($taskProcess.ExitCode) failed" }
    Write-Output "Passed $($taskCases.Count) NSIS version migration/downgrade cases."
} finally {
    $taskResolved = [IO.Path]::GetFullPath($taskDir)
    $taskTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if ($taskResolved.StartsWith($taskTemp, [StringComparison]::OrdinalIgnoreCase) -and (Split-Path $taskResolved -Leaf) -like 'switchplus-version-test-*') {
        Remove-Item -LiteralPath $taskResolved -Recurse -Force
    }
}
