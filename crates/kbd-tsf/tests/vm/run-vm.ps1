# The VM integration test of tsf.test.vm. Run elevated, over SSH or a
# console, while the same user is signed in interactively:
#
#   1. builds kbd-tsf for x64 and x86 under a test CLSID and registers both
#      builds, installs the fixture layout DLLs under a test KLID whose
#      Layout Product Code is the test profile, and registers the profile
#      under a test LANGID
#   2. in the signed-in session, through a scheduled task, types the cases
#      into a Win32 EDIT, a RichEdit and a WPF TextBox from 64-bit
#      PowerShell, into an EDIT and a WPF TextBox from 32-bit PowerShell,
#      and into an EDIT with the layout DLL alone, for comparison
#   3. removes every registration, file and task, and reports what is left
#
# Output lines: EXPORTS, SETUP and RESULT lines from the driver, LEFT lines
# for anything cleanup could not remove. tests/vm.rs checks them.
param(
  [Parameter(Mandatory)][string]$LayoutDir,
  [Parameter(Mandatory)][string]$CasesFile,
  [string]$Repo = ''
)
$ErrorActionPreference = 'Continue'
if (-not $Repo) { $Repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path }

$Clsid = '{569BD944-16EA-4CE6-817F-DFEEFD1D5F92}'
$ProfileGuid = '{94C71262-EE9D-489B-926C-159381558D90}'
$LangId = 0x0409
$Klid = 'a0f10409'
$LayoutId = '00f1'
$LayoutFile = 'kbdtsft.dll'
$Install = Join-Path $env:ProgramFiles 'DivvunTsfVmTest'
$Task = 'kbd-tsf-vm'
$LayoutKey = "HKLM:\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\$Klid"
$System32 = Join-Path $env:SystemRoot 'System32'
$SysWow64 = Join-Path $env:SystemRoot 'SysWOW64'

function Invoke-Interactive([string]$Script, [int]$TimeoutSec = 600) {
  $out = Join-Path $env:TEMP 'kbd-tsf-vm.out'
  $wrapper = Join-Path $env:TEMP 'kbd-tsf-vm-task.ps1'
  Remove-Item -Force $out -ErrorAction SilentlyContinue
  Set-Content -Encoding UTF8 $wrapper "& { $Script } *>&1 | Out-File -Encoding UTF8 '$out'; 'DONE' | Out-File -Append -Encoding UTF8 '$out'"
  $action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$wrapper`""
  $principal = New-ScheduledTaskPrincipal -UserId (whoami) -LogonType Interactive -RunLevel Highest
  Register-ScheduledTask -TaskName $Task -Action $action -Principal $principal -Force | Out-Null
  Start-ScheduledTask -TaskName $Task
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  while ((Get-Date) -lt $deadline) {
    if ((Test-Path $out) -and ((Get-Content -Raw $out) -match 'DONE')) { break }
    Start-Sleep -Milliseconds 500
  }
  Unregister-ScheduledTask -TaskName $Task -Confirm:$false
  if (Test-Path $out) { Get-Content $out | Where-Object { $_ -ne 'DONE' } } else { 'TIMEOUT no output from the interactive session' }
  Remove-Item -Force $out, $wrapper -ErrorAction SilentlyContinue
}

# regsvr32 is a GUI program, which PowerShell does not wait for.
function Invoke-Regsvr32([string]$Exe, [string]$Dll, [string]$Verb = '') {
  $arguments = @('/s') + @($Verb | Where-Object { $_ }) + @("`"$Dll`"")
  (Start-Process -Wait -PassThru -FilePath $Exe -ArgumentList $arguments).ExitCode
}

# A loaded text service DLL cannot be deleted. TSF loads it into processes
# such as explorer.exe once the profile is active in the session, and they
# keep it until they unload it; wait for that, then fall back to deleting
# it at the next restart.
function Remove-Locked([string]$Path) {
  for ($i = 0; $i -lt 60 -and (Test-Path $Path); $i++) {
    Remove-Item -Force $Path -ErrorAction SilentlyContinue
    if (Test-Path $Path) { Start-Sleep -Seconds 1 }
  }
  if (Test-Path $Path) {
    $moved = Join-Path $env:TEMP ('kbd-tsf-vm-' + [guid]::NewGuid() + '.dll')
    Move-Item -Force $Path $moved
    Add-Type -Namespace K -Name M -MemberDefinition '[DllImport("kernel32.dll", CharSet=CharSet.Unicode)] public static extern bool MoveFileEx(string from, string to, int flags);'
    [void][K.M]::MoveFileEx($moved, $null, 4)
    "PENDING $Path is loaded by " + ((tasklist /m kbd_tsf.dll /fo csv | ConvertFrom-Csv | ForEach-Object { $_.'Image Name' }) -join ',') + "; moved to $moved and deleted at restart"
  }
}

function Show-Exports([string]$Dll, [string]$Arch) {
  $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
  $dumpbin = & $vswhere -latest -find 'VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe' | Select-Object -First 1
  $names = & $dumpbin /nologo /exports $Dll | ForEach-Object {
    if ($_ -match '^\s+\d+\s+[0-9A-F]+\s+[0-9A-F]+\s+(\S+)') { $Matches[1] }
  }
  "EXPORTS $Arch " + (($names | Sort-Object) -join ',')
}

$quser = (query user 2>&1) -join ' '
if ($quser -notmatch 'Active') { 'SKIPPED nobody is signed in interactively'; exit 0 }

try {
  $env:KBD_TSF_CLSID = $Clsid
  $targetDir = Join-Path $Repo 'target\tsf-vm'
  foreach ($triple in 'x86_64-pc-windows-msvc', 'i686-pc-windows-msvc') {
    $log = & cargo build --release -p kbd-tsf --target $triple --target-dir $targetDir --manifest-path (Join-Path $Repo 'Cargo.toml') 2>&1
    if ($LASTEXITCODE -ne 0) { $log; throw "cargo build for $triple failed" }
  }
  $x64 = Join-Path $targetDir 'x86_64-pc-windows-msvc\release\kbd_tsf.dll'
  $x86 = Join-Path $targetDir 'i686-pc-windows-msvc\release\kbd_tsf.dll'
  Show-Exports $x64 'x64'
  Show-Exports $x86 'x86'

  foreach ($arch in 'x64', 'x86') { Remove-Locked "$Install\$arch\kbd_tsf.dll" }
  New-Item -ItemType Directory -Force "$Install\x64", "$Install\x86" | Out-Null
  Copy-Item -Force $x64 "$Install\x64\kbd_tsf.dll"
  Copy-Item -Force $x86 "$Install\x86\kbd_tsf.dll"
  'REGISTER x64 ' + (Invoke-Regsvr32 "$System32\regsvr32.exe" "$Install\x64\kbd_tsf.dll")
  'REGISTER x86 ' + (Invoke-Regsvr32 "$SysWow64\regsvr32.exe" "$Install\x86\kbd_tsf.dll")

  Copy-Item -Force (Join-Path $LayoutDir "x64\$LayoutFile") (Join-Path $System32 $LayoutFile)
  Copy-Item -Force (Join-Path $LayoutDir "wow64\$LayoutFile") (Join-Path $SysWow64 $LayoutFile)
  New-Item -Force $LayoutKey | Out-Null
  Set-ItemProperty $LayoutKey 'Layout File' $LayoutFile
  Set-ItemProperty $LayoutKey 'Layout Text' 'kbd-tsf VM test layout'
  Set-ItemProperty $LayoutKey 'Layout Id' $LayoutId
  Set-ItemProperty $LayoutKey 'Layout Product Code' $ProfileGuid
  Set-ItemProperty $LayoutKey 'Layout Display Name' "@%SystemRoot%\system32\$LayoutFile,-1000"

  Add-Type -Path "$PSScriptRoot\TsfDriver.cs" -ReferencedAssemblies System.Windows.Forms, PresentationFramework, PresentationCore, WindowsBase, System.Xaml
  'PROFILE register 0x{0:x}' -f [TsfDriver]::Register($Clsid, $LangId, $ProfileGuid, 'kbd-tsf VM test', (Join-Path $System32 $LayoutFile))

  $typer = Join-Path $PSScriptRoot 'type.ps1'
  $ps64 = "$System32\WindowsPowerShell\v1.0\powershell.exe"
  $ps32 = "$SysWow64\WindowsPowerShell\v1.0\powershell.exe"
  $common = "-Clsid '$Clsid' -ProfileGuid '$ProfileGuid' -LangId $LangId -CasesFile '$CasesFile'"
  Invoke-Interactive @"
& '$ps64' -NoProfile -ExecutionPolicy Bypass -File '$typer' -Kinds edit $common -Klid $Klid
& '$ps64' -NoProfile -ExecutionPolicy Bypass -File '$typer' -Kinds edit,rich,wpf $common
& '$ps32' -NoProfile -ExecutionPolicy Bypass -File '$typer' -Kinds edit,wpf $common
"@
}
finally {
  if ('TsfDriver' -as [type]) {
    'PROFILE unregister 0x{0:x}' -f [TsfDriver]::Unregister($Clsid, $LangId, $ProfileGuid)
  }
  if (Test-Path "$Install\x64\kbd_tsf.dll") { 'UNREGISTER x64 ' + (Invoke-Regsvr32 "$System32\regsvr32.exe" "$Install\x64\kbd_tsf.dll" '/u') }
  if (Test-Path "$Install\x86\kbd_tsf.dll") { 'UNREGISTER x86 ' + (Invoke-Regsvr32 "$SysWow64\regsvr32.exe" "$Install\x86\kbd_tsf.dll" '/u') }
  Remove-Item -Recurse -Force "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$Clsid" -ErrorAction SilentlyContinue
  Remove-Item -Recurse -Force $LayoutKey -ErrorAction SilentlyContinue
  foreach ($file in (Join-Path $System32 $LayoutFile), (Join-Path $SysWow64 $LayoutFile)) {
    Remove-Item -Force $file -ErrorAction SilentlyContinue
  }
  foreach ($arch in 'x64', 'x86') { Remove-Locked "$Install\$arch\kbd_tsf.dll" }
  Remove-Item -Recurse -Force $Install -ErrorAction SilentlyContinue
  if (Get-ScheduledTask -TaskName $Task -ErrorAction SilentlyContinue) { Unregister-ScheduledTask -TaskName $Task -Confirm:$false }
  Remove-Item Env:\KBD_TSF_CLSID -ErrorAction SilentlyContinue

  $left = @(
    "HKLM:\SOFTWARE\Classes\CLSID\$Clsid",
    "HKLM:\SOFTWARE\WOW6432Node\Classes\CLSID\$Clsid",
    "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$Clsid",
    $LayoutKey,
    (Join-Path $System32 $LayoutFile),
    (Join-Path $SysWow64 $LayoutFile),
    $Install
  ) | Where-Object { Test-Path $_ }
  foreach ($item in $left) { "LEFT $item" }
  'CLEANUP done'
}
