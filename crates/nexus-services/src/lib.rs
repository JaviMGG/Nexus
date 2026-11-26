//! nexus-services: servicios de aplicación del monolito.
//! Validación, reglas de negocio, auditoría con deshacer y composición de vistas.
//! La IA es opcional: todo lo de aquí funciona sin ningún proveedor conectado.

pub mod activity;
pub mod backup;
pub mod canvas;
pub mod capture;
pub mod chat;
pub mod dashboard;
pub mod databases;
pub mod events;
pub mod files;
pub mod inbox;
pub mod mail;
pub mod meals;
pub mod notes;
pub mod projects;
pub mod settings;
pub mod tasks;
pub mod validate;

use nexus_db::Db;

pub use mail::{MailAccount, MailMessage};

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
    pub canvas: canvas::CanvasService,
    pub databases: databases::DatabaseService,
    pub meals: meals::MealService,
    pub files: files::FileService,
    pub mail: mail::MailService,
}

impl Services {
    pub fn new(db: Db) -> Self {
        Self::with_data_dir(db, &default_data_dir())
    }

    pub fn with_data_dir(db: Db, data_dir: &std::path::Path) -> Self {
        Self {
            mail: mail::MailService::new(db.clone()),
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
            canvas: canvas::CanvasService { db: db.clone() },
            databases: databases::DatabaseService { db: db.clone() },
            meals: meals::MealService { db: db.clone() },
            files: files::FileService::new(db.clone(), data_dir),
            db,
        }
    }
}

/// Directorio de datos por defecto cuando el llamante no lo especifica (tests).
fn default_data_dir() -> std::path::PathBuf {
    std::env::var("NEXUS_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let base = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            std::path::PathBuf::from(base).join(".local/share/nexus")
        })
}
