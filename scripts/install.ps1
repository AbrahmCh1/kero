# Compila Kero, lo firma (si existe el certificado) y lo instala como app en
# %LOCALAPPDATA%\Programs\Kero, con acceso directo en Inicio y entrada en
# Configuración › Aplicaciones. Volver a ejecutarlo actualiza la instalación.
#
# La instalación vive aquí y no dentro de kero.exe a propósito: un exe sin firmar
# que se copia a sí mismo y se registra dispara la heurística de Defender.
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\.."

Push-Location $root
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build falló" }
} finally {
    Pop-Location
}

$exe = Join-Path $root "target\release\kero.exe"
$hasCert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
    Where-Object { $_.Subject -eq "CN=ACY Solutions" -and $_.NotAfter -gt (Get-Date) }
if ($hasCert) {
    & "$PSScriptRoot\sign.ps1" -File $exe
} else {
    Write-Warning "Sin certificado: se instala sin firmar (ejecuta scripts\setup-cert.ps1 para firmar)."
}

# Cierra la instancia abierta (de forma ordenada) para poder reemplazar el exe.
# WaitForExit y no -Wait: -Wait también esperaría a los procesos hijos.
(Start-Process $exe -ArgumentList "--quit" -PassThru).WaitForExit()

$dir = Join-Path $env:LOCALAPPDATA "Programs\Kero"
$dst = Join-Path $dir "kero.exe"
New-Item -ItemType Directory -Force $dir | Out-Null
for ($i = 0; ; $i++) {
    try { Copy-Item $exe $dst -Force; break }
    catch { if ($i -ge 10) { throw }; Start-Sleep -Milliseconds 300 }
}
Copy-Item "$PSScriptRoot\uninstall.ps1" $dir -Force

# Acceso directo en el menú Inicio
$lnk = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Kero.lnk"
$shell = New-Object -ComObject WScript.Shell
$sc = $shell.CreateShortcut($lnk)
$sc.TargetPath = $dst
$sc.WorkingDirectory = $dir
$sc.IconLocation = "$dst,0"
$sc.Description = "Lanzador estilo Spotlight"
$sc.Save()

# Entrada en Configuración › Aplicaciones instaladas
$key = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Kero"
New-Item -Path $key -Force | Out-Null
$version = (Get-Item $dst).VersionInfo.ProductVersion
$uninstall = "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$dir\uninstall.ps1`""
$values = @{
    DisplayName     = "Kero"
    DisplayVersion  = $version
    Publisher       = "ACY Solutions"
    DisplayIcon     = "$dst,0"
    InstallLocation = $dir
    UninstallString = $uninstall
    InstallDate     = (Get-Date -Format "yyyyMMdd")
    Comments        = "Lanzador estilo Spotlight para Windows"
}
foreach ($k in $values.Keys) { Set-ItemProperty -Path $key -Name $k -Value $values[$k] }
Set-ItemProperty -Path $key -Name EstimatedSize -Value ([int]((Get-Item $dst).Length / 1KB)) -Type DWord
Set-ItemProperty -Path $key -Name NoModify -Value 1 -Type DWord
Set-ItemProperty -Path $key -Name NoRepair -Value 1 -Type DWord

# Si el inicio automático estaba activo, que apunte a la copia instalada.
$run = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
if (Get-ItemProperty -Path $run -Name Kero -ErrorAction SilentlyContinue) {
    Set-ItemProperty -Path $run -Name Kero -Value "`"$dst`" --hidden"
}

Start-Process $dst
Write-Host "Kero $version instalado en $dir (menú Inicio y Configuración > Aplicaciones)."
