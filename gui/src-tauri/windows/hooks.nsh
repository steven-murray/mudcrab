; Installer hooks for the GUI's Windows installer (bundle.windows.nsis in
; tauri.conf.json).
;
; The command line, mudcrab.exe, is installed beside the GUI (see
; tauri.cli.conf.json), and the install folder goes on the user's PATH, so
; `mudcrab` runs from any terminal opened afterwards. Uninstalling takes it
; off again. PowerShell edits the variable: NSIS has nothing for a
; `;`-separated list, and SetEnvironmentVariable tells running programs
; that it changed. The folder reaches PowerShell through the environment, not
; the command line, so a quote in it (a user called O'Brien) cannot break the
; command.

!macro MUDCRAB_PATH ACTION
  System::Call 'Kernel32::SetEnvironmentVariable(t "MUDCRAB_DIR", t "$INSTDIR")'
  nsExec::Exec `powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$d = $$env:MUDCRAB_DIR; $$parts = @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';' | Where-Object { $$_ -and $$_ -ne $$d }); if ('${ACTION}' -eq 'add') { $$parts += $$d }; [Environment]::SetEnvironmentVariable('Path', ($$parts -join ';'), 'User')"`
  Pop $0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro MUDCRAB_PATH add
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  !insertmacro MUDCRAB_PATH remove
!macroend
