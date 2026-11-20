use nexus_core::{AppSettings, Id, NexusError, ProviderConfig, ProviderKind, Result};
use nexus_db::{repos, Db};

const APP_KEY: &str = "app";
const PROVIDERS_KEY: &str = "providers";
const SECRET_PREFIX: &str = "secret:provider:";

#[derive(Clone)]
pub struct SettingsService {
    pub db: Db,
}

impl SettingsService {
    pub async fn get_app(&self) -> Result<AppSettings> {
        Ok(repos::settings::get_json::<AppSettings>(&self.db, APP_KEY)
            .await?
            .unwrap_or_default())
    }

    /// Fusiona un patch JSON sobre los ajustes actuales.
    pub async fn update_app(&self, patch: serde_json::Value) -> Result<AppSettings> {
        let current = self.get_app().await?;
        let mut base =
            serde_json::to_value(&current).map_err(|e| NexusError::Internal(e.to_string()))?;
        merge(&mut base, &patch);
        let merged: AppSettings = serde_json::from_value(base)
            .map_err(|e| NexusError::validation(format!("ajustes inválidos: {e}")))?;
        repos::settings::set_json(&self.db, APP_KEY, &merged).await?;
        Ok(merged)
    }

    pub async fn get_providers(&self) -> Result<Vec<ProviderConfig>> {
        Ok(
            repos::settings::get_json::<Vec<ProviderConfig>>(&self.db, PROVIDERS_KEY)
                .await?
                .unwrap_or_default(),
        )
    }

    pub async fn get_provider(&self, id: &str) -> Result<ProviderConfig> {
        self.get_providers()
            .await?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| NexusError::not_found("proveedor", id))
    }

    /// Guarda un proveedor. `api_key` solo se persiste si se proporciona;
    /// nunca se devuelve al frontend ni se registra en logs.
    pub async fn save_provider(
        &self,
        mut config: ProviderConfig,
        api_key: Option<String>,
    ) -> Result<ProviderConfig> {
        if config.name.trim().is_empty() {
            return Err(NexusError::validation("el proveedor necesita un nombre"));
        }
        if config.base_url.trim().is_empty()
            || !(config.base_url.starts_with("http://") || config.base_url.starts_with("https://"))
        {
            return Err(NexusError::validation(
                "base_url inválida (debe empezar por http:// o https://)",
            ));
        }
        if config.model.trim().is_empty() {
            return Err(NexusError::validation("el modelo no puede estar vacío"));
        }
        if !(0.0..=2.0).contains(&config.temperature) {
            return Err(NexusError::validation("temperature debe estar entre 0 y 2"));
        }
        if config.max_tokens <= 0 || config.max_tokens > 200_000 {
            return Err(NexusError::validation("max_tokens fuera de rango"));
        }
        if config.requests_per_minute <= 0 || config.requests_per_minute > 600 {
            return Err(NexusError::validation("requests_per_minute fuera de rango"));
        }
        config.base_url = config.base_url.trim().trim_end_matches('/').to_string();

        let mut providers = self.get_providers().await?;
        if let Some(key) = api_key.as_deref().filter(|k| !k.trim().is_empty()) {
            repos::settings::set(
                &self.db,
                &format!("{SECRET_PREFIX}{}", config.id),
                key.trim(),
            )
            .await?;
            config.api_key_set = true;
        } else if let Some(existing) = providers.iter().find(|p| p.id == config.id) {
            config.api_key_set = existing.api_key_set;
        }

        if config.is_default {
            for p in providers.iter_mut() {
                p.is_default = false;
            }
        }
        match providers.iter_mut().find(|p| p.id == config.id) {
            Some(p) => *p = config.clone(),
            None => providers.push(config.clone()),
        }
        if !providers.iter().any(|p| p.is_default) {
            if let Some(first) = providers.first_mut() {
                first.is_default = true;
            }
        }
        repos::settings::set_json(&self.db, PROVIDERS_KEY, &providers).await?;
        let mut app = self.get_app().await?;
        app.default_provider = providers
            .iter()
            .find(|p| p.is_default)
            .map(|p| p.id.clone());
        repos::settings::set_json(&self.db, APP_KEY, &app).await?;
        Ok(config)
    }

    pub async fn delete_provider(&self, id: &str) -> Result<()> {
        let mut providers = self.get_providers().await?;
        let before = providers.len();
        providers.retain(|p| p.id != id);
        if providers.len() == before {
            return Err(NexusError::not_found("proveedor", id));
        }
        repos::settings::delete(&self.db, &format!("{SECRET_PREFIX}{id}")).await?;
        if !providers.iter().any(|p| p.is_default) {
            if let Some(first) = providers.first_mut() {
                first.is_default = true;
            }
        }
        repos::settings::set_json(&self.db, PROVIDERS_KEY, &providers).await?;
        Ok(())
    }

    pub async fn set_default_provider(&self, id: &str) -> Result<Vec<ProviderConfig>> {
        let mut providers = self.get_providers().await?;
        if !providers.iter().any(|p| p.id == id) {
            return Err(NexusError::not_found("proveedor", id));
        }
        for p in providers.iter_mut() {
            p.is_default = p.id == id;
        }
        repos::settings::set_json(&self.db, PROVIDERS_KEY, &providers).await?;
        self.update_app(serde_json::json!({"default_provider": id}))
            .await?;
        Ok(providers)
    }

    /// Solo para uso interno del backend (adaptadores de IA). Nunca exponer por API.
    pub async fn get_secret(&self, provider_id: &str) -> Result<Option<String>> {
        repos::settings::get(&self.db, &format!("{SECRET_PREFIX}{provider_id}")).await
    }

    pub fn new_provider_id() -> Id {
        nexus_core::new_id()
    }

    pub fn default_qwen_config() -> ProviderConfig {
        ProviderConfig {
            id: new_provider_id(),
            name: "Qwen".into(),
            kind: ProviderKind::OpenaiCompatible,
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".into(),
            model: "qwen-max".into(),
            api_key_set: false,
            temperature: 0.7,
            max_tokens: 4096,
            enabled: true,
            is_default: true,
            requests_per_minute: 30,
        }
    }
}

fn new_provider_id() -> Id {
    nexus_core::new_id()
}

fn merge(base: &mut serde_json::Value, patch: &serde_json::Value) {
    match (base, patch) {
        (serde_json::Value::Object(b), serde_json::Value::Object(p)) => {
            for (k, v) in p {
                merge(b.entry(k.clone()).or_insert(serde_json::Value::Null), v);
            }
        }
        (b, p) => *b = p.clone(),
    }
}
