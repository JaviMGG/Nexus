use chrono::{Duration, Utc};
use nexus_core::{DashboardStats, DashboardSummary, Result, Task, TaskFilter, TaskStatus};
use nexus_db::{repos, Db};

#[derive(Clone)]
pub struct DashboardService {
    pub db: Db,
}

impl DashboardService {
    pub async fn summary(&self, user_name: &str) -> Result<DashboardSummary> {
        let now = Utc::now();
        let today = now.format("%Y-%m-%d").to_string();
        let week_ago = (now - Duration::days(7)).to_rfc3339();
        let start_of_day = now.format("%Y-%m-%dT00:00:00").to_string();
        let in_7_days = (now + Duration::days(7)).format("%Y-%m-%d").to_string();

        let open = repos::tasks::list(
            &self.db,
            &TaskFilter {
                root_only: Some(true),
                ..Default::default()
            },
        )
        .await?;
        let open_active: Vec<Task> = open
            .into_iter()
            .filter(|t| t.status == TaskStatus::Todo || t.status == TaskStatus::InProgress)
            .collect();

        let is_overdue = |t: &Task| {
            t.due_date
                .as_deref()
                .map(|d| &d[..10.min(d.len())] < today.as_str())
                .unwrap_or(false)
        };

        let overdue: Vec<Task> = open_active
            .iter()
            .filter(|t| is_overdue(t))
            .cloned()
            .collect();
        let tasks_today: Vec<Task> = open_active
            .iter()
            .filter(|t| {
                t.due_date
                    .as_deref()
                    .map(|d| &d[..10.min(d.len())] == today.as_str())
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        let mut priority_tasks: Vec<Task> = open_active
            .iter()
            .filter(|t| {
                matches!(
                    t.priority,
                    nexus_core::Priority::High | nexus_core::Priority::Urgent
                )
            })
            .cloned()
            .collect();
        let rank = |p: nexus_core::Priority| match p {
            nexus_core::Priority::Urgent => 3,
            nexus_core::Priority::High => 2,
            nexus_core::Priority::Medium => 1,
            nexus_core::Priority::Low => 0,
        };
        priority_tasks.sort_by_key(|t| std::cmp::Reverse(rank(t.priority)));
        priority_tasks.truncate(8);
        let mut upcoming: Vec<Task> = open_active
            .iter()
            .filter(|t| {
                t.due_date
                    .as_deref()
                    .map(|d| {
                        let d = &d[..10.min(d.len())];
                        d > today.as_str() && d <= in_7_days.as_str()
                    })
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        upcoming.sort_by(|a, b| a.due_date.cmp(&b.due_date));
        upcoming.truncate(8);

        let projects = crate::projects::ProjectService {
            db: self.db.clone(),
        }
        .active_with_progress()
        .await?;

        let stats = DashboardStats {
            open_tasks: repos::tasks::count_open(&self.db).await?,
            completed_today: repos::tasks::count_completed_since(&self.db, &start_of_day).await?,
            completed_week: repos::tasks::count_completed_since(&self.db, &week_ago).await?,
            overdue: overdue.len() as i64,
            total_notes: repos::notes::count(&self.db).await?,
            inbox_pending: repos::inbox::count_pending(&self.db).await?,
            active_projects: projects.len() as i64,
        };

        let recent_activity = repos::activity::list(&self.db, 10, None, None).await?;
        let recent_runs = repos::runs::list_runs(&self.db, 5).await?;

        Ok(DashboardSummary {
            user_name: user_name.to_string(),
            today,
            tasks_today,
            overdue,
            priority_tasks,
            upcoming,
            active_projects: projects,
            stats,
            recent_activity,
            recent_runs,
        })
    }
}
