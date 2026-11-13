; Hooks load before INSTALLERICON is defined. Resolve from this tracked file
; so MUI uses the SwitchPlus icon for the uninstaller on every build machine.
; The pinned Rust Tauri config predates the uninstallerIcon JSON field.
!define MUI_UNICON "${__FILEDIR__}\..\icons\switchplus\icon.ico"
