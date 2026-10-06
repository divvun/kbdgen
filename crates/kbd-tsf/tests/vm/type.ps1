# Types the cases of -CasesFile into each control kind of -Kinds, in this
# PowerShell process (64- or 32-bit), and prints the driver's lines.
param(
  [Parameter(Mandatory)][string]$Kinds,
  [Parameter(Mandatory)][string]$Clsid,
  [Parameter(Mandatory)][string]$ProfileGuid,
  [Parameter(Mandatory)][int]$LangId,
  [Parameter(Mandatory)][string]$CasesFile,
  [string]$Klid = ''
)
Add-Type -Path "$PSScriptRoot\TsfDriver.cs" -ReferencedAssemblies System.Windows.Forms, PresentationFramework, PresentationCore, WindowsBase, System.Xaml
$cases = @(Get-Content -Encoding UTF8 $CasesFile | Where-Object { $_ -ne '' })
foreach ($kind in $Kinds.Split(',')) {
  [TsfDriver]::TypeCases($kind, $Clsid, $ProfileGuid, [uint16]$LangId, $Klid, $cases)
}
