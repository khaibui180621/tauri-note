// ============================================================
// src-tauri/src/lib.rs
// ScreenNote Overlay — Rust Backend Logic
//
// Chứa toàn bộ logic backend:
//   1. Tauri Commands (set_click_through)
//   2. Global Shortcut (Cmd+Shift+D trên macOS, Ctrl+Shift+D trên Windows/Linux)
//   3. System Tray
//   4. Window setup (hide from dock, fullscreen overlay support on macOS)
// ============================================================

// Manager trait cần thiết để gọi get_webview_window trên App và AppHandle
#[allow(unused_imports)]
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WebviewWindow,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

// ============================================================
// TAURI COMMANDS
// ============================================================

/// Bật/tắt chế độ click-through cho cửa sổ chỉ định.
///
/// Khi `enabled = true`:
///   - Rust gọi set_ignore_cursor_events(true) → cửa sổ không nhận chuột ở cấp OS.
///   - Nhưng Toolbar trên Frontend sẽ được CSS pointer-events: auto bảo vệ.
///   - Giải pháp Hybrid: thực ra ở đây chúng ta vẫn set ignore = false từ Rust,
///     vì toolbar cần nhận chuột. Toàn bộ click-through logic được xử lý bởi CSS
///     pointer-events: none trên canvas ở Frontend (không tốn OS call).
///   - Khi người dùng cần click HOÀN TOÀN xuyên qua (kể cả toolbar bị ẩn),
///     thì mới gọi set_ignore_cursor_events(true).
///
/// NOTE: Tauri v2 — `set_ignore_cursor_events` thuộc core window permission.
#[tauri::command]
async fn set_click_through(
    window: WebviewWindow,
    enabled: bool,
) -> Result<(), String> {
    window
        .set_ignore_cursor_events(enabled)
        .map_err(|e| format!("Lỗi khi thay đổi chế độ click-through: {}", e))
}

/// Lấy trạng thái hiện tại của drawing mode (được lưu trong app state).
#[tauri::command]
fn get_draw_mode(state: tauri::State<AppState>) -> bool {
    *state.is_drawing_mode.lock().unwrap()
}

/// Cập nhật trạng thái drawing mode trong app state.
#[tauri::command]
fn set_draw_mode(state: tauri::State<AppState>, enabled: bool) {
    *state.is_drawing_mode.lock().unwrap() = enabled;
}

// ============================================================
// APP STATE
// ============================================================

/// Shared state giữa các commands và event handlers.
/// Dùng Mutex để thread-safe (Tauri commands có thể chạy async).
struct AppState {
    is_drawing_mode: std::sync::Mutex<bool>,
}

// ============================================================
// SETUP FUNCTION
// ============================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // --- Plugins ---
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // --- Managed State ---
        .manage(AppState {
            is_drawing_mode: std::sync::Mutex::new(true),
        })
        // --- Setup Hook ---
        .setup(|app| {
            setup_app(app)?;
            Ok(())
        })
        // --- Commands ---
        .invoke_handler(tauri::generate_handler![
            set_click_through,
            get_draw_mode,
            set_draw_mode,
        ])
        .run(tauri::generate_context!())
        .expect("Lỗi khởi động ScreenNote Overlay");
}

// ============================================================
// SETUP: Window, Tray, Global Shortcut
// ============================================================

fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    // --- 1. Thiết lập cửa sổ ---
    setup_window(app)?;

    // --- 2. Thiết lập System Tray ---
    setup_tray(app)?;

    // --- 3. Đăng ký Global Shortcut ---
    // Dùng Ctrl+Shift+D (Windows/Linux) hoặc Cmd+Shift+D (macOS)
    setup_global_shortcut(app_handle)?;

    Ok(())
}

/// Thiết lập cửa sổ chính:
/// - Set background color transparent tường minh (macOS WKWebView cần điều này)
/// - Ẩn khỏi Dock trên macOS
/// - Cho phép overlay hiển thị trên fullscreen apps (macOS NSWindowCollectionBehavior)
fn setup_window(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let window = app.get_webview_window("main")
        .ok_or("Không tìm thấy cửa sổ 'main'")?;

    // Đảm bảo cửa sổ luôn nổi trên cùng
    window.set_always_on_top(true)?;

    // Background transparency được xử lý bởi:
    // 1. tauri.conf.json: "backgroundColor": "#00000000"
    // 2. CSS: background: transparent trên html/body
    // Không cần gọi set_background_color qua API ở đây.

    // Trên macOS: ẩn khỏi Dock và cho phép overlay trên fullscreen apps
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;

        // Ẩn ứng dụng khỏi Dock bằng ActivationPolicy
        app.set_activation_policy(tauri::ActivationPolicy::Accessory);

        // Lấy NSWindow handle (con trỏ *mut Object của Objective-C)
        // window.ns_window() trả về *mut c_void trên macOS
        let ns_window_ptr = window.ns_window()
            .map_err(|e| format!("Không lấy được ns_window: {}", e))?;

        // NSWindowCollectionBehavior flags (raw numeric):
        //   NSWindowCollectionBehaviorCanJoinAllSpaces    = 1 << 0  (0x001)
        //   NSWindowCollectionBehaviorStationary          = 1 << 4  (0x010)
        //   NSWindowCollectionBehaviorIgnoresCycle        = 1 << 6  (0x040)
        //   NSWindowCollectionBehaviorFullScreenAuxiliary = 1 << 8  (0x100)
        //
        // FullScreenAuxiliary cho phép cửa sổ hiển thị BÊN TRÊN các app đang ở fullscreen.
        // CanJoinAllSpaces + Stationary giữ overlay trên mọi Space/Desktop.
        let behavior: u64 = (1 << 0)   // CanJoinAllSpaces
                          | (1 << 4)   // Stationary
                          | (1 << 6)   // IgnoresCycle
                          | (1 << 8);  // FullScreenAuxiliary

        unsafe {
            let ns_window = &mut *(ns_window_ptr as *mut AnyObject);
            let _: () = msg_send![ns_window, setCollectionBehavior: behavior];
        }
    }

    Ok(())
}

/// Tạo System Tray với menu đơn giản.
fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle();

    // Tạo menu items
    let toggle_item = MenuItem::with_id(app_handle, "toggle", "Hiện/Ẩn Toolbar", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app_handle, "quit", "Thoát ScreenNote", true, None::<&str>)?;

    let menu = Menu::with_items(app_handle, &[&toggle_item, &quit_item])?;

    // Tạo tray icon
    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip("ScreenNote Overlay\nCmd+Shift+D để bật/tắt vẽ")
        // Xử lý click trái vào tray icon → toggle toolbar visibility
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    // Emit event để frontend toggle toolbar
                    let _ = window.emit("tray-toggle-toolbar", ());
                }
            }
        })
        // Xử lý click vào menu items
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.emit("tray-toggle-toolbar", ());
                }
            }
            "quit" => {
                // Thoát ứng dụng an toàn
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Đăng ký phím tắt toàn cục:
///   - macOS:        Cmd+Shift+D  (dùng SUPER modifier)
///   - Windows/Linux: Ctrl+Shift+D (dùng CONTROL modifier)
///
/// Xử lý lỗi: nếu phím tắt đã bị ứng dụng khác chiếm dụng, ta:
/// 1. Log cảnh báo (không crash)
/// 2. Emit event thông báo lỗi tới frontend để hiển thị cho user
fn setup_global_shortcut(app_handle: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // macOS dùng Cmd (SUPER), Windows/Linux dùng Ctrl (CONTROL)
    #[cfg(target_os = "macos")]
    let modifier = Modifiers::SUPER | Modifiers::SHIFT;

    #[cfg(not(target_os = "macos"))]
    let modifier = Modifiers::CONTROL | Modifiers::SHIFT;

    let shortcut = Shortcut::new(Some(modifier), Code::KeyD);

    // Label dùng trong log/toast
    #[cfg(target_os = "macos")]
    let shortcut_label = "Cmd+Shift+D";
    #[cfg(not(target_os = "macos"))]
    let shortcut_label = "Ctrl+Shift+D";

    // Đăng ký với global shortcut plugin
    let result = app_handle.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, event| {
        // Chỉ xử lý khi nhấn xuống (tránh trigger 2 lần)
        if event.state() == ShortcutState::Pressed {
            if let Some(window) = app.get_webview_window("main") {
                // Gửi event "toggle-draw-mode" tới frontend
                // Frontend sẽ tự xử lý việc đảo trạng thái
                let _ = window.emit("toggle-draw-mode", ());
            }
        }
    });

    match result {
        Ok(_) => {
            eprintln!("[ScreenNote] Đã đăng ký phím tắt {} thành công", shortcut_label);
        }
        Err(e) => {
            // Phím tắt bị chiếm dụng hoặc lỗi khác — KHÔNG crash
            // Thông báo lỗi tới frontend
            eprintln!("[ScreenNote] CẢNH BÁO: Không thể đăng ký phím tắt: {}", e);
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.emit(
                    "shortcut-register-failed",
                    format!("Phím tắt {} đã bị phần mềm khác chiếm dụng: {}", shortcut_label, e),
                );
            }
        }
    }

    Ok(())
}
