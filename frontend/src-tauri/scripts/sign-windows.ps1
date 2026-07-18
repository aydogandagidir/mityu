param(
    [Parameter(Mandatory=$true)]
    [string]$FilePath
)

# Check if signing is enabled
if (-not $env:DIGICERT_KEYPAIR_ALIAS) {
    if ($env:MITYU_REQUIRE_WINDOWS_SIGNING -eq '1') {
        Write-Error "Windows signing is required, but no DigiCert keypair alias is configured"
        exit 1
    }
    Write-Host "Skipping signing - DIGICERT_KEYPAIR_ALIAS not set"
    exit 0
}

Write-Host "Signing: $FilePath"

# Keep provider output out of public logs because it may contain account or
# certificate metadata. The final Authenticode checks provide safe evidence.
$signOutput = smctl sign --keypair-alias $env:DIGICERT_KEYPAIR_ALIAS --input $FilePath 2>&1
$signExitCode = $LASTEXITCODE

if ($signExitCode -ne 0) {
    Write-Error "Signing failed with exit code: $signExitCode"
    exit $signExitCode
}

# Verify the signature was applied
$sig = Get-AuthenticodeSignature -FilePath $FilePath
if ($sig.Status -ne 'Valid') {
    Write-Error "Signature verification failed after signing"
    Write-Error "Status: $($sig.Status)"
    Write-Error "Message: $($sig.StatusMessage)"
    exit 1
}

if ($env:SM_CODE_SIGNING_CERT_SHA1_HASH) {
    $expectedThumbprint = ($env:SM_CODE_SIGNING_CERT_SHA1_HASH -replace '\s', '').ToUpperInvariant()
    $actualThumbprint = ($sig.SignerCertificate.Thumbprint -replace '\s', '').ToUpperInvariant()
    if ($actualThumbprint -ne $expectedThumbprint) {
        Write-Error "Signature was produced by an unexpected certificate"
        exit 1
    }
}

if (-not $sig.TimeStamperCertificate) {
    Write-Error "Signature is missing a trusted timestamp"
    exit 1
}

Write-Host "Successfully signed: $FilePath"
Write-Host "Signature status: $($sig.Status)"
