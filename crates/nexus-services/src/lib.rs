//! nexus-services: servicios de aplicación del monolito.
//! Validación, reglas de negocio, auditoría con deshacer y composición de vistas.
//! La IA es opcional: todo lo de aquí funciona sin ningún proveedor conectado.

pub mod activity;
pub mod backup;
pub mod capture;
pub mod chat;
pub mod dashboard;
pub mod events;
pub mod inbox;
pub mod notes;
pub mod projects;
pub mod settings;
pub mod tasks;
pub mod validate;

use nexus_db::Db;

#[derive(Clone)]
pub struct Services {
    pub db: Db,
    pub tasks: tasks::TaskService,
    pub projects: projects::ProjectService,
    pub notes: notes::NoteService,
    pub events: events::EventService,
    pub inbox: inbox::InboxService,
    pub chat: chat::ChatService,
    pub activity: activity::ActivityService,
    pub settings: settings::SettingsService,
    pub dashboard: dashboard::DashboardService,
    pub backup: backup::BackupService,
}

impl Services {
    pub fn new(db: Db) -> Self {
        Self {
            tasks: tasks::TaskService { db: db.clone() },
            projects: projects::ProjectService { db: db.clone() },
            notes: notes::NoteService { db: db.clone() },
            events: events::EventService { db: db.clone() },
            inbox: inbox::InboxService { db: db.clone() },
            chat: chat::ChatService { db: db.clone() },
            activity: activity::ActivityService { db: db.clone() },
            settings: settings::SettingsService { db: db.clone() },
            dashboard: dashboard::DashboardService { db: db.clone() },
            backup: backup::BackupService { db: db.clone() },
            db,
        }
    }
}
