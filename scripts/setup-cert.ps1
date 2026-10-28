# Crea un certificado de firma de código "ACY Solutions" sólo para este usuario
# y lo marca como de confianza, para que Windows muestre "ACY Solutions" como
# editor de kero.exe en lugar de "Editor desconocido".
#
# - Se ejecuta una sola vez. Windows pedirá confirmar la instalación del certificado raíz.
# - La clave privada no es exportable y sólo sirve para firmar código (no para webs).
# - Sólo es válido en este equipo y usuario: para otros equipos hace falta un certificado comercial.

$ErrorActionPreference = "Stop"
$subject = "CN=ACY Solutions"

$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
    Where-Object { $_.Subject -eq $subject -and $_.NotAfter -gt (Get-Date) } |
    Select-Object -First 1

if (-not $cert) {
    $cert = New-SelfSignedCertificate `
        -Type CodeSigningCert `
        -Subject $subject `
        -FriendlyName "ACY Solutions (firma de código)" `
        -CertStoreLocation Cert:\CurrentUser\My `
        -KeyExportPolicy NonExportable `
        -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 `
        -NotAfter (Get-Date).AddYears(5)
    Write-Host "Certificado creado: $($cert.Thumbprint)"
} else {
    Write-Host "Ya existe el certificado: $($cert.Thumbprint)"
}

$cer = Join-Path $env:TEMP "acy-solutions-codesign.cer"
Export-Certificate -Cert $cert -FilePath $cer | Out-Null
try {
    foreach ($store in "Root", "TrustedPublisher") {
        $path = "Cert:\CurrentUser\$store"
        if (-not (Get-ChildItem $path | Where-Object Thumbprint -eq $cert.Thumbprint)) {
            Import-Certificate -FilePath $cer -CertStoreLocation $path | Out-Null
            Write-Host "Añadido a $store"
        }
    }
} finally {
    Remove-Item $cer -ErrorAction SilentlyContinue
}
Write-Host "Listo. Ahora ejecuta scripts\install.ps1"
