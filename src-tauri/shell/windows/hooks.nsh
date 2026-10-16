!macro NSIS_HOOK_POSTINSTALL
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --install-shell-integration' $0
  ${If} $0 != 0
    DetailPrint "MaxRAR desktop actions could not be enabled. Use Help > Desktop integration to retry."
  ${EndIf}
  SetShellVarContext current
  CreateShortCut "$SENDTO\Compress with MaxRAR.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "--compress --" "$INSTDIR\${MAINBINARYNAME}.exe" 0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
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
