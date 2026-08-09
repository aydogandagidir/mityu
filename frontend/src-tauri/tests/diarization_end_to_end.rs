//! The whole diarization path, with real models and real audio.
//!
//! `#[ignore]` because it downloads ~34 MB of models, needs the built
//! `diarize-helper`, and takes seconds rather than milliseconds. Everything the
//! unit tests can reach is covered there; this covers the part they cannot —
//! that the four pieces (audio probe, decode, sidecar, storage) actually fit
//! together, which is exactly where an integration bug would live.
//!
//! ```text
//! cargo build -p diarize-helper
//! set MITYU_DIARIZE_HELPER=target\debug\diarize-helper.exe
//! cargo test -p mityu --test diarization_end_to_end -- --ignored --nocapture
//! ```

use app_lib::context::{AuthContext, RequestId, Role, TenantId, UserId};
use app_lib::database::repositories::speaker_turn::SpeakerTurnsRepository;
use app_lib::diarization::{models, service};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

fn ctx() -> AuthContext {
    AuthContext {
        tenant_id: TenantId::new("local"),
        user_id: UserId::new("user"),
        roles: vec![Role::Owner],
        request_id: RequestId::generate(),
    }
}

async fn db(path: &std::path::Path) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open db");
    MIGRATOR.run(&pool).await.expect("migrations apply");
    pool
}

/// A meeting that kept no audio must report `NoAudio`, not fail and not offer a
/// pass that cannot run. This half needs neither models nor the sidecar.
#[tokio::test]
async fn a_transcripts_only_meeting_reports_no_audio() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pool = db(&dir.path().join("t.db")).await;
    let ctx = ctx();

    let folder = dir.path().join("Meeting_no_audio");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("transcripts.json"), b"{}").expect("transcripts only");

    sqlx::query(
        "INSERT INTO meetings (id, workspace_id, title, created_at, updated_at, folder_path) \
         VALUES ('m-1', 'local', 'No audio', '2026-08-09T10:00:00Z', '2026-08-09T10:00:00Z', ?)",
    )
    .bind(folder.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .expect("seed meeting");

    let models_dir = dir.path().join("models");
    let state = service::availability(&pool, &ctx, "m-1", folder.to_str(), &models_dir)
        .await
        .expect("availability");
    assert_eq!(state, service::Availability::NoAudio);

    // And running anyway must refuse with a reason, not panic or write nothing
    // while reporting success.
    let err = service::diarize_meeting(&pool, &ctx, "m-1", folder.to_str(), &models_dir)
        .await
        .expect_err("must refuse");
    assert!(err.to_string().contains("no saved audio"), "{err}");
}

/// The real thing: audio on disk, real models, the real sidecar, and rows in
/// the database at the end.
#[tokio::test]
#[ignore = "downloads ~34 MB of models and needs the built diarize-helper"]
async fn a_real_recording_is_diarized_and_stored() {
    let helper = std::env::var("MITYU_DIARIZE_HELPER")
        .expect("set MITYU_DIARIZE_HELPER to the built diarize-helper binary");
    assert!(
        std::path::Path::new(&helper).is_file(),
        "MITYU_DIARIZE_HELPER does not point at a file: {helper}"
    );
    let sample = std::env::var("MITYU_DIARIZE_SAMPLE_WAV")
        .expect("set MITYU_DIARIZE_SAMPLE_WAV to a multi-speaker wav");

    let dir = tempfile::tempdir().expect("tempdir");
    let pool = db(&dir.path().join("t.db")).await;
    let ctx = ctx();

    // A meeting folder shaped the way the recorder leaves one.
    let folder = dir.path().join("Meeting_2026-08-09_10-00");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::copy(&sample, folder.join("audio.wav")).expect("place recording");

    sqlx::query(
        "INSERT INTO meetings (id, workspace_id, title, created_at, updated_at, folder_path) \
         VALUES ('m-1', 'local', 'Two speakers', '2026-08-09T10:00:00Z', '2026-08-09T10:00:00Z', ?)",
    )
    .bind(folder.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .expect("seed meeting");

    let models_dir = dir.path().join("models");
    assert_eq!(
        service::availability(&pool, &ctx, "m-1", folder.to_str(), &models_dir)
            .await
            .expect("availability"),
        service::Availability::ModelsMissing,
        "audio is there but the models are not, and those are different problems"
    );

    models::ensure(&models_dir, |label, done, total| {
        if total > 0 && done == total {
            println!("  fetched {label}");
        }
    })
    .await
    .expect("acquire models");

    assert_eq!(
        service::availability(&pool, &ctx, "m-1", folder.to_str(), &models_dir)
            .await
            .expect("availability"),
        service::Availability::Ready
    );

    let stored = service::diarize_meeting(&pool, &ctx, "m-1", folder.to_str(), &models_dir)
        .await
        .expect("diarize");
    println!("  stored {stored} turns");
    assert!(stored > 0, "a two-speaker recording produced no turns");

    let turns = SpeakerTurnsRepository::list_for_meeting(&pool, &ctx, "m-1")
        .await
        .expect("read turns");
    assert_eq!(turns.len(), stored);
    // Anonymous, 1-based, in milliseconds, no invented confidence — the
    // ADR-0035 conversion, checked where it actually lands.
    assert!(turns
        .iter()
        .all(|t| t.speaker_label.starts_with("Speaker ")));
    assert!(turns.iter().all(|t| t.end_ms > t.start_ms));
    assert!(turns.iter().all(|t| t.confidence.is_none()));
    let speakers: std::collections::BTreeSet<&str> =
        turns.iter().map(|t| t.speaker_label.as_str()).collect();
    println!("  speakers: {speakers:?}");
    assert!(speakers.len() >= 2, "expected at least two speakers");

    // And the state has moved on, so the UI stops offering a pass.
    match service::availability(&pool, &ctx, "m-1", folder.to_str(), &models_dir)
        .await
        .expect("availability")
    {
        service::Availability::Done { turns, .. } => assert_eq!(turns, stored),
        other => panic!("expected Done, got {other:?}"),
    }
}
