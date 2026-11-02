# Desinstala Kero. Lo invoca Configuración › Aplicaciones (se copia junto a kero.exe).
# La configuración (%APPDATA%\Kero) se conserva; el índice y el historial de uso se borran.
param([switch]$Quiet)

$dir = Join-Path $env:LOCALAPPDATA "Programs\Kero"
$exe = Join-Path $dir "kero.exe"

if (Test-Path $exe) {
    (Start-Process $exe -ArgumentList "--quit" -PassThru).WaitForExit()
}
Remove-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name Kero -ErrorAction SilentlyContinue
Remove-Item (Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Kero.lnk") -ErrorAction SilentlyContinue
Remove-Item "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Kero" -Recurse -ErrorAction SilentlyContinue
Remove-Item (Join-Path $env:LOCALAPPDATA "Kero") -Recurse -Force -ErrorAction SilentlyContinue

# Quita la carpeta del PATH del usuario (sin expandir las %VARIABLES%).
$envKey = Get-Item "HKCU:\Environment"
$userPath = $envKey.GetValue("Path", "", "DoNotExpandEnvironmentNames")
$parts = @($userPath -split ";" | Where-Object { $_ -ne "" -and $_ -ne $dir })
if ($userPath -and ($parts -join ";") -ne $userPath) {
    Set-ItemProperty -Path "HKCU:\Environment" -Name Path -Value ($parts -join ";") -Type $envKey.GetValueKind("Path")
    [Environment]::SetEnvironmentVariable("KERO_PATH_REFRESH", "1", "User")
    [Environment]::SetEnvironmentVariable("KERO_PATH_REFRESH", $null, "User")
}

for ($i = 0; $i -lt 10 -and (Test-Path $dir); $i++) {
    Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
    if (Test-Path $dir) { Start-Sleep -Milliseconds 300 }
}

if (-not $Quiet) {
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.MessageBox]::Show(
        "Kero se desinstaló. Tu configuración se conserva en %APPDATA%\Kero.", "Kero") | Out-Null
}
