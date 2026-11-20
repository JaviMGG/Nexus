use nexus_core::*;
use nexus_db::Db;
use nexus_services::Services;

async fn setup() -> Services {
    Services::new(Db::connect_memory().await.unwrap())
}

#[tokio::test]
async fn task_crud_and_validation() {
    let s = setup().await;
    let t = s
        .tasks
        .create(
            CreateTask {
                title: "  Preparar portfolio  ".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(t.title, "Preparar portfolio");
    assert_eq!(t.status, TaskStatus::Todo);

    let t = s
        .tasks
        .update(
            &t.id,
            UpdateTask {
                status: Some(TaskStatus::Done),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(t.status, TaskStatus::Done);
    assert!(t.completed_at.is_some());

    let list = s.tasks.list(TaskFilter::default()).await.unwrap();
    assert_eq!(list.len(), 1);

    s.tasks.delete(&t.id, "user").await.unwrap();
    assert!(s
        .tasks
        .list(TaskFilter::default())
        .await
        .unwrap()
        .is_empty());

    let err = s
        .tasks
        .create(
            CreateTask {
                title: "   ".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");

    let err = s
        .tasks
        .create(
            CreateTask {
                title: "x".into(),
                due_date: Some("no-fecha".into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");
}

#[tokio::test]
async fn subtasks_and_cycle_rejection() {
    let s = setup().await;
    let parent = s
        .tasks
        .create(
            CreateTask {
                title: "Padre".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let child = s
        .tasks
        .create(
            CreateTask {
                title: "Hijo".into(),
                parent_id: Some(parent.id.clone()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let detail = s.tasks.get(&parent.id).await.unwrap();
    assert_eq!(detail.subtasks.len(), 1);
    let err = s
        .tasks
        .update(
            &parent.id,
            UpdateTask {
                parent_id: Some(Some(child.id.clone())),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");
}

#[tokio::test]
async fn project_progress_calculation() {
    let s = setup().await;
    let p = s
        .projects
        .create(
            CreateProject {
                name: "NEXUS".into(),
                goal: "Centro de control".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    for i in 0..4 {
        s.tasks
            .create(
                CreateTask {
                    title: format!("t{i}"),
                    project_id: Some(p.id.clone()),
                    ..Default::default()
                },
                "user",
            )
            .await
            .unwrap();
    }
    let tasks = s
        .tasks
        .list(TaskFilter {
            project_id: Some(p.id.clone()),
            ..Default::default()
        })
        .await
        .unwrap();
    s.tasks
        .update(
            &tasks[0].id,
            UpdateTask {
                status: Some(TaskStatus::Done),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let detail = s.projects.get(&p.id).await.unwrap();
    assert_eq!(detail.progress.total, 4);
    assert_eq!(detail.progress.done, 1);
    assert!((detail.progress.progress - 0.25).abs() < f64::EPSILON);
}

#[tokio::test]
async fn notes_fts_search() {
    let s = setup().await;
    s.notes
        .create(
            CreateNote {
                title: "Rust async".into(),
                content: "tokio y axum para servicios".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    s.notes
        .create(
            CreateNote {
                title: "Recetas".into(),
                content: "tortilla de patatas".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let found = s.notes.search("tokio", 10).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title, "Rust async");
    let found = s.notes.search("tortilla", 10).await.unwrap();
    assert_eq!(found.len(), 1);
    // La búsqueda no debe romper con caracteres especiales.
    let _ = s.notes.search("\" OR 1=1 --", 10).await.unwrap();
}

#[tokio::test]
async fn inbox_process_into_task() {
    let s = setup().await;
    let item = s
        .inbox
        .create("Tengo que llamar al banco", "user")
        .await
        .unwrap();
    let id = s
        .inbox
        .process(&item.id, "task", None, &s, "user")
        .await
        .unwrap();
    let task = s.tasks.get(&id).await.unwrap();
    assert!(task.task.title.contains("banco"));
    let items = s.inbox.list(Some(InboxStatus::Processed)).await.unwrap();
    assert_eq!(items.len(), 1);
    let err = s
        .inbox
        .process(&item.id, "task", None, &s, "user")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "conflict");
}

#[tokio::test]
async fn undo_delete_and_update() {
    let s = setup().await;
    let t = s
        .tasks
        .create(
            CreateTask {
                title: "Original".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    s.tasks
        .update(
            &t.id,
            UpdateTask {
                title: Some("Cambiado".into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let events = s
        .activity
        .list(10, Some("task"), Some(&t.id))
        .await
        .unwrap();
    let update_event = events.iter().find(|e| e.kind == "updated").unwrap();
    s.activity.undo(&update_event.id).await.unwrap();
    assert_eq!(s.tasks.get(&t.id).await.unwrap().task.title, "Original");

    s.tasks.delete(&t.id, "user").await.unwrap();
    let events = s
        .activity
        .list(10, Some("task"), Some(&t.id))
        .await
        .unwrap();
    let delete_event = events.iter().find(|e| e.kind == "deleted").unwrap();
    s.activity.undo(&delete_event.id).await.unwrap();
    assert_eq!(s.tasks.get(&t.id).await.unwrap().task.title, "Original");
}

#[tokio::test]
async fn provider_secrets_never_leak() {
    let s = setup().await;
    let mut cfg = nexus_services::settings::SettingsService::default_qwen_config();
    cfg.id = "p1".into();
    let saved = s
        .settings
        .save_provider(cfg, Some("sk-secret-123".into()))
        .await
        .unwrap();
    assert!(saved.api_key_set);
    let providers = s.settings.get_providers().await.unwrap();
    let json = serde_json::to_string(&providers).unwrap();
    assert!(!json.contains("sk-secret-123"));
    assert_eq!(
        s.settings.get_secret("p1").await.unwrap().as_deref(),
        Some("sk-secret-123")
    );
    let export = s.backup.export_json().await.unwrap();
    assert!(!export.to_string().contains("sk-secret-123"));
}

#[tokio::test]
async fn persistence_across_reopen() {
    let dir = std::env::temp_dir().join(format!("nexus-test-{}", new_id()));
    let path = dir.join("nexus.sqlite");
    {
        let db = Db::connect(&path).await.unwrap();
        let s = Services::new(db);
        s.tasks
            .create(
                CreateTask {
                    title: "Persistente".into(),
                    ..Default::default()
                },
                "user",
            )
            .await
            .unwrap();
        s.settings
            .update_app(serde_json::json!({"user_name": "Javier", "palette": "ocean"}))
            .await
            .unwrap();
    }
    {
        let db = Db::connect(&path).await.unwrap();
        let s = Services::new(db);
        let tasks = s.tasks.list(TaskFilter::default()).await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Persistente");
        let app = s.settings.get_app().await.unwrap();
        assert_eq!(app.user_name, "Javier");
        assert_eq!(app.palette, "ocean");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn backup_and_restore() {
    let dir = std::env::temp_dir().join(format!("nexus-backup-test-{}", new_id()));
    let path = dir.join("nexus.sqlite");
    let db = Db::connect(&path).await.unwrap();
    let s = Services::new(db);
    let t = s
        .tasks
        .create(
            CreateTask {
                title: "Antes del backup".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let backup = s.backup.create_backup(&dir.join("backups")).await.unwrap();
    s.tasks.delete(&t.id, "user").await.unwrap();
    assert!(s
        .tasks
        .list(TaskFilter::default())
        .await
        .unwrap()
        .is_empty());
    s.backup.restore(&backup).await.unwrap();
    let tasks = s.tasks.list(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title, "Antes del backup");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn dashboard_summary_counts() {
    let s = setup().await;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    s.tasks
        .create(
            CreateTask {
                title: "Hoy".into(),
                due_date: Some(today),
                priority: Some(Priority::Urgent),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    s.tasks
        .create(
            CreateTask {
                title: "Atrasada".into(),
                due_date: Some("2020-01-01".into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    s.inbox.create("idea suelta", "user").await.unwrap();
    let d = s.dashboard.summary("Javier").await.unwrap();
    assert_eq!(d.tasks_today.len(), 1);
    assert_eq!(d.overdue.len(), 1);
    assert_eq!(d.stats.open_tasks, 2);
    assert_eq!(d.stats.inbox_pending, 1);
    assert_eq!(d.priority_tasks.len(), 1);
}

#[tokio::test]
async fn event_crud_and_validation() {
    let s = setup().await;
    let e = s
        .events
        .create(
            CreateEvent {
                title: "  Reunión semanal  ".into(),
                date: "2026-11-20".into(),
                start_time: Some("10:00".into()),
                end_time: Some("11:30".into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(e.title, "Reunión semanal");
    assert_eq!(e.date, "2026-11-20");
    assert_eq!(e.color, EventColor::Primary);
    assert!(!e.all_day);

    let e = s
        .events
        .update(
            &e.id,
            UpdateEvent {
                title: Some("Reunión mensual".into()),
                color: Some(EventColor::Warning),
                location: Some(Some("Sala 2".into())),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(e.title, "Reunión mensual");
    assert_eq!(e.color, EventColor::Warning);
    assert_eq!(e.location, "Sala 2");

    let e = s
        .events
        .update(
            &e.id,
            UpdateEvent {
                all_day: Some(true),
                location: Some(None),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    assert!(e.all_day);
    assert!(e.start_time.is_none());
    assert!(e.end_time.is_none());
    assert_eq!(e.location, "");

    assert_eq!(s.events.get(&e.id).await.unwrap().title, "Reunión mensual");
    s.events.delete(&e.id, "user").await.unwrap();
    assert!(s
        .events
        .list(EventFilter::default())
        .await
        .unwrap()
        .is_empty());
    let restored = s.events.restore(&e.id, "user").await.unwrap();
    assert_eq!(restored.title, "Reunión mensual");

    for bad_date in ["", "no-fecha", "2026-13-01", "20-11-2026"] {
        let err = s
            .events
            .create(
                CreateEvent {
                    title: "x".into(),
                    date: bad_date.into(),
                    ..Default::default()
                },
                "user",
            )
            .await
            .unwrap_err();
        assert_eq!(err.code(), "validation", "fecha {bad_date}");
    }
    for bad_time in ["25:00", "10:99", "diez"] {
        let err = s
            .events
            .create(
                CreateEvent {
                    title: "x".into(),
                    date: "2026-11-20".into(),
                    start_time: Some(bad_time.into()),
                    ..Default::default()
                },
                "user",
            )
            .await
            .unwrap_err();
        assert_eq!(err.code(), "validation", "hora {bad_time}");
    }
    let err = s
        .events
        .create(
            CreateEvent {
                title: "x".into(),
                date: "2026-11-20".into(),
                start_time: Some("12:00".into()),
                end_time: Some("11:00".into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");
    let err = serde_json::from_value::<CreateEvent>(serde_json::json!({
        "title": "x", "date": "2026-11-20", "color": "rosa"
    }))
    .unwrap_err();
    assert!(err.to_string().contains("color") || err.to_string().contains("variant"));
    let err = s
        .events
        .create(
            CreateEvent {
                title: "   ".into(),
                date: "2026-11-20".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");
    let err = s
        .events
        .create(
            CreateEvent {
                title: "x".into(),
                date: "2026-11-20".into(),
                location: "a".repeat(301),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation");
    let err = s
        .events
        .create(
            CreateEvent {
                title: "x".into(),
                date: "2026-11-20".into(),
                project_id: Some("no-existe".into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "not_found");
}

#[tokio::test]
async fn event_range_filter_and_links() {
    let s = setup().await;
    let p = s
        .projects
        .create(
            CreateProject {
                name: "Calendario".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let t = s
        .tasks
        .create(
            CreateTask {
                title: "Preparar demo".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    for date in ["2026-10-31", "2026-11-01", "2026-11-15", "2026-12-01"] {
        s.events
            .create(
                CreateEvent {
                    title: format!("Evento {date}"),
                    date: date.into(),
                    project_id: Some(p.id.clone()),
                    ..Default::default()
                },
                "user",
            )
            .await
            .unwrap();
    }
    let linked = s
        .events
        .create(
            CreateEvent {
                title: "Demo".into(),
                date: "2026-11-20".into(),
                task_id: Some(t.id.clone()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    assert_eq!(linked.task_id.as_deref(), Some(t.id.as_str()));

    let nov = s
        .events
        .list(EventFilter {
            from: Some("2026-11-01".into()),
            to: Some("2026-11-30".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(nov.len(), 3);
    assert!(nov.windows(2).all(|w| w[0].date <= w[1].date));

    let by_project = s
        .events
        .list(EventFilter {
            project_id: Some(p.id.clone()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(by_project.len(), 4);

    s.events.delete(&linked.id, "user").await.unwrap();
    let with_deleted = s
        .events
        .list(EventFilter {
            include_deleted: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(with_deleted.len(), 5);
}

#[tokio::test]
async fn event_undo_delete_and_update() {
    let s = setup().await;
    let e = s
        .events
        .create(
            CreateEvent {
                title: "Original".into(),
                date: "2026-11-20".into(),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    s.events
        .update(
            &e.id,
            UpdateEvent {
                title: Some("Cambiado".into()),
                date: Some("2026-11-21".into()),
                ..Default::default()
            },
            "user",
        )
        .await
        .unwrap();
    let events = s
        .activity
        .list(10, Some("event"), Some(&e.id))
        .await
        .unwrap();
    let update_event = events.iter().find(|a| a.kind == "updated").unwrap();
    s.activity.undo(&update_event.id).await.unwrap();
    let restored = s.events.get(&e.id).await.unwrap();
    assert_eq!(restored.title, "Original");
    assert_eq!(restored.date, "2026-11-20");

    s.events.delete(&e.id, "user").await.unwrap();
    let events = s
        .activity
        .list(10, Some("event"), Some(&e.id))
        .await
        .unwrap();
    let delete_event = events.iter().find(|a| a.kind == "deleted").unwrap();
    s.activity.undo(&delete_event.id).await.unwrap();
    assert_eq!(s.events.get(&e.id).await.unwrap().title, "Original");

    let create_event = events.iter().find(|a| a.kind == "created").unwrap();
    s.activity.undo(&create_event.id).await.unwrap();
    assert!(s
        .events
        .list(EventFilter::default())
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn heuristic_capture_without_ai() {
    let r = nexus_services::capture::heuristic_capture(
        "Tengo que preparar el portfolio antes del viernes",
    );
    assert_eq!(r.kind, "task");
    assert!(!r.ai_used);
    assert!(r.task.unwrap().due_date.is_some());
}
