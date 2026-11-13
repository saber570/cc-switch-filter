; $R0 comes from the existing SwitchPlus uninstall registration. Return the
; SemverCompare result: 1 upgrade, 0 same, -1 downgrade. No registry writes.
!macro SwitchPlusCompareInstalledVersion
  ${If} $R0 == "4.0.28"
  ${OrIf} $R0 == "4.0.29"
  ${OrIf} $R0 == "4.0.28.0"
  ${OrIf} $R0 == "4.0.29.0"
    StrCpy $R0 1
  ${Else}
    nsis_tauri_utils::SemverCompare "${VERSION}" $R0
    Pop $R0
  ${EndIf}
!macroend
