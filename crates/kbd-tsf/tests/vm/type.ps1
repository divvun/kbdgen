# Types the cases of -CasesFile into each control kind of -Kinds, in this
# PowerShell process (64- or 32-bit), and prints the driver's lines. The
# kind `console` types into console windows running -Reader, switching
# their input with Win+Space; with -AddInput, it first adds that input
# ("0409:{clsid}{profile}") to the user's inputs, and removes it after.
param(
  [Parameter(Mandatory)][string]$Kinds,
  [Parameter(Mandatory)][string]$Clsid,
  [Parameter(Mandatory)][string]$ProfileGuid,
  [Parameter(Mandatory)][int]$LangId,
  [Parameter(Mandatory)][string]$CasesFile,
  [string]$Klid = '',
  [string]$Reader = '',
  [string]$AddInput = ''
)
Add-Type -Path "$PSScriptRoot\TsfDriver.cs" -ReferencedAssemblies System.Windows.Forms, PresentationFramework, PresentationCore, WindowsBase, System.Xaml
$cases = @(Get-Content -Encoding UTF8 $CasesFile | Where-Object { $_ -ne '' })
foreach ($kind in $Kinds.Split(',')) {
  if ($kind -eq 'console') {
    if ($AddInput) { "USERINPUT add $AddInput $([TsfDriver]::EnableInput($AddInput, $true))" }
    [TsfDriver]::TypeConsole($Reader, 'e05b+39', $cases)
    if ($AddInput) { "USERINPUT remove $AddInput $([TsfDriver]::EnableInput($AddInput, $false))" }
  }
  else { [TsfDriver]::TypeCases($kind, $Clsid, $ProfileGuid, [uint16]$LangId, $Klid, $cases) }
}
