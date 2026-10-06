# The VM integration test of tsf.test.vm. Run elevated, over SSH or a
# console, while the same user is signed in interactively:
#
#   1. builds kbd-tsf for x64 and x86 under a test CLSID, with the C
#      runtime linked statically as `kbdgen tsf` builds it; checks that
#      DllRegisterServer refuses a misnamed DLL, one outside a version
#      directory, one outside %ProgramFiles% and one without the restricted
#      packages' grant; registers both builds from one version directory
#      under %ProgramFiles% and reports the registry it wrote, and that an
#      unregistration from another directory leaves it; for each fixture
#      layout (Võro, and the emoji layout of
#      tsf.test.emoji) installs its layout DLLs under a test KLID whose
#      Layout Product Code is its test profile, and registers the profile
#      under a test LANGID
#   2. in the signed-in session, through a scheduled task, types each
#      layout's cases into a Win32 EDIT, a RichEdit and a WPF TextBox from
#      64-bit PowerShell, into an EDIT and a WPF TextBox from 32-bit
#      PowerShell, and into an EDIT with the layout DLL alone, for
#      comparison
#   3. removes every registration, file and task, and reports what is left
#
# Output lines: EXPORTS, DEPENDS, REFUSE, SERVER, CATEGORIES, STALE and
# UNREGISTERED lines for the builds and their registration, SETUP and RESULT
# lines from the driver, LEFT lines for anything cleanup could not remove.
# tests/vm.rs checks them.
param(
  [Parameter(Mandatory)][string]$LayoutDir,
  [Parameter(Mandatory)][string]$CasesFile,
  [Parameter(Mandatory)][string]$EmojiCasesFile,
  [string]$Repo = ''
)
$ErrorActionPreference = 'Continue'
if (-not $Repo) { $Repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path }

$Clsid = '{569BD944-16EA-4CE6-817F-DFEEFD1D5F92}'
$LangId = 0x0409
$Layouts = @(
  @{ Profile = '{94C71262-EE9D-489B-926C-159381558D90}'; Klid = 'a0f10409'; Id = '00f1'; File = 'kbdtsft.dll'; Text = 'kbd-tsf VM test layout'; Cases = $CasesFile },
  @{ Profile = '{2EF56555-902F-4B4A-AE5E-7103C1569BB6}'; Klid = 'a0f20409'; Id = '00f2'; File = 'kbdtsfe.dll'; Text = 'kbd-tsf VM emoji layout'; Cases = $EmojiCasesFile }
)
$Version = (Select-String -Path (Join-Path $Repo 'crates\kbd-tsf\Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$Root = Join-Path $env:ProgramFiles 'DivvunTsfVmTest'
$Install = Join-Path $Root $Version
$X64Dll = Join-Path $Install 'divvun_tip_x64.dll'
$X86Dll = Join-Path $Install 'divvun_tip_x86.dll'
$Outside = Join-Path $env:TEMP 'kbd-tsf-vm-outside'
$Refusals = @(
  @{ Name = 'name'; Path = "$Root\refuse-name\$Version\kbd_tsf.dll" },
  @{ Name = 'version'; Path = "$Root\refuse-version\divvun_tip_x64.dll" },
  @{ Name = 'location'; Path = "$Outside\$Version\divvun_tip_x64.dll" },
  @{ Name = 'acl'; Path = "$Root\refuse-acl\$Version\divvun_tip_x64.dll" }
)
$Stale = "$Root\stale\$Version\divvun_tip_x64.dll"
$ClassKey = "HKLM:\SOFTWARE\Classes\CLSID\$Clsid"
$WowClassKey = "HKLM:\SOFTWARE\WOW6432Node\Classes\CLSID\$Clsid"
$Task = 'kbd-tsf-vm'
$LayoutKeys = 'HKLM:\SYSTEM\CurrentControlSet\Control\Keyboard Layouts'
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
    "PENDING $Path is loaded by " + ((tasklist /m (Split-Path -Leaf $Path) /fo csv | ConvertFrom-Csv | ForEach-Object { $_.'Image Name' }) -join ',') + "; moved to $moved and deleted at restart"
  }
}

function Show-Exports([string]$Dll, [string]$Arch) {
  $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
  $dumpbin = & $vswhere -latest -find 'VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe' | Select-Object -First 1
  $names = & $dumpbin /nologo /exports $Dll | ForEach-Object {
    if ($_ -match '^\s+\d+\s+[0-9A-F]+\s+[0-9A-F]+\s+(\S+)') { $Matches[1] }
  }
  "EXPORTS $Arch " + (($names | Sort-Object) -join ',')
  $depends = & $dumpbin /nologo /dependents $Dll | ForEach-Object {
    if ($_ -match '^\s+(\S+\.dll)\s*$') { $Matches[1].ToLower() }
  }
  "DEPENDS $Arch " + (($depends | Sort-Object) -join ',')
}

# The TSF categories registered under the CLSID, sorted, or none.
function Get-Categories {
  $key = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$Clsid\Category\Category"
  $names = @(Get-ChildItem $key -ErrorAction SilentlyContinue | ForEach-Object { $_.PSChildName.ToUpper() } | Sort-Object)
  if ($names) { $names -join ',' } else { 'none' }
}

function Get-Server([string]$Key) {
  $server = Get-ItemProperty "$Key\InprocServer32" -ErrorAction SilentlyContinue
  if ($server) { $server.'(default)' + ' ' + $server.ThreadingModel } else { 'none' }
}

# DllRegisterServer of the x64 build copied to each refused place, which
# must fail and leave no CLSID behind.
function Test-Refusals([string]$Dll) {
  foreach ($refusal in $Refusals) {
    New-Item -ItemType Directory -Force (Split-Path $refusal.Path) | Out-Null
    Copy-Item -Force $Dll $refusal.Path
    if ($refusal.Name -eq 'acl') {
      icacls $refusal.Path /inheritance:d | Out-Null
      icacls $refusal.Path /remove:g '*S-1-15-2-2' | Out-Null
    }
    $hr = [TsfDriver]::CallExport($refusal.Path, 'DllRegisterServer')
    'REFUSE {0} 0x{1:x8} registered={2}' -f $refusal.Name, $hr, ((Test-Path $ClassKey) -or (Test-Path $WowClassKey))
    Remove-Item -Force $refusal.Path
  }
}

$quser = (query user 2>&1) -join ' '
if ($quser -notmatch 'Active') { 'SKIPPED nobody is signed in interactively'; exit 0 }

try {
  $env:KBD_TSF_CLSID = $Clsid
  $targetDir = Join-Path $Repo 'target\tsf-vm'
  foreach ($triple in 'x86_64-pc-windows-msvc', 'i686-pc-windows-msvc') {
    $log = & cargo rustc --release -p kbd-tsf --lib --target $triple --target-dir $targetDir --manifest-path (Join-Path $Repo 'Cargo.toml') -- -C target-feature=+crt-static 2>&1
    if ($LASTEXITCODE -ne 0) { $log; throw "cargo build for $triple failed" }
  }
  $x64 = Join-Path $targetDir 'x86_64-pc-windows-msvc\release\kbd_tsf.dll'
  $x86 = Join-Path $targetDir 'i686-pc-windows-msvc\release\kbd_tsf.dll'
  Show-Exports $x64 'x64'
  Show-Exports $x86 'x86'

  Add-Type -Path "$PSScriptRoot\TsfDriver.cs" -ReferencedAssemblies System.Windows.Forms, PresentationFramework, PresentationCore, WindowsBase, System.Xaml
  Test-Refusals $x64

  foreach ($dll in $X64Dll, $X86Dll) { Remove-Locked $dll }
  New-Item -ItemType Directory -Force $Install | Out-Null
  Copy-Item -Force $x64 $X64Dll
  Copy-Item -Force $x86 $X86Dll
  "INSTALL $Install"
  'REGISTER x86 ' + (Invoke-Regsvr32 "$SysWow64\regsvr32.exe" $X86Dll)
  'CATEGORIES x86 ' + (Get-Categories)
  'REGISTER x64 ' + (Invoke-Regsvr32 "$System32\regsvr32.exe" $X64Dll)
  'SERVER 64 ' + (Get-Server $ClassKey)
  'SERVER 32 ' + (Get-Server $WowClassKey)
  'CATEGORIES both ' + (Get-Categories)
  New-Item -ItemType Directory -Force (Split-Path $Stale) | Out-Null
  Copy-Item -Force $x64 $Stale
  'STALE 0x{0:x8} {1}' -f [TsfDriver]::CallExport($Stale, 'DllUnregisterServer'), (Get-Server $ClassKey)
  Remove-Item -Force $Stale
  $typer = Join-Path $PSScriptRoot 'type.ps1'
  $ps64 = "$System32\WindowsPowerShell\v1.0\powershell.exe"
  $ps32 = "$SysWow64\WindowsPowerShell\v1.0\powershell.exe"
  $runs = foreach ($layout in $Layouts) {
    $file = $layout.File
    Copy-Item -Force (Join-Path $LayoutDir "x64\$file") (Join-Path $System32 $file)
    Copy-Item -Force (Join-Path $LayoutDir "wow64\$file") (Join-Path $SysWow64 $file)
    $key = Join-Path $LayoutKeys $layout.Klid
    New-Item -Force $key | Out-Null
    Set-ItemProperty $key 'Layout File' $file
    Set-ItemProperty $key 'Layout Text' $layout.Text
    Set-ItemProperty $key 'Layout Id' $layout.Id
    Set-ItemProperty $key 'Layout Product Code' $layout.Profile
    Set-ItemProperty $key 'Layout Display Name' "@%SystemRoot%\system32\$file,-1000"
    'PROFILE register {0} 0x{1:x}' -f $layout.Profile, [TsfDriver]::Register($Clsid, $LangId, $layout.Profile, $layout.Text, (Join-Path $System32 $file))

    $common = "-Clsid '$Clsid' -ProfileGuid '$($layout.Profile)' -LangId $LangId -CasesFile '$($layout.Cases)'"
    "& '$ps64' -NoProfile -ExecutionPolicy Bypass -File '$typer' -Kinds edit $common -Klid $($layout.Klid)"
    "& '$ps64' -NoProfile -ExecutionPolicy Bypass -File '$typer' -Kinds edit,rich,wpf $common"
    "& '$ps32' -NoProfile -ExecutionPolicy Bypass -File '$typer' -Kinds edit,wpf $common"
  }
  $runs | Where-Object { $_ -like 'PROFILE *' }
  Invoke-Interactive (($runs | Where-Object { $_ -like '& *' }) -join "`n")
}
finally {
  if ('TsfDriver' -as [type]) {
    foreach ($layout in $Layouts) {
      'PROFILE unregister {0} 0x{1:x}' -f $layout.Profile, [TsfDriver]::Unregister($Clsid, $LangId, $layout.Profile)
    }
  }
  if (Test-Path $X86Dll) { 'UNREGISTER x86 ' + (Invoke-Regsvr32 "$SysWow64\regsvr32.exe" $X86Dll '/u') }
  'CATEGORIES x86-removed ' + (Get-Categories)
  if (Test-Path $X64Dll) { 'UNREGISTER x64 ' + (Invoke-Regsvr32 "$System32\regsvr32.exe" $X64Dll '/u') }
  'UNREGISTERED 64={0} 32={1} tip={2}' -f (Test-Path $ClassKey), (Test-Path $WowClassKey), (Test-Path "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$Clsid")
  Remove-Item -Recurse -Force "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$Clsid" -ErrorAction SilentlyContinue
  foreach ($layout in $Layouts) {
    Remove-Item -Recurse -Force (Join-Path $LayoutKeys $layout.Klid) -ErrorAction SilentlyContinue
    foreach ($file in (Join-Path $System32 $layout.File), (Join-Path $SysWow64 $layout.File)) {
      Remove-Item -Force $file -ErrorAction SilentlyContinue
    }
  }
  foreach ($dll in $X64Dll, $X86Dll) { Remove-Locked $dll }
  Remove-Item -Recurse -Force $Root, $Outside -ErrorAction SilentlyContinue
  if (Get-ScheduledTask -TaskName $Task -ErrorAction SilentlyContinue) { Unregister-ScheduledTask -TaskName $Task -Confirm:$false }
  Remove-Item Env:\KBD_TSF_CLSID -ErrorAction SilentlyContinue

  $left = @(
    "HKLM:\SOFTWARE\Classes\CLSID\$Clsid",
    "HKLM:\SOFTWARE\WOW6432Node\Classes\CLSID\$Clsid",
    "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$Clsid",
    $Root,
    $Outside
  ) + @(foreach ($layout in $Layouts) {
    (Join-Path $LayoutKeys $layout.Klid)
    (Join-Path $System32 $layout.File)
    (Join-Path $SysWow64 $layout.File)
  }) | Where-Object { Test-Path $_ }
  foreach ($item in $left) { "LEFT $item" }
  'CLEANUP done'
}
