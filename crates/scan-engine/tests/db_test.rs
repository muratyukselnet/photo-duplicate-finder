use photo_core::{DuplicateKind, ScanConfig, ScanPreset, SessionStatus};
use scan_engine::db::Database;

#[test]
fn session_create_list_and_checkpoint_round_trip() {
    let db = Database::open_in_memory().expect("db");
    let config = ScanPreset::VisualSimilar.to_config();

    db.create_session("session-1", "Test Session", "/tmp/photos", &config)
        .expect("create");

    let sessions = db.list_sessions().expect("list");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "session-1");
    assert_eq!(sessions[0].status, SessionStatus::Scanning);

    db.upsert_checkpoint(
        "session-1",
        photo_core::ScanPhase::Hashing,
        42,
        100,
        Some("/tmp/photos/a.jpg"),
    )
    .expect("checkpoint");

    let progress = db.get_checkpoint("session-1").expect("progress");
    assert_eq!(progress.files_processed, 42);
    assert_eq!(progress.files_total_estimate, 100);
}

#[test]
fn duplicate_group_insert_and_query() {
    let db = Database::open_in_memory().expect("db");
    let config = ScanConfig::default();
    db.create_session("s1", "Scan", "/tmp", &config).expect("create");

    let tx = db.begin_transaction().expect("tx");
    let id1 = db
        .upsert_file(
            &tx,
            "s1",
            "/tmp/a.jpg",
            100,
            1,
            Some(100),
            Some(100),
            None,
            1,
            None,
            None,
            None,
            None,
        )
        .expect("file1");
    let id2 = db
        .upsert_file(
            &tx,
            "s1",
            "/tmp/b.jpg",
            100,
            1,
            Some(100),
            Some(100),
            None,
            1,
            None,
            None,
            None,
            None,
        )
        .expect("file2");
    tx.commit().expect("commit");

    let group_id = db
        .insert_duplicate_group("s1", DuplicateKind::Exact, 1.0, &[id1, id2])
        .expect("group");

    let groups = db
        .list_duplicate_groups("s1", Some(photo_core::ReviewStatus::Pending), 10, 0)
        .expect("groups");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].id, group_id);

    let detail = db.get_group_detail(group_id).expect("detail");
    assert_eq!(detail.members.len(), 2);
}
