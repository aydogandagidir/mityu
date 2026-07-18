[CmdletBinding()]
param(
    [string]$RepoRoot = "",
    [string]$Manifest = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
}

if ([string]::IsNullOrWhiteSpace($Manifest)) {
    $Manifest = Join-Path $RepoRoot "eval\evidence\manifest.csv"
}

$requiredColumns = @(
    "schema_version", "clip_id", "bucket", "source_kind", "duration_seconds",
    "participant_count", "languages", "environment", "permission_confirmed",
    "consent_evidence_id", "notice_version", "recorded_at_utc", "human_reviewer_id",
    "transcript_approved_at_utc", "audio_sha256", "reference_sha256",
    "retention_delete_by"
)
$buckets = @("quiet", "field", "multi", "jargon")
$allowedSourceKinds = @("consented-roleplay", "consented-real-target")
$errors = [System.Collections.Generic.List[string]]::new()

if (-not (Test-Path -LiteralPath $Manifest -PathType Leaf)) {
    throw "Phase-0 evidence manifest is missing: $Manifest`nCopy eval/evidence/manifest.template.csv to manifest.csv and fill it after the human actions occur."
}

$rows = @(Import-Csv -LiteralPath $Manifest)
if ($rows.Count -eq 0) {
    throw "Phase-0 evidence manifest has no rows: $Manifest"
}

$headers = @($rows[0].PSObject.Properties.Name)
foreach ($column in $requiredColumns) {
    if ($column -notin $headers) {
        $errors.Add("manifest: required column '$column' is missing")
    }
}

$duplicateIds = @($rows | Group-Object clip_id | Where-Object { $_.Count -gt 1 })
foreach ($duplicate in $duplicateIds) {
    $errors.Add("manifest: duplicate clip_id '$($duplicate.Name)'")
}

foreach ($bucket in $buckets) {
    $count = @($rows | Where-Object { $_.bucket -eq $bucket }).Count
    if ($count -lt 5) {
        $errors.Add("${bucket}: $count/5 manifest rows")
    }
}

foreach ($row in $rows) {
    $id = [string]$row.clip_id
    $bucket = [string]$row.bucket
    $label = if ($id) { "$bucket/$id" } else { "row-without-id" }

    if ([string]::IsNullOrWhiteSpace($id) -or $id -notmatch '^[a-z0-9][a-z0-9_-]*$') {
        $errors.Add("${label}: clip_id must match ^[a-z0-9][a-z0-9_-]*$")
    }
    if ($bucket -notin $buckets) {
        $errors.Add("${label}: invalid bucket '$bucket'")
        continue
    }
    if ($row.schema_version -ne "1") {
        $errors.Add("${label}: schema_version must be 1")
    }
    if ($row.source_kind -notin $allowedSourceKinds) {
        $errors.Add("${label}: source_kind must be consented-roleplay or consented-real-target; public/synthetic audio cannot close the gate")
    }

    $duration = 0.0
    if (-not [double]::TryParse(
        [string]$row.duration_seconds,
        [Globalization.NumberStyles]::Float,
        [Globalization.CultureInfo]::InvariantCulture,
        [ref]$duration
    ) -or $duration -lt 120 -or $duration -gt 600) {
        $errors.Add("${label}: duration_seconds must be between 120 and 600")
    }

    $participants = 0
    if (-not [int]::TryParse([string]$row.participant_count, [ref]$participants) -or $participants -lt 1) {
        $errors.Add("${label}: participant_count must be a positive integer")
    } elseif ($bucket -eq "quiet" -and $participants -gt 2) {
        $errors.Add("${label}: quiet requires 1-2 participants")
    } elseif ($bucket -eq "field" -and $participants -gt 3) {
        $errors.Add("${label}: field requires 1-3 participants")
    } elseif ($bucket -eq "multi" -and $participants -lt 3) {
        $errors.Add("${label}: multi requires at least 3 participants")
    }

    if ([string]::IsNullOrWhiteSpace($row.languages)) {
        $errors.Add("${label}: languages is required")
    } elseif ($bucket -eq "jargon") {
        $langs = @(([string]$row.languages).ToLowerInvariant().Split(';') | ForEach-Object { $_.Trim() })
        if ("tr" -notin $langs -or "en" -notin $langs) {
            $errors.Add("${label}: jargon languages must include both tr and en, separated with ';'")
        }
    }
    if ([string]::IsNullOrWhiteSpace($row.environment)) {
        $errors.Add("${label}: environment is required")
    }
    if (([string]$row.permission_confirmed).ToLowerInvariant() -ne "true") {
        $errors.Add("${label}: permission_confirmed must be true after real permission evidence exists")
    }
    foreach ($field in @("consent_evidence_id", "notice_version", "human_reviewer_id")) {
        if ([string]::IsNullOrWhiteSpace($row.$field)) {
            $errors.Add("${label}: $field is required (use opaque IDs, never participant PII)")
        }
    }

    foreach ($field in @("recorded_at_utc", "transcript_approved_at_utc")) {
        $parsed = [DateTimeOffset]::MinValue
        if (-not [DateTimeOffset]::TryParse(
            [string]$row.$field,
            [Globalization.CultureInfo]::InvariantCulture,
            [Globalization.DateTimeStyles]::RoundtripKind,
            [ref]$parsed
        )) {
            $errors.Add("${label}: $field must be an ISO-8601 timestamp")
        }
    }
    $deleteBy = [DateTime]::MinValue
    if (-not [DateTime]::TryParseExact(
        [string]$row.retention_delete_by,
        "yyyy-MM-dd",
        [Globalization.CultureInfo]::InvariantCulture,
        [Globalization.DateTimeStyles]::None,
        [ref]$deleteBy
    )) {
        $errors.Add("${label}: retention_delete_by must be YYYY-MM-DD")
    }

    $wav = Join-Path $RepoRoot "eval\$bucket\$id.wav"
    $reference = Join-Path $RepoRoot "eval\$bucket\$id.ref.txt"
    if (-not (Test-Path -LiteralPath $wav -PathType Leaf)) {
        $errors.Add("${label}: missing $wav")
    }
    if (-not (Test-Path -LiteralPath $reference -PathType Leaf)) {
        $errors.Add("${label}: missing $reference")
    } elseif ([string]::IsNullOrWhiteSpace((Get-Content -LiteralPath $reference -Raw -Encoding UTF8))) {
        $errors.Add("${label}: reference transcript is empty")
    }

    foreach ($artifact in @(
        @{ Name = "audio_sha256"; Path = $wav },
        @{ Name = "reference_sha256"; Path = $reference }
    )) {
        $expected = ([string]$row.($artifact.Name)).ToLowerInvariant()
        if ($expected -notmatch '^[0-9a-f]{64}$') {
            $errors.Add("${label}: $($artifact.Name) must be a 64-character lowercase SHA-256")
        } elseif (Test-Path -LiteralPath $artifact.Path -PathType Leaf) {
            $actual = (Get-FileHash -LiteralPath $artifact.Path -Algorithm SHA256).Hash.ToLowerInvariant()
            if ($actual -ne $expected) {
                $errors.Add("${label}: $($artifact.Name) mismatch (expected $expected, actual $actual)")
            }
        }
    }
}

if ($errors.Count -gt 0) {
    Write-Error ("Phase-0 evidence verification FAILED:`n- " + ($errors -join "`n- "))
    exit 1
}

Write-Host "Phase-0 evidence manifest integrity PASS: $($rows.Count) rows; each bucket has at least 5 consented human recordings."
Write-Host "Next: run 'cargo run -p eval-harness -- run', then complete the human diarization, TTFT and verdict fields."
Write-Warning "This verifies manifest/file integrity, not legal sufficiency or the truth of a human attestation."
