# The acceptance test of tsf.test.acceptance. Run elevated, over SSH or a
# console, while the same user is signed in interactively:
#
#   1. builds kbdgen, the fixture's layout DLLs (`kbdgen target windows`),
#      the text service's DLLs under a test CLSID (`kbdgen tsf`) and kbdi
#      for that CLSID; compiles divvun-actions' text service installer
#      (divvun-tip.iss) under a test AppId, and the keyboard installer
#      divvun-actions generated for the fixture (install.all.iss), with its
#      payload staged as divvun-actions stages it
#   2. in the signed-in session, through a scheduled task: records the
#      user's languages, inputs and the registry and files the installers
#      touch; runs the keyboard installer silently, as a user would; types
#      every case through the profile kbdi enabled, into a Win32 EDIT, a
#      RichEdit and a WPF TextBox from 64-bit PowerShell and an EDIT and a
#      WPF TextBox from 32-bit PowerShell, and into an EDIT with the layout
#      DLL alone, and into a console (conhost) switched to the keyboard with
#      Win+Space; runs the keyboard's uninstaller silently; records the
#      state again
#   3. removes whatever is left, and reports it
#
# Output lines: LAYOUTDLL (the built x64 layout DLL, whose model gives the
# expected text), EXIT, INSTALLED, INPUT, SETUP, RESULT, RESTORE, BEFORE,
# AFTER, PENDING and LEFT lines, and CLEANUP done. tests/acceptance.rs
# checks them.
param(
  [string]$Phase = 'run',
  [string]$IssDir = '',
  [string]$Kbdi = '',
  [string]$CasesFile = '',
  [string]$Work = (Join-Path $env:TEMP 'kbd-tsf-acceptance'),
  [string]$Repo = ''
)
$ErrorActionPreference = 'Continue'
if (-not $Repo) { $Repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path }

$Clsid = '{4F600265-E72D-4131-8FBC-D56D08E17227}'
$TipAppGuid = 'DDE53BF3-BB8B-475E-B981-AD6B9C3E7F85'
$Iscc = Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'
$System32 = Join-Path $env:SystemRoot 'System32'
$SysWow64 = Join-Path $env:SystemRoot 'SysWOW64'
$LayoutKeys = 'HKLM:\SYSTEM\CurrentControlSet\Control\Keyboard Layouts'
$UninstallKeys = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall'
$TipDir = Join-Path $env:ProgramFiles 'Divvun\Text Service'
$Task = 'kbd-tsf-acceptance'
$Version = (Select-String -Path (Join-Path $Repo 'crates\kbd-tsf\Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$Iss = Join-Path $Work 'payload\install.all.iss'

# What the generated keyboard installer script says about itself: its
# AppId, application name, layout DLL and the product code kbdi gets.
function Get-Keyboard {
  $text = Get-Content -Raw -Encoding UTF8 $Iss
  @{
    AppId = '{' + [regex]::Match($text, 'AppId=\{\{([0-9A-Fa-f-]+)\}').Groups[1].Value + '}'
    App = [regex]::Match($text, 'DefaultDirName=\{pf\}\\(.+)').Groups[1].Value.Trim()
    Dll = [regex]::Match($text, '-d (\S+\.dll)').Groups[1].Value
    Product = '{' + [regex]::Match($text, 'keyboard_install [^\r\n]*-g ""\{\{([0-9A-Fa-f-]+)\}""').Groups[1].Value + '}'
  }
}

function Invoke-Interactive([string]$Script, [int]$TimeoutSec = 1800) {
  $out = Join-Path $env:TEMP 'kbd-tsf-acceptance.out'
  $wrapper = Join-Path $env:TEMP 'kbd-tsf-acceptance-task.ps1'
  Remove-Item -Force $out -ErrorAction SilentlyContinue
  # A line no output contains marks the end, not a word: `-match` ignores case.
  $done = '__KBD_TSF_ACCEPTANCE_DONE__'
  Set-Content -Encoding UTF8 $wrapper "& { $Script } *>&1 | Out-File -Encoding UTF8 '$out'; '$done' | Out-File -Append -Encoding UTF8 '$out'"
  $action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$wrapper`""
  $principal = New-ScheduledTaskPrincipal -UserId (whoami) -LogonType Interactive -RunLevel Highest
  Register-ScheduledTask -TaskName $Task -Action $action -Principal $principal -Force | Out-Null
  Start-ScheduledTask -TaskName $Task
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  while ((Get-Date) -lt $deadline) {
    if ((Test-Path $out) -and ((Get-Content $out) -contains $done)) { break }
    Start-Sleep -Milliseconds 500
  }
  Unregister-ScheduledTask -TaskName $Task -Confirm:$false
  if (-not (Test-Path $out)) { 'TIMEOUT no output from the interactive session' }
  elseif ((Get-Content $out) -notcontains $done) { Get-Content $out; 'TIMEOUT the interactive session did not finish' }
  else { Get-Content $out | Where-Object { $_ -ne $done } }
  Remove-Item -Force $out, $wrapper -ErrorAction SilentlyContinue
}

function Invoke-Checked([string]$What, [scriptblock]$Command) {
  $log = & $Command 2>&1
  if ($LASTEXITCODE -ne 0) { $log | Select-Object -Last 40; throw "$What failed with exit code $LASTEXITCODE" }
  "BUILT $What"
}

function Get-FreeGb { [math]::Round((Get-PSDrive C).Free / 1GB, 2) }

# Builds everything, compiles both installers and stages the keyboard
# installer's payload as divvun-actions' stageInstallerPayload,
# stageTipInstaller and stageWindInstaller do. Wind is a stub: its installer
# is divvun-actions' concern, not the text service's.
function Build {
  Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force $Work | Out-Null
  $manifest = Join-Path $Repo 'Cargo.toml'
  Invoke-Checked 'kbdgen' { cargo build --release --bin kbdgen --manifest-path $manifest }
  $kbdgen = Join-Path $Repo 'target\release\kbdgen.exe'
  # Outside $Work, which is removed at the end: the test reads the model
  # from the x64 DLL afterwards.
  $layout = Join-Path $Repo 'target\acceptance-layout'
  Invoke-Checked 'layout DLLs' { & $kbdgen target -b (Join-Path $PSScriptRoot 'fixture.kbdgen') -o $layout windows }
  $env:KBD_TSF_CLSID = $Clsid
  $tipDlls = Join-Path $Work 'tip-dlls'
  Invoke-Checked 'text service DLLs' { & $kbdgen tsf -w $Repo -o $tipDlls }
  Remove-Item Env:\KBD_TSF_CLSID
  $env:KBDI_TSF_CLSID = $Clsid
  $kbdiTarget = Join-Path $Repo 'target\acceptance-kbdi'
  foreach ($triple in 'x86_64-pc-windows-msvc', 'i686-pc-windows-msvc') {
    Invoke-Checked "kbdi $triple" { cargo build --locked --release --target $triple --bin kbdi --target-dir $kbdiTarget --manifest-path (Join-Path $Kbdi 'Cargo.toml') }
  }
  Remove-Item Env:\KBDI_TSF_CLSID
  Invoke-Checked 'text service installer' { & $Iscc /Qp "/DTipVersion=$Version" "/DPayloadDir=$tipDlls" "/DClsid=$Clsid" "/DAppGuid=$TipAppGuid" '/DOutputBaseFilename=divvun-tip' "/O$Work" (Join-Path $IssDir 'divvun-tip.iss') }

  $payload = Join-Path $Work 'payload'
  foreach ($variant in 'x86', 'x64', 'arm64', 'wow64') { Copy-Item -Recurse (Join-Path $layout $variant) (Join-Path $payload $variant) }
  Copy-Item (Join-Path $kbdiTarget 'i686-pc-windows-msvc\release\kbdi.exe') (Join-Path $payload 'kbdi.exe')
  Copy-Item (Join-Path $kbdiTarget 'x86_64-pc-windows-msvc\release\kbdi.exe') (Join-Path $payload 'kbdi-x64.exe')
  $tip = New-Item -ItemType Directory -Force (Join-Path $payload 'dependencies\divvun-tip')
  Copy-Item (Join-Path $Work 'divvun-tip.exe') (Join-Path $tip 'divvun-tip-installer.exe')
  $wind = New-Item -ItemType Directory -Force (Join-Path $payload 'dependencies\divvun-wind')
  Set-Content (Join-Path $wind 'divvun-wind-installer.exe') 'stub, never run'
  Set-Content (Join-Path $wind 'install-wind.ps1') 'exit 0'
  # An unsigned test build: no signing certificate on the test machine.
  Get-Content -Encoding UTF8 (Join-Path $IssDir 'install.all.iss') | Where-Object { $_ -notmatch '^(SignTool|SignedUninstaller)=' } | Set-Content -Encoding UTF8 $Iss
  Invoke-Checked 'keyboard installer' { & $Iscc /Qp "/O$Work" '/Fkeyboard' $Iss }
  'LAYOUTDLL ' + (Join-Path $layout ('x64\' + (Get-Keyboard).Dll))
}

# Files scheduled for deletion at the next restart, lowercased.
function Get-Pending {
  $ops = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager' -Name PendingFileRenameOperations -ErrorAction SilentlyContinue).PendingFileRenameOperations
  @($ops | Where-Object { $_ } | ForEach-Object { ($_ -replace '^\*?\d?\\\?\?\\', '').ToLower() })
}

# Every value under `key`, recursively, one line each.
function Get-Tree([string]$Key) {
  if (-not (Test-Path $Key)) { return "$Key absent" }
  foreach ($item in @(Get-Item $Key) + @(Get-ChildItem -Recurse $Key)) {
    $values = $item.GetValueNames() | Sort-Object | ForEach-Object { "$_=" + (@($item.GetValue($_, $null, 'DoNotExpandEnvironmentNames')) -join ',') }
    "$($item.Name) " + ($values -join ' ')
  }
}

# The state the installers may change: the user's languages and inputs,
# their registry, the welcome screen's, the text service's registration,
# every installed KLID, both uninstall entries and the installed files that
# are not already scheduled for deletion.
function Get-State {
  $keyboard = Get-Keyboard
  Get-WinUserLanguageList | ForEach-Object { "language $($_.LanguageTag) " + ($_.InputMethodTips -join ',') }
  foreach ($key in 'HKCU:\Keyboard Layout', 'HKCU:\Control Panel\International\User Profile', 'HKCU:\Software\Microsoft\CTF\SortOrder',
    'HKCU:\Software\Microsoft\CTF\HiddenDummyLayouts', "HKCU:\Software\Microsoft\CTF\TIP\$Clsid", 'Registry::HKEY_USERS\.DEFAULT\Keyboard Layout',
    "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$Clsid", "HKLM:\SOFTWARE\Classes\CLSID\$Clsid", "HKLM:\SOFTWARE\WOW6432Node\Classes\CLSID\$Clsid",
    "$UninstallKeys\$($keyboard.AppId)_is1", "$UninstallKeys\{$TipAppGuid}_is1") {
    Get-Tree $key | ForEach-Object { "registry $_" }
  }
  Get-ChildItem $LayoutKeys | ForEach-Object {
    $p = Get-ItemProperty $_.PSPath
    "klid $($_.PSChildName) $($p.'Layout File') $($p.'Layout Id') $($p.'Layout Product Code')"
  }
  $pending = Get-Pending
  $files = @("$System32\$($keyboard.Dll)", "$SysWow64\$($keyboard.Dll)", (Join-Path $env:ProgramFiles $keyboard.App), (Join-Path ${env:ProgramFiles(x86)} $keyboard.App),
    (Join-Path $env:ProgramData "Microsoft\Windows\Start Menu\Programs\$($keyboard.App)"), $TipDir)
  foreach ($root in $files) {
    foreach ($file in @(Get-Item $root -ErrorAction SilentlyContinue) + @(Get-ChildItem -Recurse $root -ErrorAction SilentlyContinue)) {
      if ($pending -notcontains $file.FullName.ToLower()) { "file $($file.FullName)" }
    }
  }
}

# Runs an Inno installer or uninstaller silently and waits until `done`
# holds: an uninstaller hands over to a copy of itself and exits at once.
function Invoke-Inno([string]$Exe, [string]$Log, [scriptblock]$Done) {
  $process = Start-Process -Wait -PassThru $Exe -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', "/LOG=`"$Log`""
  for ($i = 0; $i -lt 240 -and -not (& $Done); $i++) { Start-Sleep -Milliseconds 500 }
  "EXIT $(Split-Path -Leaf $Exe) $($process.ExitCode) finished=$(& $Done)"
}

# Runs the keyboard installer and reports, after `label`, the text
# service's registration and any of its DLLs whose path is to be deleted at
# the next restart, which would lose a DLL installed before then.
function Install-Keyboard([string]$Label, [string]$AppKey) {
  Invoke-Inno (Join-Path $Work 'keyboard.exe') (Join-Path $Work "$Label.log") { Test-Path $AppKey }
  $pending = Get-Pending
  foreach ($view in @{ Name = 'server64'; Key = "HKLM:\SOFTWARE\Classes\CLSID\$Clsid" }, @{ Name = 'server32'; Key = "HKLM:\SOFTWARE\WOW6432Node\Classes\CLSID\$Clsid" }) {
    $server = (Get-ItemProperty "$($view.Key)\InprocServer32" -ErrorAction SilentlyContinue).'(default)'
    "$Label $($view.Name) $server"
    if ($server -and $pending -contains $server.ToLower()) { "$Label pending $server" }
  }
}

function Uninstall-Keyboard([string]$AppKey) {
  $uninstaller = (Get-ItemProperty $AppKey -ErrorAction SilentlyContinue).UninstallString
  if ($uninstaller) {
    Invoke-Inno $uninstaller.Trim('"') (Join-Path $Work 'uninstall.log') { -not (Test-Path $AppKey) -and -not (Test-Path "$UninstallKeys\{$TipAppGuid}_is1") }
  }
}

# Types every case through the installed keyboard's profile, under the
# LANGID kbdi enabled it with, and with its layout DLL alone.
function Type-Cases($Keyboard) {
  $klid = Get-ChildItem $LayoutKeys | Where-Object { (Get-ItemProperty $_.PSPath).'Layout File' -eq $Keyboard.Dll } | Select-Object -First 1
  "INSTALLED klid $($klid.PSChildName) $((Get-ItemProperty $klid.PSPath).'Layout Product Code')"
  Get-ChildItem "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$Clsid\LanguageProfile" -ErrorAction SilentlyContinue | Get-ChildItem | ForEach-Object {
    "INSTALLED profile $(Split-Path -Leaf $_.PSParentPath) $($_.PSChildName) enable=$((Get-ItemProperty $_.PSPath).Enable)"
  }
  $tip = Get-WinUserLanguageList | ForEach-Object { $_.InputMethodTips | ForEach-Object { $_ } } | Where-Object { $_ -like "*:$Clsid$($Keyboard.Product)" } | Select-Object -First 1
  "INPUT $tip"
  if (-not $tip) { return }
  $lang = [Convert]::ToInt32($tip.Split(':')[0], 16)
  $typer = Join-Path $PSScriptRoot '..\vm\type.ps1'
  $common = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $typer, '-Clsid', $Clsid, '-ProfileGuid', $Keyboard.Product, '-LangId', $lang, '-CasesFile', $CasesFile)
  # The layout DLL's run goes first: once another process of this task
  # has switched the session's input, by ActivateProfile or
  # UnloadKeyboardLayout, AltGr chords sent with SendInput type nothing
  # with the layout DLL alone until the task ends.
  & "$System32\WindowsPowerShell\v1.0\powershell.exe" @common -Kinds edit -Klid $klid.PSChildName
  & "$System32\WindowsPowerShell\v1.0\powershell.exe" @common -Kinds edit,rich,wpf
  & "$SysWow64\WindowsPowerShell\v1.0\powershell.exe" @common -Kinds edit,wpf
  $reader = Join-Path $Work 'reader.exe'
  Add-Type -Path (Join-Path $PSScriptRoot 'ConsoleReader.cs') -OutputAssembly $reader -OutputType ConsoleApplication
  & "$System32\WindowsPowerShell\v1.0\powershell.exe" @common -Kinds console -Reader $reader
}

# Step 2 of the header, in the signed-in session. Between typing and the
# final uninstall, a process keeps the text service's DLL loaded through an
# uninstall, a reinstall and its release, as Explorer does once the
# keyboard was active there.
function Session {
  $keyboard = Get-Keyboard
  Get-State | ForEach-Object { "BEFORE $_" }
  $pendingBefore = Get-Pending
  $appKey = "$UninstallKeys\$($keyboard.AppId)_is1"
  Install-Keyboard 'INSTALLED' $appKey
  Type-Cases $keyboard
  $server = (Get-ItemProperty "HKLM:\SOFTWARE\Classes\CLSID\$Clsid\InprocServer32" -ErrorAction SilentlyContinue).'(default)'
  $release = Join-Path $Work 'release'
  $holder = Start-Process -PassThru -WindowStyle Hidden "$System32\WindowsPowerShell\v1.0\powershell.exe" -ArgumentList '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$(Join-Path $PSScriptRoot 'hold.ps1')`"", '-Dll', "`"$server`"", '-Release', "`"$release`""
  for ($i = 0; $i -lt 50 -and -not ($holder.Modules.FileName -contains $server); $i++) { Start-Sleep -Milliseconds 200; $holder.Refresh() }
  "HOLDER loaded=$($holder.Modules.FileName -contains $server)"
  Uninstall-Keyboard $appKey
  Install-Keyboard 'REINSTALLED' $appKey
  Set-Content $release ''
  $holder.WaitForExit(10000) | Out-Null
  Uninstall-Keyboard $appKey
  Start-Sleep -Seconds 5
  Get-State | ForEach-Object { "AFTER $_" }
  $pending = Get-Pending
  foreach ($file in Get-ChildItem -Recurse -File $TipDir -ErrorAction SilentlyContinue) {
    if ($pending -contains $file.FullName.ToLower()) { "PENDING $($file.FullName)" }
  }
  # The DLLs the uninstall moved aside, now that nothing holds them.
  $temp = (Join-Path $env:SystemRoot 'Temp\').ToLower()
  foreach ($file in $pending | Where-Object { $pendingBefore -notcontains $_ -and $_.StartsWith($temp) }) {
    Remove-Item -Force $file -ErrorAction SilentlyContinue
    "DISCARDED $file removed=$(-not (Test-Path $file))"
  }
}

# Uninstalls whatever a failed run left: the keyboard, then the text
# service, then any profile, registration or file of the test CLSID.
function Cleanup {
  $keyboard = Get-Keyboard
  $appKey = "$UninstallKeys\$($keyboard.AppId)_is1"
  $tipKey = "$UninstallKeys\{$TipAppGuid}_is1"
  foreach ($key in $appKey, $tipKey) {
    $uninstaller = (Get-ItemProperty $key -ErrorAction SilentlyContinue).UninstallString
    if ($uninstaller) { "CLEANUP uninstall $key"; Invoke-Inno $uninstaller.Trim('"') (Join-Path $env:TEMP 'kbd-tsf-acceptance-cleanup.log') { -not (Test-Path $key) } }
  }
  foreach ($kbdi in "$Repo\target\acceptance-kbdi\x86_64-pc-windows-msvc\release\kbdi.exe") {
    if ((Test-Path $kbdi) -and (Test-Path "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$Clsid\LanguageProfile")) { & $kbdi keyboard_uninstall $keyboard.Product 2>&1 | Out-Null; "CLEANUP kbdi keyboard_uninstall $($keyboard.Product)" }
  }
  foreach ($view in @{ Key = "HKLM:\SOFTWARE\Classes\CLSID\$Clsid"; Exe = "$System32\regsvr32.exe" }, @{ Key = "HKLM:\SOFTWARE\WOW6432Node\Classes\CLSID\$Clsid"; Exe = "$SysWow64\regsvr32.exe" }) {
    $server = (Get-ItemProperty "$($view.Key)\InprocServer32" -ErrorAction SilentlyContinue).'(default)'
    if ($server) { "CLEANUP unregister $server"; Start-Process -Wait $view.Exe -ArgumentList '/s', '/u', "`"$server`"" }
  }
  Remove-Item -Recurse -Force "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$Clsid" -ErrorAction SilentlyContinue
  if (Get-ScheduledTask -TaskName $Task -ErrorAction SilentlyContinue) { Unregister-ScheduledTask -TaskName $Task -Confirm:$false }
  $left = @("HKLM:\SOFTWARE\Classes\CLSID\$Clsid", "HKLM:\SOFTWARE\WOW6432Node\Classes\CLSID\$Clsid", "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$Clsid",
    $appKey, $tipKey, "$System32\$($keyboard.Dll)", "$SysWow64\$($keyboard.Dll)", (Join-Path $env:ProgramFiles $keyboard.App)) | Where-Object { Test-Path $_ }
  $left += Get-ChildItem $LayoutKeys | Where-Object { (Get-ItemProperty $_.PSPath).'Layout File' -eq $keyboard.Dll } | ForEach-Object { $_.Name }
  foreach ($item in $left) { "LEFT $item" }
}

# `run` is the test. The other phases are its parts, for investigating a
# failure by hand: `build`, then `session` from the signed-in session, then
# `cleanup`.
switch ($Phase) {
  'build' { Build }
  'session' { Session }
  'cleanup' { Cleanup; Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue }
  'run' {
    $quser = (query user 2>&1) -join ' '
    if ($quser -notmatch 'Active') { 'SKIPPED nobody is signed in interactively'; exit 0 }
    $free = Get-FreeGb
    "FREE GB $free"
    if ($free -lt 2) { "SKIPPED only $free GB free"; exit 0 }
    try {
      Build
      "FREE GB $(Get-FreeGb)"
      Invoke-Interactive "& '$PSCommandPath' -Phase session -Work '$Work' -CasesFile '$CasesFile' -Repo '$Repo'"
    }
    finally {
      if (Test-Path $Iss) { Cleanup }
      Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
      'CLEANUP done'
    }
  }
}
