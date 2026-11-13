; Hooks load before INSTALLERICON is defined. Resolve from this tracked file
; so MUI uses the SwitchPlus icon for the uninstaller on every build machine.
; The pinned Rust Tauri config predates the uninstallerIcon JSON field.
!define MUI_UNICON "${__FILEDIR__}\..\icons\switchplus\icon.ico"
!include "${__FILEDIR__}\version-policy.nsh"

!macro SwitchPlusReadWixValue DEST NAME
  ${If} $WixRegistryRoot == "HKCU"
    ReadRegStr ${DEST} HKCU "$R6" "${NAME}"
  ${Else}
    ReadRegStr ${DEST} HKLM "$R6" "${NAME}"
  ${EndIf}
!macroend
