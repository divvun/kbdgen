# Keeps -Dll loaded in this process until the file -Release exists, as
# Explorer keeps a text service loaded once it was active there.
param([Parameter(Mandatory)][string]$Dll, [Parameter(Mandatory)][string]$Release)
Add-Type -Namespace KbdTsf -Name Hold -MemberDefinition '[DllImport("kernel32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr LoadLibraryW(string path);'
[void][KbdTsf.Hold]::LoadLibraryW($Dll)
while (-not (Test-Path $Release)) { Start-Sleep -Milliseconds 200 }
