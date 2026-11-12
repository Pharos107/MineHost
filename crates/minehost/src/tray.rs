//! Icon khay hệ thống trên Windows (spec P1, mục 5.9).
//!
//! Vòng lặp message của khay chạy trên luồng chính; tokio và controller chạy trên luồng khác.
//! Hai bên nói chuyện qua `Shared` và message `post_wake`/`post_quit` của `win32`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use mh_app::App;
use tokio::sync::broadcast::error::RecvError;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::serve::ReadyHook;
use crate::win32;

const ICON_SIZE: u32 = 32;
/// Bấm "Đặt lại mật khẩu" lần hai trong chừng này mới xóa thật (spec P2, mục 3.3).
const RESET_CONFIRM: Duration = Duration::from_secs(10);
const RESET_TEXT: &str = "Đặt lại mật khẩu";
const RESET_CONFIRM_TEXT: &str = "Bấm lần nữa để xóa mật khẩu";

#[derive(Debug, Default)]
struct Shared {
    /// Có khi controller đã mở cổng: app để gọi Thoát, url để mở trình duyệt.
    ready: Mutex<Option<(Arc<App>, String)>>,
    running: AtomicUsize,
    /// Có khi vừa bấm "Đặt lại mật khẩu" lần đầu: hạn chót của lần bấm thứ hai.
    reset_armed: Mutex<Option<Instant>>,
}

impl Shared {
    fn ready(&self) -> Option<(Arc<App>, String)> {
        self.ready
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn reset_armed(&self) -> std::sync::MutexGuard<'_, Option<Instant>> {
        self.reset_armed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// `true` khi còn trong hạn bấm lần hai.
    fn reset_pending(&self) -> bool {
        self.reset_armed()
            .is_some_and(|deadline| Instant::now() < deadline)
    }
}

/// Chạy `controller` trên luồng riêng và giữ icon khay trên luồng này tới khi controller tắt.
/// Không tạo được icon (ví dụ máy không có màn hình) thì chạy controller như bình thường.
pub fn run(
    controller: impl FnOnce(ReadyHook) -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<()> {
    let open_item = MenuItem::new("Mở MineHost", true, None);
    let reset_item = MenuItem::new(RESET_TEXT, true, None);
    let quit_item = MenuItem::new("Thoát", true, None);
    let menu = Menu::new();
    let tray = icon().and_then(|icon| {
        menu.append_items(&[
            &open_item,
            &reset_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ])?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("MineHost")
            .with_icon(icon)
            .build()?;
        Ok(tray)
    });
    let tray = match tray {
        Ok(tray) => tray,
        Err(err) => {
            tracing::warn!(%err, "không tạo được icon khay, chạy không có khay");
            return controller(Box::new(|_, _| {}));
        }
    };

    let main_thread = win32::current_thread_id();
    let shared = Arc::new(Shared::default());
    on_menu(
        Arc::clone(&shared),
        MenuIds {
            open: open_item.id().clone(),
            reset: reset_item.id().clone(),
            quit: quit_item.id().clone(),
        },
        main_thread,
    );

    let hook_shared = Arc::clone(&shared);
    let on_ready: ReadyHook = Box::new(move |app, url| {
        *hook_shared
            .ready
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((Arc::clone(app), url.to_owned()));
        tokio::spawn(watch_running(
            Arc::clone(app),
            Arc::clone(&hook_shared),
            main_thread,
        ));
    });
    let worker = std::thread::Builder::new()
        .name("controller".into())
        .spawn(move || {
            let result = controller(on_ready);
            win32::post_quit(main_thread);
            result
        })?;

    win32::run_message_loop(|| {
        refresh(&tray, &quit_item, &shared);
        reset_item.set_text(if shared.reset_pending() {
            RESET_CONFIRM_TEXT
        } else {
            RESET_TEXT
        });
    });
    drop(tray);
    worker
        .join()
        .map_err(|_| anyhow::anyhow!("luồng controller dừng bất thường"))?
}

struct MenuIds {
    open: tray_icon::menu::MenuId,
    reset: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

/// Bấm menu: Windows gọi hàm này trên luồng khay, trong lúc xử lý message.
fn on_menu(shared: Arc<Shared>, ids: MenuIds, tray_thread: u32) {
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let Some((app, url)) = shared.ready() else {
            return;
        };
        if event.id == ids.open {
            if let Err(err) = open::that_detached(&url) {
                tracing::warn!(%err, "không mở được trình duyệt");
            }
        } else if event.id == ids.reset {
            on_reset(&app, &shared, tray_thread);
        } else if event.id == ids.quit {
            tracing::info!("người dùng bấm Thoát ở khay");
            app.request_shutdown();
        }
    }));
}

/// Lần đầu chỉ đổi chữ trên menu; lần hai trong 10 giây mới xóa mật khẩu và tắt LAN.
/// Khay không có hộp thoại xác nhận (ADR 0002), nên dùng hai lần bấm thay cho hộp thoại.
fn on_reset(app: &App, shared: &Shared, tray_thread: u32) {
    if shared.reset_pending() {
        *shared.reset_armed() = None;
        match app.auth().reset() {
            Ok(()) => tracing::info!("người dùng đặt lại mật khẩu ở khay"),
            Err(err) => tracing::error!(%err, "không xóa được mật khẩu"),
        }
    } else {
        *shared.reset_armed() = Some(Instant::now() + RESET_CONFIRM);
        // Hết hạn thì đánh thức luồng khay để trả chữ về như cũ.
        std::thread::spawn(move || {
            std::thread::sleep(RESET_CONFIRM);
            win32::post_wake(tray_thread);
        });
    }
    win32::post_wake(tray_thread);
}

/// Đếm server đang chạy mỗi khi có server đổi trạng thái, rồi báo luồng khay vẽ lại.
async fn watch_running(app: Arc<App>, shared: Arc<Shared>, tray_thread: u32) {
    let mut events = app.subscribe_events();
    loop {
        let running = app
            .list()
            .map(|views| views.iter().filter(|v| v.state.has_process()).count())
            .unwrap_or(0);
        if shared.running.swap(running, Ordering::Relaxed) != running {
            win32::post_wake(tray_thread);
        }
        match events.recv().await {
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}

fn refresh(tray: &TrayIcon, quit: &MenuItem, shared: &Shared) {
    let running = shared.running.load(Ordering::Relaxed);
    let (tooltip, quit_text) = if running == 0 {
        ("MineHost".to_owned(), "Thoát".to_owned())
    } else {
        (
            format!("MineHost — {running} server đang chạy"),
            // Không có hộp thoại hỏi lại ở khay, nên ghi rõ ngay trên menu việc sẽ xảy ra.
            format!("Thoát (tắt an toàn {running} server)"),
        )
    };
    if let Err(err) = tray.set_tooltip(Some(tooltip)) {
        tracing::warn!(%err, "không đổi được tooltip của khay");
    }
    quit.set_text(quit_text);
}

/// Icon vẽ bằng code (khối vuông màu teal có viền) để không phải kèm file ảnh.
fn icon() -> anyhow::Result<Icon> {
    let mut rgba = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let edge = x < 3 || y < 3 || x >= ICON_SIZE - 3 || y >= ICON_SIZE - 3;
            let top = y < ICON_SIZE / 3;
            let color: [u8; 4] = match (edge, top) {
                (true, _) => [4, 47, 43, 255],
                (false, true) => [94, 234, 212, 255],
                (false, false) => [15, 118, 110, 255],
            };
            rgba.extend_from_slice(&color);
        }
    }
    Ok(Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE)?)
}
