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
    "retention_delete_by", "consent_withdrawn_at_utc"
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

# KVKK m.7/m.11: a participant may withdraw consent at any time. A withdrawn row
# stays in the manifest as the RECORD of the withdrawal, but its recording must be
# gone and it must not count toward gate coverage. If that drops a bucket below
# five, verification fails -- which is correct: the gate must not be closed with a
# four-clip bucket just because someone exercised a right.
$withdrawn = @{}
foreach ($row in $rows) {
    $withdrawnAt = [string]$row.consent_withdrawn_at_utc
    if (-not [string]::IsNullOrWhiteSpace($withdrawnAt)) {
        $withdrawn[[string]$row.clip_id] = $true
    }
}

foreach ($bucket in $buckets) {
    $count = @($rows | Where-Object {
        $_.bucket -eq $bucket -and -not $withdrawn.ContainsKey([string]$_.clip_id)
    }).Count
    if ($count -lt 5) {
        $errors.Add("${bucket}: $count/5 manifest rows with live consent")
    }
}

foreach ($row in $rows) {
    $id = [string]$row.clip_id
    $bucket = [string]$row.bucket
    $label = if ($id) { "$bucket/$id" } else { "row-without-id" }

    # Identity, schema and audit fields are checked for EVERY row, withdrawn or
    # not. A withdrawal record is still an audit record: it must be identifiable
    # and on the current schema. The bucket check in particular has to run first,
    # because the erasure paths below are built from it.
    if ([string]::IsNullOrWhiteSpace($id) -or $id -notmatch '^[a-z0-9][a-z0-9_-]*$') {
        $errors.Add("${label}: clip_id must match ^[a-z0-9][a-z0-9_-]*$")
    }
    if ($bucket -notin $buckets) {
        $errors.Add("${label}: invalid bucket '$bucket'")
        continue
    }
    # v2 added consent_withdrawn_at_utc (KVKK withdrawal) and split the jargon
    # bucket across two domains (ADR-0031). A v1 manifest has no way to record a
    # withdrawal, so it is rejected rather than silently accepted.
    if ($row.schema_version -ne "2") {
        $errors.Add("${label}: schema_version must be 2 (v1 predates consent_withdrawn_at_utc; re-copy manifest.template.csv)")
    }
    if ($row.source_kind -notin $allowedSourceKinds) {
        $errors.Add("${label}: source_kind must be consented-roleplay or consented-real-target; public/synthetic audio cannot close the gate")
    }
    foreach ($field in @("consent_evidence_id", "notice_version", "human_reviewer_id")) {
        if ([string]::IsNullOrWhiteSpace($row.$field)) {
            $errors.Add("${label}: $field is required (use opaque IDs, never participant PII)")
        }
    }

    # A withdrawn row is checked for ERASURE instead of completeness.
    #
    # The rule is "no file bearing this clip id may remain under eval/", not a
    # list of known filenames: the workflow produces the original recording
    # (eval/raw/<bucket>/<id>.<ext>), the normalized wav, the draft, the
    # human reference AND one verbatim <id>.<config>.hyp.txt per config -- six
    # at the default config set. An enumerated list was already incomplete when
    # written and would rot again the next time the harness emits something new.
    if ($withdrawn.ContainsKey($id)) {
        $withdrawnParsed = [DateTimeOffset]::MinValue
        if (-not [DateTimeOffset]::TryParse(
            [string]$row.consent_withdrawn_at_utc,
            [Globalization.CultureInfo]::InvariantCulture,
            [Globalization.DateTimeStyles]::RoundtripKind,
            [ref]$withdrawnParsed
        )) {
            $errors.Add("${label}: consent_withdrawn_at_utc must be an ISO-8601 timestamp")
        }
        foreach ($dir in @(
            (Join-Path $RepoRoot "eval\$bucket"),
            (Join-Path $RepoRoot "eval\raw\$bucket")
        )) {
            if (-not (Test-Path -LiteralPath $dir -PathType Container)) { continue }
            foreach ($leftover in @(Get-ChildItem -LiteralPath $dir -File -Filter "$id.*" -ErrorAction SilentlyContinue)) {
                $errors.Add("${label}: consent was withdrawn but $($leftover.FullName) still exists -- it must be deleted")
            }
        }
        continue
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
