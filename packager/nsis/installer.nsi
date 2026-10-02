; Set the compression algorithm.
!if "{{compression}}" == ""
  SetCompressor /SOLID lzma
!else
  SetCompressor /SOLID "{{compression}}"
!endif

Unicode true

!include MUI2.nsh
!include nsDialogs.nsh
!include FileFunc.nsh
!include x64.nsh
!include WordFunc.nsh
!include "FileAssociation.nsh"
!include "StrFunc.nsh"
!include "StrFunc.nsh"
${StrCase}
${StrLoc}

!define MANUFACTURER "{{manufacturer}}"
!define PRODUCTNAME "{{product_name}}"
!define VERSION "{{version}}"
!define VERSIONWITHBUILD "{{version_with_build}}"
!define SHORTDESCRIPTION "{{short_description}}"
!define INSTALLMODE "{{install_mode}}"
!define LICENSE "{{license}}"
!define INSTALLERICON "{{installer_icon}}"
!define SIDEBARIMAGE "{{sidebar_image}}"
!define HEADERIMAGE "{{header_image}}"
!define MAINBINARYNAME "{{main_binary_name}}"
!define MAINBINARYSRCPATH "{{main_binary_path}}"
!define IDENTIFIER "{{identifier}}"
!define COPYRIGHT "{{copyright}}"
!define OUTFILE "{{out_file}}"
!define ARCH "{{arch}}"
!define PLUGINSPATH "{{additional_plugins_path}}"
!define ALLOWDOWNGRADES "{{allow_downgrades}}"
!define DISPLAYLANGUAGESELECTOR "{{display_language_selector}}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}"
!define MANUPRODUCTKEY "Software\${MANUFACTURER}\${PRODUCTNAME}"
!define UNINSTALLERSIGNCOMMAND "{{uninstaller_sign_cmd}}"
!define ESTIMATEDSIZE "{{estimated_size}}"


Name "${PRODUCTNAME}"
BrandingText "${COPYRIGHT}"
OutFile "${OUTFILE}"

VIProductVersion "${VERSIONWITHBUILD}"
VIAddVersionKey "ProductName" "${PRODUCTNAME}"
VIAddVersionKey "FileDescription" "${SHORTDESCRIPTION}"
VIAddVersionKey "LegalCopyright" "${COPYRIGHT}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"

; Plugins path, currently exists for linux only
!if "${PLUGINSPATH}" != ""
    !addplugindir "${PLUGINSPATH}"
!endif

!if "${UNINSTALLERSIGNCOMMAND}" != ""
  !uninstfinalize '${UNINSTALLERSIGNCOMMAND}'
!endif

; Handle install mode, `perUser`, `perMachine` or `both`
!if "${INSTALLMODE}" == "perMachine"
  RequestExecutionLevel highest
!endif

!if "${INSTALLMODE}" == "currentUser"
  RequestExecutionLevel user
!endif

!if "${INSTALLMODE}" == "both"
  !define MULTIUSER_MUI
  !define MULTIUSER_INSTALLMODE_INSTDIR "${PRODUCTNAME}"
  !define MULTIUSER_INSTALLMODE_COMMANDLINE
  !if "${ARCH}" == "x64"
    !define MULTIUSER_USE_PROGRAMFILES64
  !else if "${ARCH}" == "arm64"
    !define MULTIUSER_USE_PROGRAMFILES64
  !endif
  !define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_KEY "${UNINSTKEY}"
  !define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_VALUENAME "CurrentUser"
  !define MULTIUSER_INSTALLMODEPAGE_SHOWUSERNAME
  !define MULTIUSER_INSTALLMODE_FUNCTION RestorePreviousInstallLocation
  !define MULTIUSER_EXECUTIONLEVEL Highest
  !include MultiUser.nsh
!endif

; installer icon
!if "${INSTALLERICON}" != ""
  !define MUI_ICON "${INSTALLERICON}"
!endif

; installer sidebar image
!if "${SIDEBARIMAGE}" != ""
  !define MUI_WELCOMEFINISHPAGE_BITMAP "${SIDEBARIMAGE}"
!endif

; installer header image
!if "${HEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE
  !define MUI_HEADERIMAGE_BITMAP  "${HEADERIMAGE}"
!endif

; Define registry key to store installer language
!define MUI_LANGDLL_REGISTRY_ROOT "HKCU"
!define MUI_LANGDLL_REGISTRY_KEY "${MANUPRODUCTKEY}"
!define MUI_LANGDLL_REGISTRY_VALUENAME "Installer Language"

; Installer pages, must be ordered as they appear
; 1. Welcome Page
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_WELCOME

; 2. License Page (if defined)
!if "${LICENSE}" != ""
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !insertmacro MUI_PAGE_LICENSE "${LICENSE}"
!endif

; 3. Install mode (if it is set to `both`)
!if "${INSTALLMODE}" == "both"
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !insertmacro MULTIUSER_PAGE_INSTALLMODE
!endif


; 4. Custom page to ask user if he wants to reinstall/uninstall
;    only if a previous installtion was detected
Var ReinstallPageCheck
{{#if file_associations}}
Var AssociateFilesCheckbox
Var AssociateFilesCheckboxState
Var ContextMenuCheckbox
Var ContextMenuCheckboxState
{{/if}}
Page custom PageReinstall PageLeaveReinstall
Function PageReinstall
  ; Uninstall previous WiX installation if exists.
  ;
  ; A WiX installer stores the isntallation info in registry
  ; using a UUID and so we have to loop through all keys under
  ; `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`
  ; and check if `DisplayName` and `Publisher` keys match ${PRODUCTNAME} and ${MANUFACTURER}
  ;
  ; This has a potentional issue that there maybe another installation that matches
  ; our ${PRODUCTNAME} and ${MANUFACTURER} but wasn't installed by our WiX installer,
  ; however, this should be fine since the user will have to confirm the uninstallation
  ; and they can chose to abort it if doesn't make sense.
  StrCpy $0 0
  wix_loop:
    EnumRegKey $1 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall" $0
    StrCmp $1 "" wix_done ; Exit loop if there is no more keys to loop on
    IntOp $0 $0 + 1
    ReadRegStr $R0 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "DisplayName"
    ReadRegStr $R1 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "Publisher"
    StrCmp "$R0$R1" "${PRODUCTNAME}${MANUFACTURER}" 0 wix_loop
    ReadRegStr $R0 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "UninstallString"
    ${StrCase} $R1 $R0 "L"
    ${StrLoc} $R0 $R1 "msiexec" ">"
    StrCmp $R0 0 0 wix_done
    StrCpy $R7 "wix"
    StrCpy $R6 "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1"
    Goto compare_version
  wix_done:

  ; Check if there is an existing installation, if not, abort the reinstall page
  ReadRegStr $R0 SHCTX "${UNINSTKEY}" ""
  ReadRegStr $R1 SHCTX "${UNINSTKEY}" "UninstallString"
  ${IfThen} "$R0$R1" == "" ${|} Abort ${|}

  ; Compare this installar version with the existing installation
  ; and modify the messages presented to the user accordingly
  compare_version:
  StrCpy $R4 "$(older)"
  ${If} $R7 == "wix"
    ReadRegStr $R0 HKLM "$R6" "DisplayVersion"
  ${Else}
    ReadRegStr $R0 SHCTX "${UNINSTKEY}" "DisplayVersion"
  ${EndIf}
  ${IfThen} $R0 == "" ${|} StrCpy $R4 "$(unknown)" ${|}

  nsis_tauri_utils::SemverCompare "${VERSION}" $R0
  Pop $R0
  ; Reinstalling the same version
  ${If} $R0 == 0
    StrCpy $R1 "$(alreadyInstalledLong)"
    StrCpy $R2 "$(addOrReinstall)"
    StrCpy $R3 "$(uninstallApp)"
    !insertmacro MUI_HEADER_TEXT "$(alreadyInstalled)" "$(chooseMaintenanceOption)"
    StrCpy $R5 "2"
  ; Upgrading
  ${ElseIf} $R0 == 1
    StrCpy $R1 "$(olderOrUnknownVersionInstalled)"
    StrCpy $R2 "$(uninstallBeforeInstalling)"
    StrCpy $R3 "$(dontUninstall)"
    !insertmacro MUI_HEADER_TEXT "$(alreadyInstalled)" "$(choowHowToInstall)"
    StrCpy $R5 "1"
  ; Downgrading
  ${ElseIf} $R0 == -1
    StrCpy $R1 "$(newerVersionInstalled)"
    StrCpy $R2 "$(uninstallBeforeInstalling)"
    !if "${ALLOWDOWNGRADES}" == "true"
      StrCpy $R3 "$(dontUninstall)"
    !else
      StrCpy $R3 "$(dontUninstallDowngrade)"
    !endif
    !insertmacro MUI_HEADER_TEXT "$(alreadyInstalled)" "$(choowHowToInstall)"
    StrCpy $R5 "1"
  ${Else}
    Abort
  ${EndIf}

  Call SkipIfPassive

  nsDialogs::Create 1018
  Pop $R4
  ${IfThen} $(^RTL) == 1 ${|} nsDialogs::SetRTL $(^RTL) ${|}

  ${NSD_CreateLabel} 0 0 100% 24u $R1
  Pop $R1

  ${NSD_CreateRadioButton} 30u 50u -30u 8u $R2
  Pop $R2
  ${NSD_OnClick} $R2 PageReinstallUpdateSelection

  ${NSD_CreateRadioButton} 30u 70u -30u 8u $R3
  Pop $R3
  ; disable this radio button if downgrading and downgrades are disabled
  !if "${ALLOWDOWNGRADES}" == "false"
    ${IfThen} $R0 == -1 ${|} EnableWindow $R3 0 ${|}
  !endif
  ${NSD_OnClick} $R3 PageReinstallUpdateSelection

  ; Check the first radio button if this the first time
  ; we enter this page or if the second button wasn't
  ; selected the last time we were on this page
  ${If} $ReinstallPageCheck != 2
    SendMessage $R2 ${BM_SETCHECK} ${BST_CHECKED} 0
  ${Else}
    SendMessage $R3 ${BM_SETCHECK} ${BST_CHECKED} 0
  ${EndIf}

  ${NSD_SetFocus} $R2
  nsDialogs::Show
FunctionEnd
Function PageReinstallUpdateSelection
  ${NSD_GetState} $R2 $R1
  ${If} $R1 == ${BST_CHECKED}
    StrCpy $ReinstallPageCheck 1
  ${Else}
    StrCpy $ReinstallPageCheck 2
  ${EndIf}
FunctionEnd
Function PageLeaveReinstall
  ${NSD_GetState} $R2 $R1

  ; $R5 holds whether we are reinstalling the same version or not
  ; $R5 == "1" -> different versions
  ; $R5 == "2" -> same version
  ;
  ; $R1 holds the radio buttons state. its meaning is dependant on the context
  StrCmp $R5 "1" 0 +2 ; Existing install is not the same version?
    StrCmp $R1 "1" reinst_uninstall reinst_done ; $R1 == "1", then user chose to uninstall existing version, otherwise skip uninstalling
  StrCmp $R1 "1" reinst_done ; Same version? skip uninstalling

  reinst_uninstall:
    HideWindow
    ClearErrors

    ${If} $R7 == "wix"
      ReadRegStr $R1 HKLM "$R6" "UninstallString"
      ExecWait '$R1' $0
    ${Else}
      ReadRegStr $4 SHCTX "${MANUPRODUCTKEY}" ""
      ReadRegStr $R1 SHCTX "${UNINSTKEY}" "UninstallString"
      ExecWait '$R1 /P _?=$4' $0
    ${EndIf}

    BringToFront

    ${IfThen} ${Errors} ${|} StrCpy $0 2 ${|} ; ExecWait failed, set fake exit code

    ${If} $0 <> 0
    ${OrIf} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
      ${If} $0 = 1 ; User aborted uninstaller?
        StrCmp $R5 "2" 0 +2 ; Is the existing install the same version?
          Quit ; ...yes, already installed, we are done
        Abort
      ${EndIf}
      MessageBox MB_ICONEXCLAMATION "$(unableToUninstall)"
      Abort
    ${Else}
      StrCpy $0 $R1 1
      ${IfThen} $0 == '"' ${|} StrCpy $R1 $R1 -1 1 ${|} ; Strip quotes from UninstallString
      Delete $R1
      RMDir $INSTDIR
    ${EndIf}
  reinst_done:
FunctionEnd

{{#if file_associations}}
Function PageFileAssociations
  Call SkipIfPassive
  nsDialogs::Create 1018
  Pop $R0
  ${IfThen} $R0 == error ${|} Abort ${|}

  !insertmacro MUI_HEADER_TEXT "$(assocTitle)" "$(assocSubtitle)"
  ${NSD_CreateCheckbox} 0 20u 100% 12u "$(assocCheckbox)"
  Pop $AssociateFilesCheckbox
  ${If} $AssociateFilesCheckboxState == 1
    ${NSD_Check} $AssociateFilesCheckbox
  ${EndIf}

  ${NSD_CreateCheckbox} 0 36u 100% 12u "$(ctxMenuCheckbox)"
  Pop $ContextMenuCheckbox
  ${If} $ContextMenuCheckboxState == 1
    ${NSD_Check} $ContextMenuCheckbox
  ${EndIf}
  nsDialogs::Show
FunctionEnd

Function PageLeaveFileAssociations
  ${NSD_GetState} $AssociateFilesCheckbox $AssociateFilesCheckboxState
  ${NSD_GetState} $ContextMenuCheckbox $ContextMenuCheckboxState
FunctionEnd
{{/if}}

; 5. Choose install directoy page
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_DIRECTORY

; 6. Start menu shortcut page
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
Var AppStartMenuFolder
!insertmacro MUI_PAGE_STARTMENU Application $AppStartMenuFolder

{{#if file_associations}}
; 6b. File associations page
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
Page custom PageFileAssociations PageLeaveFileAssociations
{{/if}}

; 7. Installation page
!insertmacro MUI_PAGE_INSTFILES

; 8. Finish page
;
; Don't auto jump to finish page after installation page,
; because the installation page has useful info that can be used debug any issues with the installer.
!define MUI_FINISHPAGE_NOAUTOCLOSE
; Use show readme button in the finish page as a button create a desktop shortcut
!define MUI_FINISHPAGE_SHOWREADME
!define MUI_FINISHPAGE_SHOWREADME_TEXT "$(createDesktop)"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION CreateDesktopShortcut
; Show run app after installation.
!define MUI_FINISHPAGE_RUN "$INSTDIR\${MAINBINARYNAME}.exe"
!define MUI_FINISHPAGE_RUN_FUNCTION RunInstalledApp
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_FINISH

; Uninstaller Pages
; 1. Confirm uninstall page
{{#if appdata_paths}}
Var DeleteAppDataCheckbox
Var DeleteAppDataCheckboxState
!define /ifndef WS_EX_LAYOUTRTL         0x00400000
!define MUI_PAGE_CUSTOMFUNCTION_SHOW un.ConfirmShow
Function un.ConfirmShow
    FindWindow $1 "#32770" "" $HWNDPARENT ; Find inner dialog
    ${If} $(^RTL) == 1
      System::Call 'USER32::CreateWindowEx(i${__NSD_CheckBox_EXSTYLE}|${WS_EX_LAYOUTRTL},t"${__NSD_CheckBox_CLASS}",t "$(deleteAppData)",i${__NSD_CheckBox_STYLE},i 50,i 100,i 400, i 25,i$1,i0,i0,i0)i.s'
    ${Else}
      System::Call 'USER32::CreateWindowEx(i${__NSD_CheckBox_EXSTYLE},t"${__NSD_CheckBox_CLASS}",t "$(deleteAppData)",i${__NSD_CheckBox_STYLE},i 0,i 100,i 400, i 25,i$1,i0,i0,i0)i.s'
    ${EndIf}
    Pop $DeleteAppDataCheckbox
    SendMessage $HWNDPARENT ${WM_GETFONT} 0 0 $1
    SendMessage $DeleteAppDataCheckbox ${WM_SETFONT} $1 1
FunctionEnd
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE un.ConfirmLeave
Function un.ConfirmLeave
    SendMessage $DeleteAppDataCheckbox ${BM_GETCHECK} 0 0 $DeleteAppDataCheckboxState
FunctionEnd
{{/if}}
!insertmacro MUI_UNPAGE_CONFIRM

; 2. Uninstalling Page
!insertmacro MUI_UNPAGE_INSTFILES

;Languages
{{#each languages}}
!insertmacro MUI_LANGUAGE "{{this}}"
{{/each}}
!insertmacro MUI_RESERVEFILE_LANGDLL
{{#each language_files}}
  !include "{{this}}"
{{/each}}

{{#if file_associations}}
LangString assocTitle ${LANG_ENGLISH} "File associations"
LangString assocTitle ${LANG_ITALIAN} "Associazioni file"
LangString assocTitle ${LANG_SPANISH} "Asociaciones de archivos"
LangString assocTitle ${LANG_PORTUGUESE} "Associações de arquivos"
LangString assocTitle ${LANG_SWEDISH} "Filkopplingar"
LangString assocTitle ${LANG_VIETNAMESE} "Liên kết tệp"
LangString assocTitle ${LANG_CZECH} "Asociace souborů"

LangString assocSubtitle ${LANG_ENGLISH} "Choose whether to associate supported file types with ${PRODUCTNAME}."
LangString assocSubtitle ${LANG_ITALIAN} "Scegli se associare i file supportati a ${PRODUCTNAME}."
LangString assocSubtitle ${LANG_SPANISH} "Elija si desea asociar los tipos de archivo compatibles con ${PRODUCTNAME}."
LangString assocSubtitle ${LANG_PORTUGUESE} "Escolha se deseja associar os tipos de arquivos suportados ao ${PRODUCTNAME}."
LangString assocSubtitle ${LANG_SWEDISH} "Välj om du vill koppla filtyper som stöds till ${PRODUCTNAME}."
LangString assocSubtitle ${LANG_VIETNAMESE} "Chọn xem có liên kết các loại tệp được hỗ trợ với ${PRODUCTNAME} hay không."
LangString assocSubtitle ${LANG_CZECH} "Zvolte, zda chcete asociovat podporované typy souborů s aplikací ${PRODUCTNAME}."

LangString assocCheckbox ${LANG_ENGLISH} "Associate supported file types with ${PRODUCTNAME}"
LangString assocCheckbox ${LANG_ITALIAN} "Associa i file supportati a ${PRODUCTNAME}"
LangString assocCheckbox ${LANG_SPANISH} "Asociar tipos de archivo compatibles con ${PRODUCTNAME}"
LangString assocCheckbox ${LANG_PORTUGUESE} "Associar tipos de arquivos suportados ao ${PRODUCTNAME}"
LangString assocCheckbox ${LANG_SWEDISH} "Koppla filtyper som stöds till ${PRODUCTNAME}"
LangString assocCheckbox ${LANG_VIETNAMESE} "Liên kết các loại tệp được hỗ trợ với ${PRODUCTNAME}"
LangString assocCheckbox ${LANG_CZECH} "Asociovat podporované typy souborů s aplikací ${PRODUCTNAME}"

LangString ctxMenuCheckbox ${LANG_ENGLISH} "Add 'Open with ${PRODUCTNAME}' to the context menu"
LangString ctxMenuCheckbox ${LANG_ITALIAN} "Aggiungi $\"Apri con ${PRODUCTNAME}$\" al menu contestuale"
LangString ctxMenuCheckbox ${LANG_SPANISH} "Añadir 'Abrir con ${PRODUCTNAME}' al menú contextual"
LangString ctxMenuCheckbox ${LANG_PORTUGUESE} "Adicionar 'Abrir com ${PRODUCTNAME}' ao menu de contexto"
LangString ctxMenuCheckbox ${LANG_SWEDISH} "Lägg till 'Öppna med ${PRODUCTNAME}' i snabbmenyn"
LangString ctxMenuCheckbox ${LANG_VIETNAMESE} "Thêm 'Mở bằng ${PRODUCTNAME}' vào menu chuột phải"
LangString ctxMenuCheckbox ${LANG_CZECH} "Přidat 'Otevřít v ${PRODUCTNAME}' do kontextové nabídky"

LangString ctxMenuLabel ${LANG_ENGLISH} "Open with ${PRODUCTNAME}"
LangString ctxMenuLabel ${LANG_ITALIAN} "Apri con ${PRODUCTNAME}"
LangString ctxMenuLabel ${LANG_SPANISH} "Abrir con ${PRODUCTNAME}"
LangString ctxMenuLabel ${LANG_PORTUGUESE} "Abrir com ${PRODUCTNAME}"
LangString ctxMenuLabel ${LANG_SWEDISH} "Öppna med ${PRODUCTNAME}"
LangString ctxMenuLabel ${LANG_VIETNAMESE} "Mở bằng ${PRODUCTNAME}"
LangString ctxMenuLabel ${LANG_CZECH} "Otevřít v ${PRODUCTNAME}"

LangString older ${LANG_ENGLISH} "older"
LangString older ${LANG_ITALIAN} "piu vecchia"
LangString older ${LANG_SPANISH} "más antigua"
LangString older ${LANG_PORTUGUESE} "mais antiga"
LangString older ${LANG_SWEDISH} "äldre"
LangString older ${LANG_VIETNAMESE} "cũ hơn"
LangString older ${LANG_CZECH} "starší"

LangString unknown ${LANG_ENGLISH} "unknown"
LangString unknown ${LANG_ITALIAN} "sconosciuta"
LangString unknown ${LANG_SPANISH} "desconocida"
LangString unknown ${LANG_PORTUGUESE} "desconhecida"
LangString unknown ${LANG_SWEDISH} "okänd"
LangString unknown ${LANG_VIETNAMESE} "không xác định"
LangString unknown ${LANG_CZECH} "neznámá"

LangString alreadyInstalledLong ${LANG_ENGLISH} "${PRODUCTNAME} is already installed. Choose the operation to perform."
LangString alreadyInstalledLong ${LANG_ITALIAN} "${PRODUCTNAME} e gia installato. Scegli l'operazione da eseguire."
LangString alreadyInstalledLong ${LANG_SPANISH} "${PRODUCTNAME} ya está instalado. Elija la operación a realizar."
LangString alreadyInstalledLong ${LANG_PORTUGUESE} "O ${PRODUCTNAME} já está instalado. Escolha a operação a realizar."
LangString alreadyInstalledLong ${LANG_SWEDISH} "${PRODUCTNAME} är redan installerat. Välj den åtgärd du vill utföra."
LangString alreadyInstalledLong ${LANG_VIETNAMESE} "${PRODUCTNAME} đã được cài đặt. Chọn thao tác muốn thực hiện."
LangString alreadyInstalledLong ${LANG_CZECH} "${PRODUCTNAME} je již nainstalován. Vyberte operaci, kterou chcete provést."

LangString addOrReinstall ${LANG_ENGLISH} "Repair or reinstall ${PRODUCTNAME}"
LangString addOrReinstall ${LANG_ITALIAN} "Ripara o reinstalla ${PRODUCTNAME}"
LangString addOrReinstall ${LANG_SPANISH} "Reparar o reinstalar ${PRODUCTNAME}"
LangString addOrReinstall ${LANG_PORTUGUESE} "Reparar ou reinstalar o ${PRODUCTNAME}"
LangString addOrReinstall ${LANG_SWEDISH} "Reparera eller installera om ${PRODUCTNAME}"
LangString addOrReinstall ${LANG_VIETNAMESE} "Sửa chữa hoặc cài đặt lại ${PRODUCTNAME}"
LangString addOrReinstall ${LANG_CZECH} "Opravit nebo přeinstalovat ${PRODUCTNAME}"

LangString uninstallApp ${LANG_ENGLISH} "Uninstall ${PRODUCTNAME}"
LangString uninstallApp ${LANG_ITALIAN} "Disinstalla ${PRODUCTNAME}"
LangString uninstallApp ${LANG_SPANISH} "Desinstalar ${PRODUCTNAME}"
LangString uninstallApp ${LANG_PORTUGUESE} "Desinstalar o ${PRODUCTNAME}"
LangString uninstallApp ${LANG_SWEDISH} "Avinstallera ${PRODUCTNAME}"
LangString uninstallApp ${LANG_VIETNAMESE} "Gỡ cài đặt ${PRODUCTNAME}"
LangString uninstallApp ${LANG_CZECH} "Odinstalovat ${PRODUCTNAME}"

LangString alreadyInstalled ${LANG_ENGLISH} "Application already installed"
LangString alreadyInstalled ${LANG_ITALIAN} "Applicazione gia installata"
LangString alreadyInstalled ${LANG_SPANISH} "Aplicación ya instalada"
LangString alreadyInstalled ${LANG_PORTUGUESE} "Aplicação já instalada"
LangString alreadyInstalled ${LANG_SWEDISH} "Applikationen är redan installerad"
LangString alreadyInstalled ${LANG_VIETNAMESE} "Ứng dụng đã được cài đặt"
LangString alreadyInstalled ${LANG_CZECH} "Aplikace je již nainstalována"

LangString chooseMaintenanceOption ${LANG_ENGLISH} "Choose the maintenance option you want."
LangString chooseMaintenanceOption ${LANG_ITALIAN} "Scegli l'opzione di manutenzione."
LangString chooseMaintenanceOption ${LANG_SPANISH} "Elija la opción de mantenimiento que desee."
LangString chooseMaintenanceOption ${LANG_PORTUGUESE} "Escolha a opção de manutenção desejada."
LangString chooseMaintenanceOption ${LANG_SWEDISH} "Välj det underhållsalternativ du vill ha."
LangString chooseMaintenanceOption ${LANG_VIETNAMESE} "Chọn tùy chọn bảo trì bạn muốn."
LangString chooseMaintenanceOption ${LANG_CZECH} "Vyberte požadovanou možnost údržby."

LangString olderOrUnknownVersionInstalled ${LANG_ENGLISH} "An older or unknown version of ${PRODUCTNAME} is installed."
LangString olderOrUnknownVersionInstalled ${LANG_ITALIAN} "E installata una versione piu vecchia o sconosciuta di ${PRODUCTNAME}."
LangString olderOrUnknownVersionInstalled ${LANG_SPANISH} "Una versión más antigua o desconocida de ${PRODUCTNAME} está instalada."
LangString olderOrUnknownVersionInstalled ${LANG_PORTUGUESE} "Uma versão mais antiga ou desconhecida do ${PRODUCTNAME} está instalada."
LangString olderOrUnknownVersionInstalled ${LANG_SWEDISH} "En äldre eller okänd version av ${PRODUCTNAME} är installerad."
LangString olderOrUnknownVersionInstalled ${LANG_VIETNAMESE} "Một phiên bản cũ hơn hoặc không xác định của ${PRODUCTNAME} đã được cài đặt."
LangString olderOrUnknownVersionInstalled ${LANG_CZECH} "Je nainstalována starší nebo neznámá verze ${PRODUCTNAME}."

LangString uninstallBeforeInstalling ${LANG_ENGLISH} "Uninstall it before installing this version."
LangString uninstallBeforeInstalling ${LANG_ITALIAN} "Disinstallala prima di installare questa versione."
LangString uninstallBeforeInstalling ${LANG_SPANISH} "Desinstálela antes de instalar esta versión."
LangString uninstallBeforeInstalling ${LANG_PORTUGUESE} "Desinstale-a antes de instalar esta versão."
LangString uninstallBeforeInstalling ${LANG_SWEDISH} "Avinstallera den innan du installerar denna version."
LangString uninstallBeforeInstalling ${LANG_VIETNAMESE} "Gỡ cài đặt trước khi cài đặt phiên bản này."
LangString uninstallBeforeInstalling ${LANG_CZECH} "Před instalací této verze ji odinstalujte."

LangString dontUninstall ${LANG_ENGLISH} "Do not uninstall"
LangString dontUninstall ${LANG_ITALIAN} "Non disinstallare"
LangString dontUninstall ${LANG_SPANISH} "No desinstalar"
LangString dontUninstall ${LANG_PORTUGUESE} "Não desinstalar"
LangString dontUninstall ${LANG_SWEDISH} "Avinstallera inte"
LangString dontUninstall ${LANG_VIETNAMESE} "Không gỡ cài đặt"
LangString dontUninstall ${LANG_CZECH} "Neodinstalovávat"

LangString choowHowToInstall ${LANG_ENGLISH} "Choose how you want to install."
LangString choowHowToInstall ${LANG_ITALIAN} "Scegli come vuoi installare."
LangString choowHowToInstall ${LANG_SPANISH} "Elija cómo desea instalar."
LangString choowHowToInstall ${LANG_PORTUGUESE} "Escolha como deseja instalar."
LangString choowHowToInstall ${LANG_SWEDISH} "Välj hur du vill installera."
LangString choowHowToInstall ${LANG_VIETNAMESE} "Chọn cách bạn muốn cài đặt."
LangString choowHowToInstall ${LANG_CZECH} "Zvolte způsob instalace."

LangString newerVersionInstalled ${LANG_ENGLISH} "A newer version of ${PRODUCTNAME} is already installed."
LangString newerVersionInstalled ${LANG_ITALIAN} "E gia installata una versione piu recente di ${PRODUCTNAME}."
LangString newerVersionInstalled ${LANG_SPANISH} "Ya hay una versión más reciente de ${PRODUCTNAME} instalada."
LangString newerVersionInstalled ${LANG_PORTUGUESE} "Já existe uma versão mais recente do ${PRODUCTNAME} instalada."
LangString newerVersionInstalled ${LANG_SWEDISH} "En nyare version av ${PRODUCTNAME} är redan installerad."
LangString newerVersionInstalled ${LANG_VIETNAMESE} "Một phiên bản mới hơn của ${PRODUCTNAME} đã được cài đặt."
LangString newerVersionInstalled ${LANG_CZECH} "Je již nainstalována novější verze ${PRODUCTNAME}."

LangString dontUninstallDowngrade ${LANG_ENGLISH} "Do not uninstall (cancel)"
LangString dontUninstallDowngrade ${LANG_ITALIAN} "Non disinstallare (annulla)"
LangString dontUninstallDowngrade ${LANG_SPANISH} "No desinstalar (cancelar)"
LangString dontUninstallDowngrade ${LANG_PORTUGUESE} "Não desinstalar (cancelar)"
LangString dontUninstallDowngrade ${LANG_SWEDISH} "Avinstallera inte (avbryt)"
LangString dontUninstallDowngrade ${LANG_VIETNAMESE} "Không gỡ cài đặt (hủy)"
LangString dontUninstallDowngrade ${LANG_CZECH} "Neodinstalovávat (zrušit)"

LangString unableToUninstall ${LANG_ENGLISH} "Unable to uninstall. Please close ${PRODUCTNAME} and try again."
LangString unableToUninstall ${LANG_ITALIAN} "Impossibile disinstallare. Chiudi ${PRODUCTNAME} e riprova."
LangString unableToUninstall ${LANG_SPANISH} "No se puede desinstalar. Por favor, cierre ${PRODUCTNAME} e inténtelo de nuevo."
LangString unableToUninstall ${LANG_PORTUGUESE} "Não foi possível desinstalar. Por favor, feche o ${PRODUCTNAME} e tente novamente."
LangString unableToUninstall ${LANG_SWEDISH} "Kunde inte avinstallera. Stäng ${PRODUCTNAME} och försök igen."
LangString unableToUninstall ${LANG_VIETNAMESE} "Không thể gỡ cài đặt. Vui lòng đóng ${PRODUCTNAME} và thử lại."
LangString unableToUninstall ${LANG_CZECH} "Odinstalaci se nepodařilo provést. Zavřete ${PRODUCTNAME} a zkuste to znovu."

LangString createDesktop ${LANG_ENGLISH} "Create a desktop shortcut"
LangString createDesktop ${LANG_ITALIAN} "Crea un collegamento sul desktop"
LangString createDesktop ${LANG_SPANISH} "Crear un acceso directo en el escritorio"
LangString createDesktop ${LANG_PORTUGUESE} "Criar um atalho na área de trabalho"
LangString createDesktop ${LANG_SWEDISH} "Skapa en skrivbordsgenväg"
LangString createDesktop ${LANG_VIETNAMESE} "Tạo phím tắt trên màn hình nền"
LangString createDesktop ${LANG_CZECH} "Vytvořit zástupce na ploše"

LangString appRunningOkKill ${LANG_ENGLISH} "${PRODUCTNAME} is running. Close it now?"
LangString appRunningOkKill ${LANG_ITALIAN} "${PRODUCTNAME} e in esecuzione. Chiuderla ora?"
LangString appRunningOkKill ${LANG_SPANISH} "${PRODUCTNAME} se está ejecutando. ¿Cerrarlo ahora?"
LangString appRunningOkKill ${LANG_PORTUGUESE} "O ${PRODUCTNAME} está em execução. Fechá-lo agora?"
LangString appRunningOkKill ${LANG_SWEDISH} "${PRODUCTNAME} körs. Stäng nu?"
LangString appRunningOkKill ${LANG_VIETNAMESE} "${PRODUCTNAME} đang chạy. Đóng nó ngay bây giờ?"
LangString appRunningOkKill ${LANG_CZECH} "${PRODUCTNAME} právě běží. Chcete jej nyní zavřít?"

LangString appRunning ${LANG_ENGLISH} "${PRODUCTNAME} is running."
LangString appRunning ${LANG_ITALIAN} "${PRODUCTNAME} e in esecuzione."
LangString appRunning ${LANG_SPANISH} "${PRODUCTNAME} se está ejecutando."
LangString appRunning ${LANG_PORTUGUESE} "O ${PRODUCTNAME} está em execução."
LangString appRunning ${LANG_SWEDISH} "${PRODUCTNAME} körs."
LangString appRunning ${LANG_VIETNAMESE} "${PRODUCTNAME} đang chạy."
LangString appRunning ${LANG_CZECH} "${PRODUCTNAME} právě běží."

LangString failedToKillApp ${LANG_ENGLISH} "Failed to close ${PRODUCTNAME}."
LangString failedToKillApp ${LANG_ITALIAN} "Impossibile chiudere ${PRODUCTNAME}."
LangString failedToKillApp ${LANG_SPANISH} "Error al cerrar ${PRODUCTNAME}."
LangString failedToKillApp ${LANG_PORTUGUESE} "Falha ao fechar o ${PRODUCTNAME}."
LangString failedToKillApp ${LANG_SWEDISH} "Misslyckades med att stänga ${PRODUCTNAME}."
LangString failedToKillApp ${LANG_VIETNAMESE} "Không thể đóng ${PRODUCTNAME}."
LangString failedToKillApp ${LANG_CZECH} "Nepodařilo se zavřít ${PRODUCTNAME}."
LangString assocTitle ${LANG_SERBIAN} "Povezivanje datoteka"
LangString assocTitle ${LANG_UKRAINIAN} "Асоціації файлів"
LangString assocTitle ${LANG_SIMPCHINESE} "文件关联"
LangString assocTitle ${LANG_LITHUANIAN} "Failų susiejimai"
LangString assocSubtitle ${LANG_SERBIAN} "Izaberite da li zelite da povezete podrzane tipove datoteka sa ${PRODUCTNAME}."
LangString assocSubtitle ${LANG_UKRAINIAN} "Виберіть, чи асоціювати підтримувані типи файлів із ${PRODUCTNAME}."
LangString assocSubtitle ${LANG_SIMPCHINESE} "选择是否将受支持的文件类型与 ${PRODUCTNAME} 关联。"
LangString assocSubtitle ${LANG_LITHUANIAN} "Pasirinkite, ar susieti palaikomus failų tipus su ${PRODUCTNAME}."
LangString assocCheckbox ${LANG_SERBIAN} "Povezi podrzane tipove datoteka sa ${PRODUCTNAME}"
LangString assocCheckbox ${LANG_UKRAINIAN} "Асоціювати підтримувані типи файлів із ${PRODUCTNAME}"
LangString assocCheckbox ${LANG_SIMPCHINESE} "将受支持的文件类型与 ${PRODUCTNAME} 关联"
LangString assocCheckbox ${LANG_LITHUANIAN} "Susieti palaikomus failų tipus su ${PRODUCTNAME}"
LangString ctxMenuCheckbox ${LANG_SERBIAN} "Dodaj 'Otvori pomocu ${PRODUCTNAME}' u kontekstni meni"
LangString ctxMenuCheckbox ${LANG_UKRAINIAN} "Додати 'Відкрити за допомогою ${PRODUCTNAME}' до контекстного меню"
LangString ctxMenuCheckbox ${LANG_SIMPCHINESE} "将 '使用 ${PRODUCTNAME} 打开' 添加到右键菜单"
LangString ctxMenuCheckbox ${LANG_LITHUANIAN} "Pridėti „Atidaryti su ${PRODUCTNAME}“ į kontekstinį meniu"
LangString ctxMenuLabel ${LANG_SERBIAN} "Otvori pomocu ${PRODUCTNAME}"
LangString ctxMenuLabel ${LANG_UKRAINIAN} "Відкрити за допомогою ${PRODUCTNAME}"
LangString ctxMenuLabel ${LANG_SIMPCHINESE} "使用 ${PRODUCTNAME} 打开"
LangString ctxMenuLabel ${LANG_LITHUANIAN} "Atidaryti su ${PRODUCTNAME}"
LangString older ${LANG_SERBIAN} "starija"
LangString older ${LANG_UKRAINIAN} "старіша"
LangString older ${LANG_SIMPCHINESE} "较旧版本"
LangString older ${LANG_LITHUANIAN} "senesnė"
LangString unknown ${LANG_SERBIAN} "nepoznata"
LangString unknown ${LANG_UKRAINIAN} "невідома"
LangString unknown ${LANG_SIMPCHINESE} "未知"
LangString unknown ${LANG_LITHUANIAN} "nežinoma"
LangString alreadyInstalledLong ${LANG_SERBIAN} "${PRODUCTNAME} je vec instaliran. Izaberite operaciju koju zelite da izvrsite."
LangString alreadyInstalledLong ${LANG_UKRAINIAN} "${PRODUCTNAME} уже встановлено. Виберіть дію для виконання."
LangString alreadyInstalledLong ${LANG_SIMPCHINESE} "${PRODUCTNAME} 已安装。请选择要执行的操作。"
LangString alreadyInstalledLong ${LANG_LITHUANIAN} "${PRODUCTNAME} jau įdiegta. Pasirinkite veiksmą, kurį norite atlikti."
LangString addOrReinstall ${LANG_SERBIAN} "Popravi ili ponovo instaliraj ${PRODUCTNAME}"
LangString addOrReinstall ${LANG_UKRAINIAN} "Відновити або перевстановити ${PRODUCTNAME}"
LangString addOrReinstall ${LANG_SIMPCHINESE} "修复或重新安装 ${PRODUCTNAME}"
LangString addOrReinstall ${LANG_LITHUANIAN} "Taisyti arba iš naujo įdiegti ${PRODUCTNAME}"
LangString uninstallApp ${LANG_SERBIAN} "Deinstaliraj ${PRODUCTNAME}"
LangString uninstallApp ${LANG_UKRAINIAN} "Видалити ${PRODUCTNAME}"
LangString uninstallApp ${LANG_SIMPCHINESE} "卸载 ${PRODUCTNAME}"
LangString uninstallApp ${LANG_LITHUANIAN} "Pašalinti ${PRODUCTNAME}"
LangString alreadyInstalled ${LANG_SERBIAN} "Aplikacija je vec instalirana"
LangString alreadyInstalled ${LANG_UKRAINIAN} "Програму вже встановлено"
LangString alreadyInstalled ${LANG_SIMPCHINESE} "应用程序已安装"
LangString alreadyInstalled ${LANG_LITHUANIAN} "Programa jau įdiegta"
LangString chooseMaintenanceOption ${LANG_SERBIAN} "Izaberite zeljenu opciju odrzavanja."
LangString chooseMaintenanceOption ${LANG_UKRAINIAN} "Виберіть потрібний варіант обслуговування."
LangString chooseMaintenanceOption ${LANG_SIMPCHINESE} "选择所需的维护选项。"
LangString chooseMaintenanceOption ${LANG_LITHUANIAN} "Pasirinkite norimą priežiūros parinktį."
LangString olderOrUnknownVersionInstalled ${LANG_SERBIAN} "Instalirana je starija ili nepoznata verzija ${PRODUCTNAME}."
LangString olderOrUnknownVersionInstalled ${LANG_UKRAINIAN} "Встановлено старішу або невідому версію ${PRODUCTNAME}."
LangString olderOrUnknownVersionInstalled ${LANG_SIMPCHINESE} "已安装较旧或未知版本的 ${PRODUCTNAME}。"
LangString olderOrUnknownVersionInstalled ${LANG_LITHUANIAN} "Įdiegta senesnė arba nežinoma ${PRODUCTNAME} versija."
LangString uninstallBeforeInstalling ${LANG_SERBIAN} "Deinstalirajte je pre instalacije ove verzije."
LangString uninstallBeforeInstalling ${LANG_UKRAINIAN} "Видаліть її перед встановленням цієї версії."
LangString uninstallBeforeInstalling ${LANG_SIMPCHINESE} "安装此版本之前请先卸载。"
LangString uninstallBeforeInstalling ${LANG_LITHUANIAN} "Prieš diegdami šią versiją, ją pašalinkite."
LangString dontUninstall ${LANG_SERBIAN} "Ne deinstaliraj"
LangString dontUninstall ${LANG_UKRAINIAN} "Не видаляти"
LangString dontUninstall ${LANG_SIMPCHINESE} "不要卸载"
LangString dontUninstall ${LANG_LITHUANIAN} "Nešalinti"
LangString choowHowToInstall ${LANG_SERBIAN} "Izaberite nacin instalacije."
LangString choowHowToInstall ${LANG_UKRAINIAN} "Виберіть спосіб встановлення."
LangString choowHowToInstall ${LANG_SIMPCHINESE} "选择安装方式。"
LangString choowHowToInstall ${LANG_LITHUANIAN} "Pasirinkite, kaip norite įdiegti."
LangString newerVersionInstalled ${LANG_SERBIAN} "Novija verzija ${PRODUCTNAME} je vec instalirana."
LangString newerVersionInstalled ${LANG_UKRAINIAN} "Новіша версія ${PRODUCTNAME} вже встановлена."
LangString newerVersionInstalled ${LANG_SIMPCHINESE} "已安装较新版本的 ${PRODUCTNAME}。"
LangString newerVersionInstalled ${LANG_LITHUANIAN} "Jau įdiegta naujesnė ${PRODUCTNAME} versija."
LangString dontUninstallDowngrade ${LANG_SERBIAN} "Ne deinstaliraj (otkazi)"
LangString dontUninstallDowngrade ${LANG_UKRAINIAN} "Не видаляти (скасувати)"
LangString dontUninstallDowngrade ${LANG_SIMPCHINESE} "不要卸载（取消）"
LangString dontUninstallDowngrade ${LANG_LITHUANIAN} "Nešalinti (atšaukti)"
LangString unableToUninstall ${LANG_SERBIAN} "Nije moguce deinstalirati. Zatvorite ${PRODUCTNAME} i pokusajte ponovo."
LangString unableToUninstall ${LANG_UKRAINIAN} "Не вдалося видалити. Закрийте ${PRODUCTNAME} і спробуйте знову."
LangString unableToUninstall ${LANG_SIMPCHINESE} "无法卸载。请关闭 ${PRODUCTNAME} 后重试。"
LangString unableToUninstall ${LANG_LITHUANIAN} "Nepavyko pašalinti. Uždarykite ${PRODUCTNAME} ir bandykite dar kartą."
LangString createDesktop ${LANG_SERBIAN} "Napravi precicu na radnoj povrsini"
LangString createDesktop ${LANG_UKRAINIAN} "Створити ярлик на робочому столі"
LangString createDesktop ${LANG_SIMPCHINESE} "创建桌面快捷方式"
LangString createDesktop ${LANG_LITHUANIAN} "Sukurti darbalaukio nuorodą"
LangString appRunningOkKill ${LANG_SERBIAN} "${PRODUCTNAME} je pokrenut. Zatvoriti sada?"
LangString appRunningOkKill ${LANG_UKRAINIAN} "${PRODUCTNAME} запущено. Закрити зараз?"
LangString appRunningOkKill ${LANG_SIMPCHINESE} "${PRODUCTNAME} 正在运行。现在关闭吗？"
LangString appRunningOkKill ${LANG_LITHUANIAN} "${PRODUCTNAME} veikia. Uždaryti dabar?"
LangString appRunning ${LANG_SERBIAN} "${PRODUCTNAME} je pokrenut."
LangString appRunning ${LANG_UKRAINIAN} "${PRODUCTNAME} запущено."
LangString appRunning ${LANG_SIMPCHINESE} "${PRODUCTNAME} 正在运行。"
LangString appRunning ${LANG_LITHUANIAN} "${PRODUCTNAME} veikia."
LangString failedToKillApp ${LANG_SERBIAN} "Nije uspelo zatvaranje ${PRODUCTNAME}."
LangString failedToKillApp ${LANG_UKRAINIAN} "Не вдалося закрити ${PRODUCTNAME}."
LangString failedToKillApp ${LANG_SIMPCHINESE} "无法关闭 ${PRODUCTNAME}。"
LangString failedToKillApp ${LANG_LITHUANIAN} "Nepavyko uždaryti ${PRODUCTNAME}."
{{/if}}

!macro SetContext
  !if "${INSTALLMODE}" == "currentUser"
    SetShellVarContext current
  !else if "${INSTALLMODE}" == "perMachine"
    SetShellVarContext all
  !endif

  ${If} ${RunningX64}
    !if "${ARCH}" == "x64"
      SetRegView 64
    !else if "${ARCH}" == "arm64"
      SetRegView 64
    !else
      SetRegView 32
    !endif
  ${EndIf}
!macroend

Var PassiveMode
Function .onInit
  ${GetOptions} $CMDLINE "/P" $PassiveMode
  IfErrors +2 0
    StrCpy $PassiveMode 1

{{#if file_associations}}
  StrCpy $AssociateFilesCheckboxState 0
  StrCpy $ContextMenuCheckboxState 1
{{/if}}

  !if "${DISPLAYLANGUAGESELECTOR}" == "true"
    !insertmacro MUI_LANGDLL_DISPLAY
  !endif

  !insertmacro SetContext

  ${If} $INSTDIR == ""
    ; Set default install location
    !if "${INSTALLMODE}" == "perMachine"
      ${If} ${RunningX64}
        !if "${ARCH}" == "x64"
          StrCpy $INSTDIR "$PROGRAMFILES64\${PRODUCTNAME}"
        !else if "${ARCH}" == "arm64"
          StrCpy $INSTDIR "$PROGRAMFILES64\${PRODUCTNAME}"
        !else
          StrCpy $INSTDIR "$PROGRAMFILES\${PRODUCTNAME}"
        !endif
      ${Else}
        StrCpy $INSTDIR "$PROGRAMFILES\${PRODUCTNAME}"
      ${EndIf}
    !else if "${INSTALLMODE}" == "currentUser"
      StrCpy $INSTDIR "$LOCALAPPDATA\${PRODUCTNAME}"
    !endif

    Call RestorePreviousInstallLocation
  ${EndIf}


  !if "${INSTALLMODE}" == "both"
    !insertmacro MULTIUSER_INIT
  !endif
FunctionEnd


Section EarlyChecks
  ; Abort silent installer if downgrades is disabled
  !if "${ALLOWDOWNGRADES}" == "false"
  IfSilent 0 silent_downgrades_done
    ; If downgrading
    ${If} $R0 == -1
      System::Call 'kernel32::AttachConsole(i -1)i.r0'
      ${If} $0 != 0
        System::Call 'kernel32::GetStdHandle(i -11)i.r0'
        System::call 'kernel32::SetConsoleTextAttribute(i r0, i 0x0004)' ; set red color
        FileWrite $0 "$(silentDowngrades)"
      ${EndIf}
      Abort
    ${EndIf}
  silent_downgrades_done:
  !endif

SectionEnd

{{#if preinstall_section}}
{{unescape_newlines preinstall_section}}
{{/if}}

!macro CheckIfAppIsRunning
  nsis_tauri_utils::FindProcess "${MAINBINARYNAME}.exe"
  Pop $R0
  ${If} $R0 = 0
      IfSilent kill 0
      ${IfThen} $PassiveMode != 1 ${|} MessageBox MB_OKCANCEL "$(appRunningOkKill)" IDOK kill IDCANCEL cancel ${|}
      kill:
        nsis_tauri_utils::KillProcess "${MAINBINARYNAME}.exe"
        Pop $R0
        Sleep 500
        ${If} $R0 = 0
          Goto app_check_done
        ${Else}
          IfSilent silent ui
          silent:
            System::Call 'kernel32::AttachConsole(i -1)i.r0'
            ${If} $0 != 0
              System::Call 'kernel32::GetStdHandle(i -11)i.r0'
              System::call 'kernel32::SetConsoleTextAttribute(i r0, i 0x0004)' ; set red color
              FileWrite $0 "$(appRunning)$\n"
            ${EndIf}
            Abort
          ui:
            Abort "$(failedToKillApp)"
        ${EndIf}
      cancel:
        Abort "$(appRunning)"
  ${EndIf}
  app_check_done:
!macroend

Section Install
  SetOutPath $INSTDIR

  !insertmacro CheckIfAppIsRunning

  ; Backup current .txt ProgID before changing associations, so uninstall can restore it.
  ClearErrors
  ReadRegStr $R8 SHCTX "Software\Classes\.txt" ""
  ${IfNot} ${Errors}
    WriteRegStr SHCTX "${MANUPRODUCTKEY}" "TxtPrevProgId" "$R8"
  ${Else}
    DeleteRegValue SHCTX "${MANUPRODUCTKEY}" "TxtPrevProgId"
  ${EndIf}

  ; Copy main executable
  File "${MAINBINARYSRCPATH}"

  ; Create resources directory structure
  {{#each resources_dirs}}
    CreateDirectory "$INSTDIR\\{{this}}"
  {{/each}}

  ; Copy resources
  {{#each resources}}
    File /a "/oname={{this}}" "{{@key}}"
  {{/each}}

  ; Copy external binaries
  {{#each binaries}}
    File /a "/oname={{this}}" "{{@key}}"
  {{/each}}

  ; Create file associations
  {{#if file_associations}}
  ${If} $AssociateFilesCheckboxState == 1
    {{#each file_associations as |association| ~}}
      {{#each association.extensions as |ext| ~}}
        !insertmacro APP_ASSOCIATE "{{ext}}" "{{or association.name ext}}" "{{association-description association.description ext}}" "$INSTDIR\${MAINBINARYNAME}.exe,0" "Open with ${PRODUCTNAME}" "$INSTDIR\${MAINBINARYNAME}.exe $\"%1$\""
      {{/each}}
    {{/each}}
    WriteRegStr SHCTX "${MANUPRODUCTKEY}" "FileAssociations" "1"
  ${Else}
    DeleteRegValue SHCTX "${MANUPRODUCTKEY}" "FileAssociations"
  ${EndIf}
  {{/if}}

  ; Create context menu entries
  {{#if file_associations}}
  ${If} $ContextMenuCheckboxState == 1
    {{#each file_associations as |association| ~}}
      {{#each association.extensions as |ext| ~}}
        WriteRegStr SHCTX "Software\Classes\SystemFileAssociations\.{{ext}}\shell\OpenWithSonarpad" "" "$(ctxMenuLabel)"
        WriteRegStr SHCTX "Software\Classes\SystemFileAssociations\.{{ext}}\shell\OpenWithSonarpad\DefaultIcon" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
        WriteRegStr SHCTX "Software\Classes\SystemFileAssociations\.{{ext}}\shell\OpenWithSonarpad\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
      {{/each}}
    {{/each}}
    WriteRegStr SHCTX "${MANUPRODUCTKEY}" "ContextMenu" "1"
  ${Else}
    DeleteRegValue SHCTX "${MANUPRODUCTKEY}" "ContextMenu"
  ${EndIf}
  {{/if}}

  ; Register deep links
  {{#each deep_link_protocols as |protocol| ~}}
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}" "URL Protocol" ""
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}" "" "URL:${BUNDLEID} protocol"
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}\DefaultIcon" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}\shell\open\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
  {{/each}}

  ; Create uninstaller
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Save $INSTDIR in registry for future installations
  WriteRegStr SHCTX "${MANUPRODUCTKEY}" "" $INSTDIR

  !if "${INSTALLMODE}" == "both"
    ; Save install mode to be selected by default for the next installation such as updating
    ; or when uninstalling
    WriteRegStr SHCTX "${UNINSTKEY}" $MultiUser.InstallMode 1
  !endif

  ; Registry information for add/remove programs
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "${PRODUCTNAME}"
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayIcon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\""
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr SHCTX "${UNINSTKEY}" "Publisher" "${MANUFACTURER}"
  WriteRegStr SHCTX "${UNINSTKEY}" "InstallLocation" "$\"$INSTDIR$\""
  WriteRegStr SHCTX "${UNINSTKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegDWORD SHCTX "${UNINSTKEY}" "NoModify" "1"
  WriteRegDWORD SHCTX "${UNINSTKEY}" "NoRepair" "1"
  WriteRegDWORD SHCTX "${UNINSTKEY}" "EstimatedSize" "${ESTIMATEDSIZE}"

  ; Create start menu shortcut (GUI)
  !insertmacro MUI_STARTMENU_WRITE_BEGIN Application
    Call CreateStartMenuShortcut
  !insertmacro MUI_STARTMENU_WRITE_END

  ; Create shortcuts for silent and passive installers, which
  ; can be disabled by passing `/NS` flag
  ; GUI installer has buttons for users to control creating them
  IfSilent check_ns_flag 0
  ${IfThen} $PassiveMode == 1 ${|} Goto check_ns_flag ${|}
  Goto shortcuts_done
  check_ns_flag:
    ${GetOptions} $CMDLINE "/NS" $R0
    IfErrors 0 shortcuts_done
      Call CreateDesktopShortcut
      Call CreateStartMenuShortcut
  shortcuts_done:

  ; Auto close this page for passive mode
  ${IfThen} $PassiveMode == 1 ${|} SetAutoClose true ${|}
SectionEnd

Function .onInstSuccess
  ; Check for `/R` flag only in silent and passive installers because
  ; GUI installer has a toggle for the user to (re)start the app
  IfSilent check_r_flag 0
  ${IfThen} $PassiveMode == 1 ${|} Goto check_r_flag ${|}
  Goto run_done
  check_r_flag:
    ${GetOptions} $CMDLINE "/R" $R0
    IfErrors run_done 0
      Call RunInstalledApp
  run_done:
FunctionEnd

Function RunInstalledApp
  ; Ensure we start the newly installed copy, not an already-running old instance.
  nsis_tauri_utils::FindProcess "${MAINBINARYNAME}.exe"
  Pop $R0
  ${If} $R0 = 0
    nsis_tauri_utils::KillProcess "${MAINBINARYNAME}.exe"
    Pop $R0
    Sleep 500
  ${EndIf}
  Exec '"$INSTDIR\${MAINBINARYNAME}.exe"'
FunctionEnd

Function un.onInit
  !insertmacro SetContext

  !if "${INSTALLMODE}" == "both"
    !insertmacro MULTIUSER_UNINIT
  !endif

  !insertmacro MUI_UNGETLANGUAGE
FunctionEnd

Section Uninstall
  !insertmacro CheckIfAppIsRunning

  ; Delete the app directory and its content from disk
  ; Copy main executable
  Delete "$INSTDIR\${MAINBINARYNAME}.exe"

  ; Delete resources
  {{#each resources}}
    Delete "$INSTDIR\\{{this}}"
  {{/each}}

  ; Delete external binaries
  {{#each binaries}}
    Delete "$INSTDIR\\{{this}}"
  {{/each}}

  ; Delete app associations
  {{#if file_associations}}
  ReadRegStr $R0 SHCTX "${MANUPRODUCTKEY}" "FileAssociations"
  ${If} $R0 == "1"
    {{#each file_associations as |association| ~}}
      {{#each association.extensions as |ext| ~}}
        !insertmacro APP_UNASSOCIATE "{{ext}}" "{{or association.name ext}}"
      {{/each}}
    {{/each}}
    ; Explicitly restore standard .txt "New Text Document" behavior.
    ReadRegStr $R8 SHCTX "${MANUPRODUCTKEY}" "TxtPrevProgId"
    ${If} $R8 == ""
      StrCpy $R8 "txtfile"
    ${EndIf}
    WriteRegStr SHCTX "Software\Classes\.txt" "" "$R8"
    WriteRegStr SHCTX "Software\Classes\$R8\ShellNew" "NullFile" ""
    WriteRegStr SHCTX "Software\Classes\txtfile\ShellNew" "NullFile" ""
    DeleteRegValue SHCTX "${MANUPRODUCTKEY}" "TxtPrevProgId"
    DeleteRegValue SHCTX "${MANUPRODUCTKEY}" "FileAssociations"
  ${EndIf}
  {{/if}}

  ; Delete context menu entries
  {{#if file_associations}}
  ReadRegStr $R0 SHCTX "${MANUPRODUCTKEY}" "ContextMenu"
  ${If} $R0 == "1"
    {{#each file_associations as |association| ~}}
      {{#each association.extensions as |ext| ~}}
        DeleteRegKey SHCTX "Software\Classes\SystemFileAssociations\.{{ext}}\shell\OpenWithSonarpad"
        DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.{{ext}}\shell\OpenWithSonarpad"
        DeleteRegKey HKLM "Software\Classes\SystemFileAssociations\.{{ext}}\shell\OpenWithSonarpad"
      {{/each}}
    {{/each}}
    DeleteRegKey SHCTX "Software\Classes\*\shell\OpenWithSonarpad"
    DeleteRegKey HKCU "Software\Classes\*\shell\OpenWithSonarpad"
    DeleteRegKey HKLM "Software\Classes\*\shell\OpenWithSonarpad"
    DeleteRegValue SHCTX "${MANUPRODUCTKEY}" "ContextMenu"
    DeleteRegValue HKCU "${MANUPRODUCTKEY}" "ContextMenu"
    DeleteRegValue HKLM "${MANUPRODUCTKEY}" "ContextMenu"
  ${EndIf}
  {{/if}}

  ; Delete deep links
  {{#each deep_link_protocols as |protocol| ~}}
    ReadRegStr $R7 SHCTX "Software\Classes\\{{protocol}}\shell\open\command" ""
    !if $R7 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
      DeleteRegKey SHCTX "Software\Classes\\{{protocol}}"
    !endif
  {{/each}}

  ; Delete uninstaller
  Delete "$INSTDIR\uninstall.exe"

  {{#each resources_dirs}}
  RMDir /REBOOTOK "$INSTDIR\\{{this}}"
  {{/each}}
  RMDir "$INSTDIR"

  ; Remove start menu shortcut
  !insertmacro MUI_STARTMENU_GETFOLDER Application $AppStartMenuFolder
  Delete "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
  RMDir "$SMPROGRAMS\$AppStartMenuFolder"

  ; Remove desktop shortcuts
  Delete "$DESKTOP\${PRODUCTNAME}.lnk"

  ; Remove registry information for add/remove programs
  !if "${INSTALLMODE}" == "both"
    DeleteRegKey SHCTX "${UNINSTKEY}"
  !else if "${INSTALLMODE}" == "perMachine"
    DeleteRegKey HKLM "${UNINSTKEY}"
  !else
    DeleteRegKey HKCU "${UNINSTKEY}"
  !endif

  DeleteRegValue HKCU "${MANUPRODUCTKEY}" "Installer Language"

  ; Delete app data
  {{#if appdata_paths}}
  ${If} $DeleteAppDataCheckboxState == 1
      SetShellVarContext current
      {{#each appdata_paths}}
      RmDir /r "{{unescape_dollar_sign this}}"
      {{/each}}
  ${EndIf}
  {{/if}}

  ${GetOptions} $CMDLINE "/P" $R0
  IfErrors +2 0
    SetAutoClose true
SectionEnd

Function RestorePreviousInstallLocation
  ReadRegStr $4 SHCTX "${MANUPRODUCTKEY}" ""
  StrCmp $4 "" +2 0
    StrCpy $INSTDIR $4
FunctionEnd

Function SkipIfPassive
  ${IfThen} $PassiveMode == 1  ${|} Abort ${|}
FunctionEnd

Function CreateDesktopShortcut
  CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  ApplicationID::Set "$DESKTOP\${PRODUCTNAME}.lnk" "${IDENTIFIER}"
FunctionEnd

Function CreateStartMenuShortcut
  CreateDirectory "$SMPROGRAMS\$AppStartMenuFolder"
  CreateShortcut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  ApplicationID::Set "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "${IDENTIFIER}"
FunctionEnd
