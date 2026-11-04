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
    db.create_session("s1", "Scan", "/tmp", &config)
        .expect("create");

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

#[test]
fn caches_survive_group_recreation_and_clear_without_touching_keepers() {
    let db = Database::open_in_memory().unwrap();
    db.create_session("cache", "Cache", "/tmp", &ScanConfig::default())
        .unwrap();
    let tx = db.begin_transaction().unwrap();
    let a = db
        .upsert_file(
            &tx,
            "cache",
            "/tmp/culling-fixture-a",
            1,
            1,
            None,
            None,
            None,
            1,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    let b = db
        .upsert_file(
            &tx,
            "cache",
            "/tmp/culling-fixture-b",
            1,
            1,
            None,
            None,
            None,
            1,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    tx.commit().unwrap();
    let id = db
        .insert_duplicate_group("cache", DuplicateKind::Exact, 1.0, &[a, b])
        .unwrap();
    db.put_ai_features("hash", "version", r#"{"sharpness":1}"#)
        .unwrap();
    db.put_cached_verdict("group-key", "rules-v1", "verdict")
        .unwrap();
    db.set_ai_result(id, "materialized").unwrap();
    db.clear_groups_for_session("cache").unwrap();
    assert!(db.get_ai_result(id).unwrap().is_none());
    assert!(db.get_ai_features("hash", "version").unwrap().is_some());
    assert_eq!(
        db.get_cached_verdict("group-key", "rules-v1")
            .unwrap()
            .as_deref(),
        Some("verdict")
    );
    let id = db
        .insert_duplicate_group("cache", DuplicateKind::Exact, 1.0, &[a, b])
        .unwrap();
    db.set_keepers(id, &[a, b]).unwrap();
    assert!(db.set_keepers(id, &[i64::MAX]).is_err());
    assert!(db
        .get_group_detail(id)
        .unwrap()
        .members
        .iter()
        .all(|m| m.is_keeper == Some(true)));
    // Keeping all through the selected flow must resolve without calling Trash.
    assert_eq!(db.move_duplicates_to_trash(id).unwrap().trashed_count, 0);
    assert_eq!(
        db.get_group_detail(id).unwrap().review_status,
        photo_core::ReviewStatus::Resolved
    );
    db.clear_ai_cache().unwrap();
    assert!(db.get_ai_features("hash", "version").unwrap().is_none());
    assert!(db
        .get_cached_verdict("group-key", "rules-v1")
        .unwrap()
        .is_none());
    assert!(db
        .get_group_detail(id)
        .unwrap()
        .members
        .iter()
        .all(|m| m.is_keeper == Some(true)));
    db.set_keepers(id, &[]).unwrap();
    let detail = db.get_group_detail(id).unwrap();
    assert_eq!(detail.review_status, photo_core::ReviewStatus::Pending);
    assert!(detail.members.iter().all(|m| m.is_keeper.is_none()));
    assert!(db.keep_selected_and_trash(id, &[]).is_err());
}
