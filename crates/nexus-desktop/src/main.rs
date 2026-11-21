#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nexus_app::AppContext;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

#[derive(Clone, Serialize)]
struct ServerInfo {
    port: u16,
    token: String,
}

#[tauri::command]
fn server_info(state: tauri::State<'_, ServerInfo>) -> ServerInfo {
    state.inner().clone()
}

/// Estado compartido con el manejador de cierre de ventana.
struct BackgroundState {
    ctx: Arc<AppContext>,
    run_in_background: AtomicBool,
}

/// Si la bandeja del sistema pudo crearse (en Linux puede faltar la librería).
struct TrayAvailable(bool);

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nexus=info,tower_http=warn".into()),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            let port: u16 = std::env::var("NEXUS_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(47800);

            // El servicio local se arranca antes de mostrar la ventana para que
            // el frontend siempre encuentre la API disponible.
            let (ctx, server) = tauri::async_runtime::block_on(async {
                let ctx = Arc::new(AppContext::new(data_dir).await.map_err(|e| e.to_string())?);
                let (port, token) = nexus_api::serve(ctx.clone(), port)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>((ctx, ServerInfo { port, token }))
            })?;
            app.manage(server);

            let settings =
                tauri::async_runtime::block_on(ctx.services.settings.get_app()).unwrap_or_default();
            let background = BackgroundState {
                ctx: ctx.clone(),
                run_in_background: AtomicBool::new(settings.automations.run_in_background),
            };
            app.manage(background);

            // Refresca el flag cuando el usuario cambia el ajuste desde la UI.
            let handle = app.handle().clone();
            let mut events = ctx.events.subscribe();
            tauri::async_runtime::spawn(async move {
                loop {
                    match events.recv().await {
                        Ok(nexus_core::AppEvent::DataChanged { entity })
                            if entity == "settings" =>
                        {
                            let state = handle.state::<BackgroundState>();
                            if let Ok(app_settings) = state.ctx.services.settings.get_app().await {
                                state.run_in_background.store(
                                    app_settings.automations.run_in_background,
                                    Ordering::Relaxed,
                                );
                            }
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });

            // Bandeja del sistema: NEXUS sigue vivo (y las automatizaciones
            // corriendo) aunque la ventana esté cerrada. En Linux requiere
            // libayatana-appindicator3; si no está, libappindicator entra en
            // pánico al cargar la librería: se captura y se degrada con un
            // aviso (sin bandeja no se oculta la ventana al cerrar).
            let build_tray = || -> tauri::Result<()> {
                let show = MenuItem::with_id(app, "show", "Abrir NEXUS", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Salir de NEXUS", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show, &quit])?;
                let mut tray = TrayIconBuilder::with_id("nexus-tray")
                    .tooltip("NEXUS — automatizaciones activas en segundo plano")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => show_main_window(app),
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            show_main_window(tray.app_handle());
                        }
                    });
                if let Some(icon) = app.default_window_icon() {
                    tray = tray.icon(icon.clone());
                }
                tray.build(app)?;
                Ok(())
            };
            // Silencia el hook de pánico solo durante la creación de la bandeja
            // para no imprimir un trace alarmante cuando falta la librería.
            let prev_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let tray_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(build_tray));
            std::panic::set_hook(prev_hook);
            let tray_available = match tray_result {
                Ok(Ok(())) => true,
                Ok(Err(e)) => {
                    tracing::warn!("bandeja del sistema no disponible: {e}");
                    false
                }
                Err(_) => {
                    tracing::warn!(
                        "bandeja del sistema no disponible: falta libayatana-appindicator3 \
                         (instálala para el modo en segundo plano; la app funciona sin bandeja)"
                    );
                    false
                }
            };
            app.manage(TrayAvailable(tray_available));

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<BackgroundState>();
                let tray = window.state::<TrayAvailable>();
                if tray.0 && state.run_in_background.load(Ordering::Relaxed) {
                    // Cerrar = minimizar a la bandeja; el motor sigue corriendo.
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![server_info])
        .run(tauri::generate_context!())
        .expect("error ejecutando NEXUS");
}
