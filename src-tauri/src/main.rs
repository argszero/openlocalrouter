//! OpenLocalRouter 桌面入口
//!
//! Tauri 托盘外壳 — 无窗口，只有托盘菜单（打开管理界面 / 退出）。
//! 核心逻辑委托给 openlocalrouter-core 库。

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
#[cfg(target_os = "macos")]
use tauri::ActivationPolicy;
use tauri::{Manager, RunEvent};
use tauri_plugin_opener::OpenerExt;

use openlocalrouter_core::config::AppConfig;
use openlocalrouter_core::db;
use openlocalrouter_core::init_logging;
use openlocalrouter_core::run_backend;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config = AppConfig::load();
            init_logging(&config.log_level);

            let db_path = config.database_path();
            let db = db::Database::open(&db_path).expect("无法打开数据库");

            log::info!("数据库已就绪: {}", db_path.display());
            log::info!(
                "代理端口: {}:{}, 管理端口: {}:{}",
                config.listen_address,
                config.listen_port,
                config.listen_address,
                config.admin_port
            );

            // 启动后端服务（单端口设计，返回单个 JoinHandle）
            let backend_handle = tauri::async_runtime::block_on(run_backend(config.clone(), db));

            // 在 app 存储中保存句柄，方便退出时清理
            app.manage(BackendHandles { backend_handle });
            app.manage(config.clone());

            #[cfg(target_os = "macos")]
            app.set_activation_policy(ActivationPolicy::Accessory);

            Ok(())
        })
        .on_window_event(|_window, event| {
            // 无窗口应用，忽略窗口事件
            let _ = event;
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Ready = event {
                // 创建托盘菜单
                let open_item =
                    MenuItem::with_id(app, "open_admin", "🔗 打开管理界面", true, None::<&str>)
                        .expect("failed to create menu item");
                let quit_item = MenuItem::with_id(app, "quit", "✕  退出", true, None::<&str>)
                    .expect("failed to create menu item");
                let menu = Menu::with_items(app, &[&open_item, &quit_item])
                    .expect("failed to create menu");

                let _tray = TrayIconBuilder::new()
                    .menu(&menu)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "open_admin" => {
                            let config = app.state::<AppConfig>();
                            let url =
                                format!("http://{}:{}", config.listen_address, config.admin_port);
                            let _ = app.opener().open_url(&url, None::<&str>);
                        }
                        "quit" => {
                            let handles = app.state::<BackendHandles>();
                            handles.backend_handle.abort();
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            let config = app.state::<AppConfig>();
                            let url =
                                format!("http://{}:{}", config.listen_address, config.admin_port);
                            let _ = app.opener().open_url(&url, None::<&str>);
                        }
                    })
                    .build(app)
                    .expect("failed to build tray");

                // 自动打开浏览器
                let config = app.state::<AppConfig>();
                let url = format!("http://{}:{}", config.listen_address, config.admin_port);
                let _ = app.opener().open_url(&url, None::<&str>);
            }
        });
}

/// 后端 tokio 任务句柄，退出时 abort
struct BackendHandles {
    backend_handle: tokio::task::JoinHandle<()>,
}
