# Firma un .exe con el certificado "ACY Solutions" creado por setup-cert.ps1.
param([string]$File = "$PSScriptRoot\..\target\release\kero.exe")

$ErrorActionPreference = "Stop"
$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
    Where-Object { $_.Subject -eq "CN=ACY Solutions" -and $_.NotAfter -gt (Get-Date) } |
    Select-Object -First 1
if (-not $cert) {
    throw "No hay certificado 'ACY Solutions'. Ejecuta primero scripts\setup-cert.ps1"
}

$file = (Resolve-Path $File).Path
$sig = Set-AuthenticodeSignature -FilePath $file -Certificate $cert -HashAlgorithm SHA256
if ($sig.Status -ne "Valid") {
    throw "La firma no es válida: $($sig.Status) $($sig.StatusMessage)"
}
Write-Host "Firmado: $file ($($sig.SignerCertificate.Subject))"
