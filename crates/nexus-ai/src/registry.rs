//! Registro de proveedores: carga desde Ajustes, fallback de entorno (Qwen),
//! selección del proveedor por defecto y estado cacheado con health check.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nexus_core::{AiState, AiStatus, NexusError, ProviderConfig, ProviderKind, Result};
use nexus_services::Services;
use tokio::sync::Mutex;

use crate::openai::OpenAiCompatibleProvider;
use crate::types::{AiMessage, AiProvider, ChatRequest};

const HEALTH_TIMEOUT: Duration = Duration::from_secs(8);
const STATUS_CACHE_TTL: Duration = Duration::from_secs(30);
const DEFAULT_QWEN_BASE_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1";

pub struct ProviderRegistry {
    services: Services,
    providers: Mutex<HashMap<String, Arc<dyn AiProvider>>>,
    order: Mutex<Vec<String>>,
    status_cache: Mutex<Option<(Instant, AiStatus)>>,
}

impl ProviderRegistry {
    pub fn new(services: Services) -> Self {
        Self {
            services,
            providers: Mutex::new(HashMap::new()),
            order: Mutex::new(Vec::new()),
            status_cache: Mutex::new(None),
        }
    }

    /// Recarga proveedores guardados (enabled + api_key_set) y el fallback
    /// `QWEN_API_KEY` si no hay ninguno usable. Nunca registra la API key.
    pub async fn refresh(&self) -> Result<()> {
        let configs = self.services.settings.get_providers().await?;
        let mut map: HashMap<String, Arc<dyn AiProvider>> = HashMap::new();
        let mut order = Vec::new();
        for cfg in configs {
            if !cfg.enabled || !cfg.api_key_set {
                continue;
            }
            let key = match self.services.settings.get_secret(&cfg.id).await? {
                Some(k) if !k.trim().is_empty() => k,
                _ => continue,
            };
            let provider = build_provider(cfg.clone(), key)?;
            order.push(cfg.id.clone());
            map.insert(cfg.id.clone(), provider);
        }
        if map.is_empty() {
            if let Some(provider) = env_provider() {
                let id = provider.config().id;
                order.push(id.clone());
                map.insert(id, provider);
            }
        }
        *self.providers.lock().await = map;
        *self.order.lock().await = order;
        *self.status_cache.lock().await = None;
        Ok(())
    }

    pub async fn default_provider(&self) -> Result<Arc<dyn AiProvider>> {
        let app = self.services.settings.get_app().await.unwrap_or_default();
        let providers = self.providers.lock().await;
        if providers.is_empty() {
            return Err(no_provider_error());
        }
        if let Some(id) = &app.default_provider {
            if let Some(p) = providers.get(id) {
                return Ok(p.clone());
            }
        }
        let order = self.order.lock().await;
        for id in order.iter() {
            if let Some(p) = providers.get(id) {
                return Ok(p.clone());
            }
        }
        providers
            .values()
            .next()
            .cloned()
            .ok_or_else(no_provider_error)
    }

    pub async fn get(&self, id: &str) -> Result<Arc<dyn AiProvider>> {
        self.providers
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| NexusError::not_found("proveedor", id))
    }

    /// Health check del proveedor por defecto con timeout ~8s y cache ~30s.
    /// Nunca entra en pánico ni devuelve error: sin proveedor → Disconnected.
    pub async fn status(&self) -> AiStatus {
        {
            let cache = self.status_cache.lock().await;
            if let Some((at, status)) = cache.as_ref() {
                if at.elapsed() < STATUS_CACHE_TTL {
                    return status.clone();
                }
            }
        }
        let status = match self.default_provider().await {
            Ok(provider) => {
                let cfg = provider.config();
                match tokio::time::timeout(HEALTH_TIMEOUT, provider.health()).await {
                    Ok(Ok(desc)) => AiStatus {
                        state: AiState::Connected,
                        provider: Some(cfg.name),
                        model: Some(cfg.model),
                        message: Some(desc),
                    },
                    Ok(Err(e)) => AiStatus {
                        state: AiState::Error,
                        provider: Some(cfg.name),
                        model: Some(cfg.model),
                        message: Some(e.to_string()),
                    },
                    Err(_) => AiStatus {
                        state: AiState::Error,
                        provider: Some(cfg.name),
                        model: Some(cfg.model),
                        message: Some("el proveedor no respondió al health check (timeout)".into()),
                    },
                }
            }
            Err(_) => AiStatus::default(),
        };
        *self.status_cache.lock().await = Some((Instant::now(), status.clone()));
        status
    }

    /// Prueba real: chat mínimo "ping". Devuelve descripción del modelo.
    pub async fn test(&self, id: Option<&str>) -> Result<String> {
        let provider = match id {
            Some(id) => self.get(id).await?,
            None => self.default_provider().await?,
        };
        let cfg = provider.config();
        let resp = tokio::time::timeout(
            HEALTH_TIMEOUT + Duration::from_secs(12),
            provider.chat(ChatRequest {
                messages: vec![AiMessage::user("ping")],
                max_tokens: Some(16),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| NexusError::Ai("la prueba no respondió a tiempo (timeout)".into()))??;
        let preview: String = resp.content.chars().take(80).collect();
        Ok(format!(
            "{} · modelo {} · respuesta: {}",
            cfg.name,
            cfg.model,
            if preview.trim().is_empty() {
                "(vacía)".to_string()
            } else {
                preview
            }
        ))
    }

    pub async fn list_configs(&self) -> Vec<ProviderConfig> {
        let providers = self.providers.lock().await;
        let order = self.order.lock().await;
        order
            .iter()
            .filter_map(|id| providers.get(id).map(|p| p.config()))
            .collect()
    }

    /// Inyecta un proveedor ya construido (p. ej. simulado en tests) y lo
    /// convierte en el preferido. No persiste nada.
    #[doc(hidden)]
    pub async fn insert_test_provider(&self, provider: Arc<dyn AiProvider>) {
        let id = provider.config().id;
        self.providers.lock().await.insert(id.clone(), provider);
        let mut order = self.order.lock().await;
        order.retain(|i| i != &id);
        order.insert(0, id);
        *self.status_cache.lock().await = None;
    }
}

fn no_provider_error() -> NexusError {
    NexusError::AiUnavailable(
        "No hay proveedor de IA configurado. Añade uno en Ajustes o define QWEN_API_KEY. NEXUS funciona en modo manual.".into(),
    )
}

fn build_provider(cfg: ProviderConfig, key: String) -> Result<Arc<dyn AiProvider>> {
    match cfg.kind {
        ProviderKind::OpenaiCompatible => Ok(Arc::new(OpenAiCompatibleProvider::new(cfg, key)?)),
    }
}

fn env_provider() -> Option<Arc<dyn AiProvider>> {
    let key = std::env::var("QWEN_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())?;
    let cfg = ProviderConfig {
        id: "env-qwen".into(),
        name: "Qwen (env)".into(),
        kind: ProviderKind::OpenaiCompatible,
        base_url: std::env::var("QWEN_BASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_QWEN_BASE_URL.into()),
        model: std::env::var("QWEN_MODEL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "qwen-max".into()),
        api_key_set: true,
        temperature: 0.7,
        max_tokens: 4096,
        enabled: true,
        is_default: true,
        requests_per_minute: 30,
    };
    match build_provider(cfg, key) {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::warn!("no se pudo crear el proveedor Qwen de entorno: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_db::Db;

    static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    async fn services() -> Services {
        Services::new(Db::connect_memory().await.unwrap())
    }

    #[tokio::test]
    async fn empty_registry_is_unavailable_and_disconnected() {
        let _guard = ENV_LOCK.lock().await;
        std::env::remove_var("QWEN_API_KEY");
        let registry = ProviderRegistry::new(services().await);
        registry.refresh().await.unwrap();
        let err = match registry.default_provider().await {
            Ok(_) => panic!("se esperaba AiUnavailable"),
            Err(e) => e,
        };
        assert!(matches!(err, NexusError::AiUnavailable(_)));
        let status = registry.status().await;
        assert_eq!(status.state, AiState::Disconnected);
        assert!(registry.list_configs().await.is_empty());
    }

    #[tokio::test]
    async fn env_fallback_creates_qwen_provider() {
        let _guard = ENV_LOCK.lock().await;
        std::env::set_var("QWEN_API_KEY", "test-key-not-real");
        std::env::remove_var("QWEN_BASE_URL");
        std::env::remove_var("QWEN_MODEL");
        let registry = ProviderRegistry::new(services().await);
        registry.refresh().await.unwrap();
        let provider = registry.default_provider().await.unwrap();
        let cfg = provider.config();
        assert_eq!(cfg.id, "env-qwen");
        assert_eq!(cfg.base_url, DEFAULT_QWEN_BASE_URL);
        assert_eq!(cfg.model, "qwen-max");
        std::env::remove_var("QWEN_API_KEY");
    }

    #[tokio::test]
    async fn loads_saved_provider_with_secret() {
        let _guard = ENV_LOCK.lock().await;
        std::env::remove_var("QWEN_API_KEY");
        let services = services().await;
        let mut cfg = nexus_services::settings::SettingsService::default_qwen_config();
        cfg.is_default = true;
        let saved = services
            .settings
            .save_provider(cfg, Some("secret-test".into()))
            .await
            .unwrap();
        let registry = ProviderRegistry::new(services);
        registry.refresh().await.unwrap();
        let provider = registry.get(&saved.id).await.unwrap();
        assert_eq!(provider.config().model, "qwen-max");
        assert!(registry.get("no-existe").await.is_err());
        let configs = registry.list_configs().await;
        assert_eq!(configs.len(), 1);
    }
}
