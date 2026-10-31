; Windows 11 compact-menu commands come from a signed sparse package (see
; src-tauri/shell/windows/package). Explorer's COM surrogate can keep the old
; maxrar_shell.dll loaded during an update, so move it aside before copying.
!macro MAXRAR_RELEASE_SHELL_DLL
  ${If} ${FileExists} "$INSTDIR\maxrar_shell.dll"
    Delete "$INSTDIR\maxrar_shell.dll.old"
    Rename "$INSTDIR\maxrar_shell.dll" "$INSTDIR\maxrar_shell.dll.old"
    Delete /REBOOTOK "$INSTDIR\maxrar_shell.dll.old"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro MAXRAR_RELEASE_SHELL_DLL
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --install-shell-integration' $0
  ${If} $0 != 0
    DetailPrint "MaxRAR desktop actions could not be enabled. Use Help > Desktop integration to retry."
  ${EndIf}
  SetShellVarContext current
  CreateShortCut "$SENDTO\Compress with MaxRAR.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "--compress --" "$INSTDIR\${MAINBINARYNAME}.exe" 0
  ReadRegStr $1 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion" "CurrentBuildNumber"
  ${If} $1 >= 22000
  ${AndIf} ${FileExists} "$INSTDIR\maxrar-explorer-menu.msix"
    nsExec::ExecToLog `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "Add-AppxPackage -Path '$INSTDIR\maxrar-explorer-menu.msix' -ExternalLocation '$INSTDIR' -ForceUpdateFromAnyVersion"`
    Pop $0
    ${If} $0 != 0
      DetailPrint "MaxRAR could not add its Windows 11 menu entry; the commands remain under Show more options."
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "Get-AppxPackage -Name Matily.MaxRAR.ExplorerMenu | Remove-AppxPackage"`
  Pop $0
  !insertmacro MAXRAR_RELEASE_SHELL_DLL
  ReadRegStr $0 HKCU "Software\Classes\*\shell\MaxRAR.Compress\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --compress -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\*\shell\MaxRAR.Compress"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\Directory\shell\MaxRAR.Compress\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --compress -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\Directory\shell\MaxRAR.Compress"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.zip\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.zip\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.rar\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.rar\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.7z\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.7z\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.tar\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.tar\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.gz\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.gz\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.bz2\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.xz\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.xz\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.zst\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.zst\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.mrar\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.mrar\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.tgz\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.tbz\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.tbz\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.tbz2\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.tbz2\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.txz\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.txz\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.tzst\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.tzst\shell\MaxRAR.Extract"
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Classes\SystemFileAssociations\.zstd\shell\MaxRAR.Extract\command" ""
  ${If} $0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --extract -- $\"%1$\"'
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.zstd\shell\MaxRAR.Extract"
  ${EndIf}
  SetShellVarContext current
  !insertmacro IsShortcutTarget "$SENDTO\Compress with MaxRAR.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 = 1
    Delete "$SENDTO\Compress with MaxRAR.lnk"
  ${EndIf}
!macroend
