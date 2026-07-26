//! eval/report.json + eval/report.md writers, including the §4 threshold check
//! from docs/PHASE0_VALIDATION.md. The verdict line is deliberately left for a
//! human — the harness never self-approves the gate.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

/// §4 starting thresholds (the pilot refines them; final numbers go to DECISIONS.md).
pub const GO_QUIET_WER: f64 = 0.15;
pub const GO_FIELD_WER: f64 = 0.25;
pub const GO_TERM_RECALL: f64 = 0.80;

#[derive(Serialize, Clone)]
pub struct Row {
    pub clip: String,
    pub bucket: String,
    /// Domain vocabulary set this clip was biased and scored with (`default`
    /// unless the clip carries a `<id>.vocab.txt` sidecar). Term recall is only
    /// comparable inside one set, so the gate reports it per set.
    pub vocab_set: String,
    pub config: String,
    pub lang: Option<String>,
    pub audio_secs: f64,
    pub wall_secs: f64,
    pub rtf: f64,
    pub wer: f64,
    pub wer_folded: f64,
    pub cer: f64,
    pub cer_folded: f64,
    pub term_recall: Option<f64>,
    pub note: Option<String>,
    pub hyp_file: String,
}

pub struct RunMeta {
    /// Whisper models actually loaded this run (e.g. large-v3, large-v3-turbo).
    pub whisper_models: Vec<String>,
    pub parakeet_model: Option<String>,
    pub quick: Option<usize>,
    pub notes: Vec<String>,
}

#[derive(Serialize, Clone, Default)]
pub struct MedianCell {
    pub clips: usize,
    pub wer: Option<f64>,
    pub wer_folded: Option<f64>,
    pub cer: Option<f64>,
    pub cer_folded: Option<f64>,
    pub term_recall: Option<f64>,
    pub rtf: Option<f64>,
}

pub fn write_reports(eval_dir: &Path, rows: &[Row], meta: &RunMeta) -> Result<(PathBuf, PathBuf)> {
    let medians = compute_medians(rows);
    let domain_medians = compute_domain_medians(rows);
    let json_path = eval_dir.join("report.json");
    let md_path = eval_dir.join("report.md");

    let json = serde_json::json!({
        "generated_at": chrono::Local::now().to_rfc3339(),
        "harness": "eval-harness (Rust workspace bin; app engines: whisper-rs + ort)",
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "debug_build": cfg!(debug_assertions),
        "whisper_models": meta.whisper_models,
        "parakeet_model": meta.parakeet_model,
        "quick": meta.quick,
        "latency_semantics": {
            "wall_secs": "Bir klibin batch transkripsiyonunun başlamasından tamamlanmasına kadar geçen duvar saati süresi.",
            "rtf": "wall_secs / audio_secs; 1.0 gerçek zaman, 1.0 altı gerçek zamandan hızlı.",
            "live_ui_ttft_measured": false,
            "note": "Bu harness konuşmadan ekrandaki ilk metne kadar gecikmeyi (TTFT) veya canlı UI streaming gecikmesini ölçmez."
        },
        "human_review": {
            "reviewer": null,
            "reviewed_at": null,
            "multi_speaker_diarization_sanity": null,
            "multi_speaker_notes": null
        },
        "thresholds": {
            "go_quiet_wer": GO_QUIET_WER,
            "go_field_wer": GO_FIELD_WER,
            "go_term_recall": GO_TERM_RECALL,
        },
        "notes": meta.notes,
        "medians": medians,
        "medians_by_domain": domain_medians,
        "jargon_domains": jargon_domains(rows),
        "rows": rows,
    });
    std::fs::write(
        &json_path,
        serde_json::to_string_pretty(&json).context("rapor JSON serileştirilemedi")?,
    )
    .with_context(|| format!("yazılamadı: {}", json_path.display()))?;

    std::fs::write(
        &md_path,
        render_markdown(rows, &medians, &domain_medians, meta),
    )
    .with_context(|| format!("yazılamadı: {}", md_path.display()))?;

    Ok((json_path, md_path))
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        Some(v[n / 2])
    } else {
        Some((v[n / 2 - 1] + v[n / 2]) / 2.0)
    }
}

fn cell_from(rs: &[&Row]) -> MedianCell {
    MedianCell {
        clips: rs.len(),
        wer: median(&rs.iter().map(|r| r.wer).collect::<Vec<_>>()),
        wer_folded: median(&rs.iter().map(|r| r.wer_folded).collect::<Vec<_>>()),
        cer: median(&rs.iter().map(|r| r.cer).collect::<Vec<_>>()),
        cer_folded: median(&rs.iter().map(|r| r.cer_folded).collect::<Vec<_>>()),
        term_recall: median(&rs.iter().filter_map(|r| r.term_recall).collect::<Vec<_>>()),
        rtf: median(&rs.iter().map(|r| r.rtf).collect::<Vec<_>>()),
    }
}

fn compute_medians(rows: &[Row]) -> BTreeMap<String, MedianCell> {
    let mut groups: BTreeMap<(String, String), Vec<&Row>> = BTreeMap::new();
    for r in rows {
        groups
            .entry((r.config.clone(), r.bucket.clone()))
            .or_default()
            .push(r);
    }
    groups
        .into_iter()
        .map(|((config, bucket), rs)| (format!("{config}|{bucket}"), cell_from(&rs)))
        .collect()
}

/// Medians split by domain vocabulary as well as by bucket.
///
/// When one bucket carries clips from two domains (e.g. 5 legal + 5
/// intralogistics jargon clips), a single merged median averages them: legal
/// 0.60 and intralogistics 0.95 read as ~0.78 and the failing domain disappears
/// behind a passing number. The gate looks at exactly that number, so it must be
/// reported per domain.
fn compute_domain_medians(rows: &[Row]) -> BTreeMap<String, MedianCell> {
    let mut groups: BTreeMap<(String, String, String), Vec<&Row>> = BTreeMap::new();
    for r in rows {
        groups
            .entry((r.config.clone(), r.bucket.clone(), r.vocab_set.clone()))
            .or_default()
            .push(r);
    }
    groups
        .into_iter()
        .map(|((config, bucket, set), rs)| (format!("{config}|{bucket}|{set}"), cell_from(&rs)))
        .collect()
}

/// Distinct domain vocabularies present in the `jargon` bucket, sorted.
fn jargon_domains(rows: &[Row]) -> Vec<String> {
    let mut seen: Vec<String> = rows
        .iter()
        .filter(|r| r.bucket == "jargon")
        .map(|r| r.vocab_set.clone())
        .collect();
    seen.sort();
    seen.dedup();
    seen
}

/// Distinct domain vocabularies across every bucket, sorted.
fn all_domains(rows: &[Row]) -> Vec<String> {
    let mut seen: Vec<String> = rows.iter().map(|r| r.vocab_set.clone()).collect();
    seen.sort();
    seen.dedup();
    seen
}

fn distinct_configs(rows: &[Row]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for r in rows {
        if !seen.contains(&r.config) {
            seen.push(r.config.clone());
        }
    }
    seen
}

fn opt(v: Option<f64>, prec: usize) -> String {
    v.map_or_else(|| "—".to_string(), |x| format!("{x:.prec$}"))
}

fn gate(v: Option<f64>, threshold: f64, at_most: bool) -> &'static str {
    match v {
        None => "—",
        Some(x) => {
            let ok = if at_most {
                x <= threshold
            } else {
                x >= threshold
            };
            if ok {
                "PASS"
            } else {
                "FAIL"
            }
        }
    }
}

fn render_markdown(
    rows: &[Row],
    medians: &BTreeMap<String, MedianCell>,
    domain_medians: &BTreeMap<String, MedianCell>,
    meta: &RunMeta,
) -> String {
    let mut md = String::new();
    md.push_str("# Phase 0 Transcription Report\n\n");
    md.push_str(&format!(
        "- Üretim zamanı: {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S %z")
    ));
    md.push_str(
        "- Harness: `eval-harness` (Rust workspace bin) — uygulamanın KENDİ motorları: \
         whisper (`whisper-rs`) + Parakeet (`ort`). Harici whisper CLI / pip kullanılmadı.\n",
    );
    let whisper_models = if meta.whisper_models.is_empty() {
        "—".to_string()
    } else {
        meta.whisper_models.join(", ")
    };
    md.push_str(&format!(
        "- Whisper modelleri: {} | Parakeet modeli: {}\n",
        whisper_models,
        meta.parakeet_model.as_deref().unwrap_or("—")
    ));
    let mut pairs: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r.bucket.as_str(), r.clip.as_str()))
        .collect();
    pairs.sort_unstable();
    pairs.dedup();
    md.push_str(&format!(
        "- Klip: {} | Satır: {}{}\n",
        pairs.len(),
        rows.len(),
        meta.quick
            .map_or(String::new(), |n| format!(" | quick=ilk {n} klip/kova"))
    ));
    md.push_str(
        "- Normalizasyon: NFC → TR küçük harf (I→ı, İ→i) → kesme işaretleri silinir, \
         diğer noktalama→boşluk → boşluk sıkıştırma. `fold` sütunları ek olarak aksan katlar \
         (ç→c, ğ→g, ı→i, ö→o, ş→s, ü→u, â→a, ...). Strict metrikler aksan hatasını sayar; \
         fold saymaz. Terim yakalama fold eşleşmesiyle hesaplanır (alt-dizgi).\n\n",
    );
    md.push_str(
        "- Gecikme semantiği: `Duvar(s)` bir klibin batch transkripsiyonunun baştan sona \
         wall-clock süresidir; `RTF = wall_secs / audio_secs`. Bu harness konuşmadan ekrandaki ilk \
         metne kadar gecikmeyi (**canlı UI TTFT**) veya streaming yenileme gecikmesini ölçmez; \
         bunlar ayrı bir insan smoke testi gerektirir.\n\n",
    );

    if !meta.notes.is_empty() {
        md.push_str("## Notlar\n\n");
        for n in &meta.notes {
            md.push_str(&format!("- {n}\n"));
        }
        md.push('\n');
    }

    md.push_str("## Medyanlar (config | kova)\n\n");
    md.push_str(
        "| Config | Kova | Klip | WER | WER(fold) | CER | CER(fold) | Terim yakalama | RTF |\n",
    );
    md.push_str("|---|---|---:|---:|---:|---:|---:|---:|---:|\n");
    for (key, c) in medians {
        let (config, bucket) = key.split_once('|').unwrap_or((key.as_str(), ""));
        md.push_str(&format!(
            "| {config} | {bucket} | {} | {} | {} | {} | {} | {} | {} |\n",
            c.clips,
            opt(c.wer, 4),
            opt(c.wer_folded, 4),
            opt(c.cer, 4),
            opt(c.cer_folded, 4),
            opt(c.term_recall, 3),
            opt(c.rtf, 2)
        ));
    }
    md.push('\n');

    // Only worth a table when more than one domain vocabulary is in play;
    // otherwise it would repeat the table above with one extra column.
    let domains_all = all_domains(rows);
    if domains_all.len() > 1 {
        md.push_str("## Medyanlar (config | kova | domain)\n\n");
        md.push_str(&format!(
            "Bu koşuda {} domain sözlüğü var ({}). Terim yakalama yalnızca aynı sözlük içinde \
             karşılaştırılabilir, bu yüzden domain kırılımı ayrıca verilir.\n\n",
            domains_all.len(),
            domains_all.join(", ")
        ));
        md.push_str(
            "| Config | Kova | Domain | Klip | WER | WER(fold) | CER | Terim yakalama | RTF |\n",
        );
        md.push_str("|---|---|---|---:|---:|---:|---:|---:|---:|\n");
        for (key, c) in domain_medians {
            let mut parts = key.split('|');
            let config = parts.next().unwrap_or("");
            let bucket = parts.next().unwrap_or("");
            let set = parts.next().unwrap_or("");
            md.push_str(&format!(
                "| {config} | {bucket} | {set} | {} | {} | {} | {} | {} | {} |\n",
                c.clips,
                opt(c.wer, 4),
                opt(c.wer_folded, 4),
                opt(c.cer, 4),
                opt(c.term_recall, 3),
                opt(c.rtf, 2)
            ));
        }
        md.push('\n');
    }

    md.push_str("## Klip bazında sonuçlar\n\n");
    md.push_str(
        "| Kova | Klip | Domain | Config | Dil | WER | WER(fold) | CER | Terim | Ses(s) | Duvar(s) | RTF | Not |\n",
    );
    md.push_str("|---|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---|\n");
    for r in rows {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.4} | {:.4} | {:.4} | {} | {:.1} | {:.1} | {:.2} | {} |\n",
            r.bucket,
            r.clip,
            r.vocab_set,
            r.config,
            r.lang.as_deref().unwrap_or("auto"),
            r.wer,
            r.wer_folded,
            r.cer,
            opt(r.term_recall, 3),
            r.audio_secs,
            r.wall_secs,
            r.rtf,
            r.note.as_deref().unwrap_or("")
        ));
    }
    md.push('\n');

    md.push_str("## İnsan incelemesi — multi-speaker / diyarisasyon sanity\n\n");
    md.push_str(
        "Harness diyarisasyonu otomatik puanlamaz. Bir insan `multi` kovasındaki hipotezleri \
         kayıtlarla karşılaştırıp aşağıdaki alanları doldurmalıdır:\n\n",
    );
    md.push_str("- İnceleyen: ____________________\n");
    md.push_str("- İnceleme tarihi: ____________________\n");
    md.push_str(
        "- Multi-speaker / diyarisasyon sanity (PASS / FAIL / N/A): ____________________\n",
    );
    md.push_str("- İncelenen klipler ve konuşmacı dönüşü notları: ____________________\n\n");

    md.push_str("## Verdict — Phase-0 kapısı (docs/PHASE0_VALIDATION.md §4)\n\n");
    md.push_str(&format!(
        "Eşikler (başlangıç çıtası — kesinleşen değerler DECISIONS.md'ye yazılır):\n\n\
         - **GO (saha dahil tam kapsam):** medyan WER ≤ {GO_QUIET_WER} (quiet) VE ≤ {GO_FIELD_WER} (field) \
         VE **her jargon domaininde** terim yakalama ≥ {GO_TERM_RECALL} (vocab ayarlı config).\n\
         - **CONDITIONAL (yalnız toplantı odası):** quiet çıtayı geçer, field geçemez → Q ortamları için çık; saha ertelenir.\n\
         - **NO-GO:** ayara rağmen quiet WER kullanılamaz düzeyde → STT yaklaşımı yeniden değerlendirilir.\n\n"
    ));
    let domains = jargon_domains(rows);
    md.push_str(
        "Hesaplanan medyanlar (strict WER; jargon terim yakalama **domain sözlüğü başına** ayrı):\n\n",
    );
    if domains.len() > 1 {
        md.push_str(&format!(
            "> Bu koşuda {} jargon domaini var ({}). Her domain kendi satırında raporlanır ve \
             **GO için hepsinin** terim yakalama eşiğini geçmesi gerekir. Domainleri tek medyanda \
             harmanlamak birinin eşiği geçemediğini gizler.\n\n",
            domains.len(),
            domains.join(", ")
        ));
    }
    md.push_str(&format!(
        "| Config | Jargon domain | Quiet WER | ≤{GO_QUIET_WER}? | Field WER | ≤{GO_FIELD_WER}? | Terim yakalama | ≥{GO_TERM_RECALL}? |\n"
    ));
    md.push_str("|---|---|---:|:-:|---:|:-:|---:|:-:|\n");
    for config in distinct_configs(rows) {
        let quiet_wer = medians.get(&format!("{config}|quiet")).and_then(|c| c.wer);
        let field_wer = medians.get(&format!("{config}|field")).and_then(|c| c.wer);
        let quiet_cells = format!(
            "{} | {} | {} | {}",
            opt(quiet_wer, 4),
            gate(quiet_wer, GO_QUIET_WER, true),
            opt(field_wer, 4),
            gate(field_wer, GO_FIELD_WER, true)
        );
        if domains.is_empty() {
            // No jargon clips in this run — the recall gate cannot be evaluated.
            md.push_str(&format!("| {config} | — | {quiet_cells} | — | — |\n"));
            continue;
        }
        for domain in &domains {
            let recall = domain_medians
                .get(&format!("{config}|jargon|{domain}"))
                .and_then(|c| c.term_recall);
            md.push_str(&format!(
                "| {config} | {domain} | {quiet_cells} | {} | {} |\n",
                opt(recall, 3),
                gate(recall, GO_TERM_RECALL, false)
            ));
        }
    }
    md.push_str(
        "\n**Verdict (İNSAN karar verir — GO / CONDITIONAL(meeting-room) / NO-GO):** ________\n\n",
    );
    md.push_str("**Standartlaşılacak STT konfigi (motor + vocab):** ________\n\n");
    md.push_str(
        "Karar + eşikler docs/DECISIONS.md'ye ADR olarak işlenmeli; saha bağımlı BACKLOG kalemleri \
         (EPIC C) ancak ondan sonra açılır.\n",
    );
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(config: &str, bucket: &str, clip: &str, wer: f64, recall: Option<f64>) -> Row {
        row_in(config, bucket, clip, wer, recall, "default")
    }

    fn row_in(
        config: &str,
        bucket: &str,
        clip: &str,
        wer: f64,
        recall: Option<f64>,
        vocab_set: &str,
    ) -> Row {
        Row {
            clip: clip.to_string(),
            bucket: bucket.to_string(),
            vocab_set: vocab_set.to_string(),
            config: config.to_string(),
            lang: None,
            audio_secs: 60.0,
            wall_secs: 30.0,
            rtf: 0.5,
            wer,
            wer_folded: wer,
            cer: wer / 2.0,
            cer_folded: wer / 2.0,
            term_recall: recall,
            note: None,
            hyp_file: String::new(),
        }
    }

    #[test]
    fn median_odd_even_empty() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[0.3]), Some(0.3));
        assert_eq!(median(&[0.1, 0.3, 0.2]), Some(0.2));
        assert_eq!(median(&[0.1, 0.2, 0.3, 0.4]), Some(0.25));
    }

    #[test]
    fn medians_group_by_config_and_bucket() {
        let rows = vec![
            row("cfgA", "quiet", "q1", 0.10, None),
            row("cfgA", "quiet", "q2", 0.20, None),
            row("cfgA", "jargon", "j1", 0.30, Some(0.9)),
            row("cfgB", "quiet", "q1", 0.40, None),
        ];
        let m = compute_medians(&rows);
        let a_quiet = m.get("cfgA|quiet").expect("cfgA|quiet");
        assert_eq!(a_quiet.clips, 2);
        assert!((a_quiet.wer.expect("wer") - 0.15).abs() < 1e-9);
        let a_jargon = m.get("cfgA|jargon").expect("cfgA|jargon");
        assert!((a_jargon.term_recall.expect("recall") - 0.9).abs() < 1e-9);
        assert!(m.contains_key("cfgB|quiet"));
        // rows without term_recall → None (not 1.0)
        assert!(a_quiet.term_recall.is_none());
    }

    #[test]
    fn markdown_contains_verdict_scaffold() {
        let rows = vec![
            row("whisper_large_v3", "quiet", "q1", 0.12, None),
            row("whisper_large_v3", "field", "f1", 0.30, None),
            row("whisper_large_v3", "jargon", "j1", 0.20, Some(0.85)),
        ];
        let meta = RunMeta {
            whisper_models: vec!["large-v3".into(), "large-v3-turbo".into()],
            parakeet_model: None,
            quick: None,
            notes: vec!["test notu".into()],
        };
        let md = render_markdown(
            &rows,
            &compute_medians(&rows),
            &compute_domain_medians(&rows),
            &meta,
        );
        assert!(md.contains("Verdict"));
        assert!(md.contains("İNSAN karar verir"));
        assert!(md.contains("PASS")); // quiet 0.12 ≤ 0.15
        assert!(md.contains("FAIL")); // field 0.30 > 0.25
        assert!(md.contains("test notu"));
        assert!(md.contains("large-v3, large-v3-turbo")); // joined whisper model list
        assert!(md.contains("canlı UI TTFT"));
        assert!(md.contains("multi-speaker / diyarisasyon sanity"));
        assert!(md.contains("PASS / FAIL / N/A"));
    }

    // --- per-domain term recall ----------------------------------------------

    /// Two jargon domains in one bucket: legal fails the recall gate badly
    /// (0.60), intralogistics is perfect (1.00). Merged, the bucket median lands
    /// exactly on the threshold and reads PASS — which is precisely how a
    /// failing domain used to disappear.
    fn split_domain_rows() -> Vec<Row> {
        vec![
            row_in("cfgA", "quiet", "q1", 0.10, None, "default"),
            row_in("cfgA", "jargon", "jl1", 0.20, Some(0.60), "legal"),
            row_in("cfgA", "jargon", "jl2", 0.20, Some(0.60), "legal"),
            row_in("cfgA", "jargon", "ji1", 0.20, Some(1.00), "default"),
            row_in("cfgA", "jargon", "ji2", 0.20, Some(1.00), "default"),
        ]
    }

    #[test]
    fn merged_bucket_median_hides_a_failing_domain() {
        let rows = split_domain_rows();

        // The old, merged number: median(0.60, 0.60, 1.00, 1.00) = 0.80 → PASS.
        let merged = compute_medians(&rows)["cfgA|jargon"]
            .term_recall
            .expect("merged recall");
        assert!((merged - 0.80).abs() < 1e-9);
        assert_eq!(gate(Some(merged), GO_TERM_RECALL, false), "PASS");

        // Split: legal is exposed as a FAIL, intralogistics still passes.
        let per_domain = compute_domain_medians(&rows);
        let legal = per_domain["cfgA|jargon|legal"]
            .term_recall
            .expect("legal recall");
        let intra = per_domain["cfgA|jargon|default"]
            .term_recall
            .expect("default recall");
        assert!((legal - 0.60).abs() < 1e-9);
        assert!((intra - 1.00).abs() < 1e-9);
        assert_eq!(gate(Some(legal), GO_TERM_RECALL, false), "FAIL");
        assert_eq!(gate(Some(intra), GO_TERM_RECALL, false), "PASS");
    }

    #[test]
    fn threshold_table_reports_each_jargon_domain_on_its_own_row() {
        let rows = split_domain_rows();
        let meta = RunMeta {
            whisper_models: vec!["large-v3".into()],
            parakeet_model: None,
            quick: None,
            notes: vec![],
        };
        let md = render_markdown(
            &rows,
            &compute_medians(&rows),
            &compute_domain_medians(&rows),
            &meta,
        );

        // One threshold row per (config, domain), carrying that domain's recall.
        assert!(md.contains("| cfgA | legal |"));
        assert!(md.contains("| cfgA | default |"));
        assert!(md.contains("0.600"));
        assert!(md.contains("1.000"));
        // The GO rule and the multi-domain warning must both be explicit.
        assert!(md.contains("her jargon domaininde"));
        assert!(md.contains("GO için hepsinin"));
        // The blended figure must not be presented as the gate number.
        assert!(!md.contains("jargon kovası medyanı"));
    }

    #[test]
    fn single_domain_run_needs_no_domain_warning() {
        let rows = vec![
            row("cfgA", "quiet", "q1", 0.10, None),
            row("cfgA", "jargon", "j1", 0.20, Some(0.85)),
        ];
        let meta = RunMeta {
            whisper_models: vec![],
            parakeet_model: None,
            quick: None,
            notes: vec![],
        };
        let md = render_markdown(
            &rows,
            &compute_medians(&rows),
            &compute_domain_medians(&rows),
            &meta,
        );
        assert!(!md.contains("GO için hepsinin"));
        assert!(!md.contains("## Medyanlar (config | kova | domain)"));
        assert!(md.contains("| cfgA | default |"));
    }

    #[test]
    fn run_without_jargon_clips_reports_recall_as_unavailable() {
        let rows = vec![row("cfgA", "quiet", "q1", 0.10, None)];
        assert!(jargon_domains(&rows).is_empty());
        let meta = RunMeta {
            whisper_models: vec![],
            parakeet_model: None,
            quick: None,
            notes: vec![],
        };
        let md = render_markdown(
            &rows,
            &compute_medians(&rows),
            &compute_domain_medians(&rows),
            &meta,
        );
        // config row still present, recall columns blank rather than fabricated
        assert!(md.contains("| cfgA | — |"));
    }
}
