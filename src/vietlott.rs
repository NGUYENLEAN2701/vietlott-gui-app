//! Vietlott 6/45 - All-in-one GTK4 App
//! cargo run --bin vietlott
//!Nguyễn Lê An 0123456789
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, FileDialog, Frame,
    IconTheme, Label, Orientation, PolicyType, ScrolledWindow, Separator, TextView, Window,
};
use glib::clone;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

// ─── CẤU HÌNH ────────────────────────────────────────────────────────────────

const MAX_WORKERS: usize = 10;
const BASE_URL: &str = "https://vietlott.vn/vi/trung-thuong/ket-qua-trung-thuong/645";
const WORKER_DELAY: u64 = 300;
const DEFAULT_START: u32 = 1;
const DEFAULT_END: u32 = 1529;
const APP_ICON_NAME: &str = "com.vietlott.datatool";

// ─── LOG & DATA DIR ──────────────────────────────────────────────────────────

static LOG_TX: OnceLock<mpsc::Sender<String>> = OnceLock::new();
static DATA_DIR: OnceLock<Mutex<PathBuf>> = OnceLock::new();

fn app_log(msg: impl Display) {
    let s = msg.to_string();
    if let Some(tx) = LOG_TX.get() {
        let _ = tx.send(s);
    } else {
        println!("{s}");
    }
}

fn app_log_err(msg: impl Display) {
    let s = msg.to_string();
    if let Some(tx) = LOG_TX.get() {
        let _ = tx.send(s);
    } else {
        eprintln!("{s}");
    }
}

fn default_app_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join("Desktop").join("vietlott-app")
}

fn config_file() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".config").join("com.vietlott.datatool");
    std::fs::create_dir_all(&dir).ok();
    dir.join("data_dir.txt")
}

fn load_saved_dir() -> Option<PathBuf> {
    let path = std::fs::read_to_string(config_file()).ok()?;
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

fn save_data_dir(path: &Path) {
    if let Some(s) = path.to_str() {
        let _ = std::fs::write(config_file(), s);
    }
}

fn init_data_dir() -> &'static Mutex<PathBuf> {
    DATA_DIR.get_or_init(|| Mutex::new(load_saved_dir().unwrap_or_else(default_app_dir)))
}

fn app_dir() -> PathBuf {
    let dir = init_data_dir().lock().unwrap().clone();
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn set_app_dir(path: PathBuf) {
    std::fs::create_dir_all(&path).ok();
    *init_data_dir().lock().unwrap() = path.clone();
    save_data_dir(&path);
}

fn raw_dir() -> PathBuf {
    let d = app_dir().join("vietlott-raw");
    std::fs::create_dir_all(&d).ok();
    d
}
fn missing_file() -> PathBuf {
    app_dir().join("missing.txt")
}
fn output_file() -> PathBuf {
    app_dir().join("vietlott_6-45.txt")
}
fn raw_path(id: &str) -> PathBuf {
    raw_dir().join(format!("{}.txt", id))
}

// ─── ENTRY POINT ─────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("scraper") => {
            let start = args
                .get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_START);
            let end = args
                .get(3)
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_END);
            run_scraper_cli(start, end, false);
        }
        Some("missing") => {
            let start = args
                .get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_START);
            let end = args
                .get(3)
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_END);
            run_scraper_cli(start, end, true);
        }
        Some("checker") => {
            let start = args
                .get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_START);
            let end = args
                .get(3)
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_END);
            run_checker_cli(start, end);
        }
        Some("parser") => run_parser_cli(),
        _ => run_gui(),
    }
}

// ─── GUI ─────────────────────────────────────────────────────────────────────

fn bundled_icon_theme_dir() -> Option<PathBuf> {
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
    if dev.join("hicolor").is_dir() {
        return Some(dev);
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            for candidate in [
                parent.join("assets/icons"),
                parent.join("../share/icons"),
            ] {
                if candidate.join("hicolor").is_dir() {
                    return candidate.canonicalize().ok();
                }
            }
        }
    }

    None
}

fn setup_app_icon() {
    if let Some(dir) = bundled_icon_theme_dir() {
        if let Some(display) = gtk4::gdk::Display::default() {
            IconTheme::for_display(&display).add_search_path(dir);
        }
    }
    Window::set_default_icon_name(APP_ICON_NAME);
}

fn run_gui() {
    let app = Application::builder().application_id("com.vietlott.datatool").build();
    app.connect_activate(build_ui);
    app.run();
}

fn append_log(text_view: &TextView, msg: &str) {
    let buffer = text_view.buffer();
    let mut end = buffer.end_iter();
    buffer.insert(&mut end, &format!("{msg}\n"));
    text_view.scroll_to_mark(&buffer.create_mark(None, &buffer.end_iter(), true), 0.0, false, 0.0, 0.0);
}

fn set_buttons_enabled(buttons: &[Button], enabled: bool) {
    for btn in buttons {
        btn.set_sensitive(enabled);
    }
}

fn begin_task(buttons: &[Button], label: &str) {
    set_buttons_enabled(buttons, false);
    app_log(format!("═══ Bắt đầu: {label} ═══"));
}

fn run_in_background<F>(task: F, done_tx: mpsc::Sender<()>)
where
    F: FnOnce() + Send + 'static,
{
    std::thread::spawn(move || {
        task();
        let _ = done_tx.send(());
    });
}

fn build_ui(app: &Application) {
    setup_app_icon();

    let (log_tx, log_rx) = mpsc::channel::<String>();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let _ = LOG_TX.set(log_tx);
    init_data_dir();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Vietlott 6/45 - Data Tool")
        .icon_name(APP_ICON_NAME)
        .default_width(560)
        .default_height(640)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 10);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);

    // ── Title ──
    let title = Label::new(Some("🎱 Vietlott Mega 6/45 Data Tool"));
    title.add_css_class("title-2");
    title.set_halign(Align::Center);
    root.append(&title);

    // ── Data dir picker ──
    let dir_frame = Frame::new(Some(" Thư mục lưu dữ liệu "));
    let dir_box = GtkBox::new(Orientation::Horizontal, 8);
    dir_box.set_margin_top(10);
    dir_box.set_margin_bottom(10);
    dir_box.set_margin_start(10);
    dir_box.set_margin_end(10);

    let entry_dir = Entry::builder()
        .text(app_dir().display().to_string())
        .editable(false)
        .hexpand(true)
        .build();

    let btn_browse = Button::with_label("📂  Chọn thư mục");

    dir_box.append(&entry_dir);
    dir_box.append(&btn_browse);
    dir_frame.set_child(Some(&dir_box));
    root.append(&dir_frame);
    root.append(&Separator::new(Orientation::Horizontal));

    // ── ID range ──
    let range_frame = Frame::new(Some(" Dải kỳ quay "));
    let range_box = GtkBox::new(Orientation::Horizontal, 10);
    range_box.set_margin_top(10);
    range_box.set_margin_bottom(10);
    range_box.set_margin_start(10);
    range_box.set_margin_end(10);
    range_box.set_halign(Align::Center);

    range_box.append(&Label::new(Some("Start ID:")));
    let entry_start = Entry::builder().text("1").width_chars(8).build();
    range_box.append(&entry_start);

    range_box.append(&Label::new(Some("End ID:")));
    let entry_end = Entry::builder().text("1529").width_chars(8).build();
    range_box.append(&entry_end);

    range_frame.set_child(Some(&range_box));
    root.append(&range_frame);

    // ── Buttons ──
    let btn_frame = Frame::new(Some(" Chức năng "));
    let btn_box = GtkBox::new(Orientation::Vertical, 8);
    btn_box.set_margin_top(10);
    btn_box.set_margin_bottom(10);
    btn_box.set_margin_start(10);
    btn_box.set_margin_end(10);

    let btn_scraper = Button::with_label("🚀  Scraper  (cào theo dải ID)");
    let btn_missing = Button::with_label("🔁  Scraper  (cào bù missing.txt)");
    let btn_checker = Button::with_label("🔍  Checker  (kiểm tra file thiếu)");
    let btn_parser = Button::with_label("⚙️   Parser   (parse raw → database)");

    btn_scraper.add_css_class("suggested-action");
    btn_missing.add_css_class("suggested-action");
    btn_checker.add_css_class("destructive-action");
    btn_parser.add_css_class("suggested-action");

    btn_box.append(&btn_scraper);
    btn_box.append(&btn_missing);
    btn_box.append(&btn_checker);
    btn_box.append(&btn_parser);
    btn_frame.set_child(Some(&btn_box));
    root.append(&btn_frame);

    // ── Log panel ──
    let log_frame = Frame::new(Some(" Nhật ký "));
    let log_box = GtkBox::new(Orientation::Vertical, 0);
    log_box.set_margin_top(8);
    log_box.set_margin_bottom(8);
    log_box.set_margin_start(8);
    log_box.set_margin_end(8);

    let log_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Automatic)
        .min_content_height(200)
        .vexpand(true)
        .build();

    let log_view = TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk4::WrapMode::WordChar)
        .build();
    log_view.add_css_class("monospace");
    log_scroll.set_child(Some(&log_view));
    log_box.append(&log_scroll);
    log_frame.set_child(Some(&log_box));
    root.append(&log_frame);
    root.set_vexpand(true);

    window.set_child(Some(&root));

    let all_buttons = Arc::new([
        btn_scraper.clone(),
        btn_missing.clone(),
        btn_checker.clone(),
        btn_parser.clone(),
        btn_browse.clone(),
    ]);

    // ── Poll log & task-done messages ──
    let poll_buttons = Arc::clone(&all_buttons);
    glib::timeout_add_local(Duration::from_millis(50), move || {
        while let Ok(msg) = log_rx.try_recv() {
            append_log(&log_view, &msg);
        }
        while done_rx.try_recv().is_ok() {
            set_buttons_enabled(poll_buttons.as_ref(), true);
            append_log(&log_view, "─── Hoàn tất ───");
        }
        glib::ControlFlow::Continue
    });

    // ── Browse folder ──
    btn_browse.connect_clicked(clone!(#[strong] window, #[strong] entry_dir, move |_| {
        let dialog = {
            let builder = FileDialog::builder()
                .title("Chọn thư mục lưu dữ liệu")
                .modal(true);
            if let Ok(current) = std::fs::canonicalize(app_dir()) {
                builder
                    .initial_folder(&gio::File::for_path(current))
                    .build()
            } else {
                builder.build()
            }
        };

        let entry_dir = entry_dir.clone();
        dialog.select_folder(
            Some(&window),
            None::<&gio::Cancellable>,
            move |result| {
                match result {
                    Ok(file) => {
                        if let Some(path) = file.path() {
                            set_app_dir(path.clone());
                            let display = path.display().to_string();
                            entry_dir.set_text(&display);
                            app_log(format!("📁 Đã chọn thư mục: {display}"));
                        }
                    }
                    Err(e) => {
                        if e.kind::<gio::IOErrorEnum>() != Some(gio::IOErrorEnum::Cancelled) {
                            app_log_err(format!("❌ Không chọn được thư mục: {e}"));
                        }
                    }
                }
            },
        );
    }));

    // ── Connect buttons ──
    {
        let btns = Arc::clone(&all_buttons);
        let tx = done_tx.clone();
        btn_scraper.connect_clicked(clone!(#[weak] entry_start, #[weak] entry_end, move |_| {
            let s = entry_start.text().to_string().parse().unwrap_or(DEFAULT_START);
            let e = entry_end.text().to_string().parse().unwrap_or(DEFAULT_END);
            begin_task(btns.as_ref(), "Scraper");
            run_in_background(move || run_scraper_cli(s, e, false), tx.clone());
        }));
    }

    {
        let btns = Arc::clone(&all_buttons);
        let tx = done_tx.clone();
        btn_missing.connect_clicked(clone!(#[weak] entry_start, #[weak] entry_end, move |_| {
            let s = entry_start.text().to_string().parse().unwrap_or(DEFAULT_START);
            let e = entry_end.text().to_string().parse().unwrap_or(DEFAULT_END);
            begin_task(btns.as_ref(), "Scraper (missing)");
            run_in_background(move || run_scraper_cli(s, e, true), tx.clone());
        }));
    }

    {
        let btns = Arc::clone(&all_buttons);
        let tx = done_tx.clone();
        btn_checker.connect_clicked(clone!(#[weak] entry_start, #[weak] entry_end, move |_| {
            let s = entry_start.text().to_string().parse().unwrap_or(DEFAULT_START);
            let e = entry_end.text().to_string().parse().unwrap_or(DEFAULT_END);
            begin_task(btns.as_ref(), "Checker");
            run_in_background(move || run_checker_cli(s, e), tx.clone());
        }));
    }

    {
        let btns = Arc::clone(&all_buttons);
        let tx = done_tx.clone();
        btn_parser.connect_clicked(move |_| {
            begin_task(btns.as_ref(), "Parser");
            run_in_background(run_parser_cli, tx.clone());
        });
    }

    app_log(format!("📁 Thư mục dữ liệu: {}", app_dir().display()));
    app_log("Sẵn sàng. Chọn chức năng để bắt đầu.");

    window.present();
}

// ─── SCRAPER CLI ─────────────────────────────────────────────────────────────

fn run_scraper_cli(start: u32, end: u32, use_missing: bool) {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        use chromiumoxide::browser::{Browser, BrowserConfig};
        use futures::StreamExt;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        app_log(format!("📁 Data dir: {}", app_dir().display()));

        let mut ids: Vec<String> = if use_missing && missing_file().exists() {
            let content = tokio::fs::read_to_string(missing_file()).await.unwrap_or_default();
            let re = regex::Regex::new(r"^\d+").unwrap();
            content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| re.find(l.trim()))
                .map(|m| format!("{:0>5}", m.as_str()))
                .collect()
        } else {
            (start..=end).map(|i| format!("{:0>5}", i)).collect()
        };

        let before = ids.len();
        ids.retain(|id| !raw_path(id).exists());
        let skipped = before - ids.len();
        if skipped > 0 {
            app_log(format!("⏭️  Bỏ qua {skipped} file đã có."));
        }

        if ids.is_empty() {
            app_log("✅ Tất cả đã có, không cần cào thêm.");
            return;
        }

        let total = ids.len();
        app_log(format!("📡 Khởi chạy browser ({MAX_WORKERS} workers) ..."));

        let (browser, mut handler) = match Browser::launch(BrowserConfig::builder().build().unwrap()).await
        {
            Ok(b) => b,
            Err(e) => {
                app_log_err(format!("❌ Không khởi động được browser: {e}"));
                return;
            }
        };

        let handler_task = tokio::spawn(async move {
            while handler.next().await.is_some() {}
        });

        let browser = Arc::new(browser);
        let queue = Arc::new(Mutex::new(ids));
        let counter = Arc::new(Mutex::new(0usize));
        let mut tasks = vec![];

        for i in 1..=MAX_WORKERS {
            let q = Arc::clone(&queue);
            let b = Arc::clone(&browser);
            let c = Arc::clone(&counter);

            tasks.push(tokio::spawn(async move {
                app_log(format!("👷 [Worker {i}] Sẵn sàng."));
                loop {
                    let id_str = {
                        let mut q = q.lock().await;
                        if q.is_empty() {
                            break;
                        }
                        q.remove(0)
                    };
                    let progress = {
                        let mut c = c.lock().await;
                        *c += 1;
                        *c
                    };
                    let url = format!("{BASE_URL}?id={id_str}&nocatche=1");
                    app_log(format!("🚀 [Worker {i}] [{progress}/{total}] #{id_str}"));

                    let mut page_opt = None;
                    match b.new_page(&url).await {
                        Ok(page) => {
                            let _ = page.wait_for_navigation().await;
                            if let Ok(val) = page
                                .evaluate(
                                    r#"() => {
                                ['script','style','noscript','iframe','nav','header','footer','.footer']
                                    .forEach(sel => document.querySelectorAll(sel).forEach(el => el.remove()));
                                return (document.body.innerText||'')
                                    .split('\n').map(l=>l.trim()).filter(l=>l.length>0).join('\n');
                            }"#,
                                )
                                .await
                            {
                                if let Ok(text) = val.into_value::<String>() {
                                    let fpath = raw_path(&id_str);
                                    match tokio::fs::write(&fpath, &text).await {
                                        Ok(_) => app_log(format!("✅ [Worker {i}] {id_str}.txt")),
                                        Err(e) => app_log_err(format!("❌ Lỗi ghi {id_str}: {e}")),
                                    }
                                }
                            }
                            page_opt = Some(page);
                        }
                        Err(e) => app_log_err(format!("❌ [Worker {i}] {id_str}: {e}")),
                    }
                    if let Some(p) = page_opt {
                        let _ = p.close().await;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(WORKER_DELAY)).await;
                }
                app_log(format!("🏁 [Worker {i}] Xin nghỉ!"));
            }));
        }

        for t in tasks {
            let _ = t.await;
        }
        handler_task.abort();
        let done = *counter.lock().await;
        app_log(format!("\n🎉 HOÀN TẤT! {done}/{total} ID."));
    });
}

// ─── CHECKER CLI ─────────────────────────────────────────────────────────────

fn run_checker_cli(start: u32, end: u32) {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        app_log(format!("📁 Data dir: {}", app_dir().display()));
        app_log(format!("📂 Raw dir : {}", raw_dir().display()));
        app_log(format!("🔍 Kiểm tra {:0>5} → {:0>5} ...", start, end));

        let mut missing: Vec<String> = vec![];

        for i in start..=end {
            let id_str = format!("{:0>5}", i);
            let fpath = raw_path(&id_str);
            if !fpath.exists() {
                missing.push(id_str);
            } else {
                match tokio::fs::read_to_string(&fpath).await {
                    Ok(content) if content.trim().is_empty() => {
                        app_log(format!("⚠️  File trống: {id_str}.txt"));
                        let _ = tokio::fs::remove_file(&fpath).await;
                        missing.push(id_str);
                    }
                    Ok(content)
                        if content.contains("bad gateway")
                            || content.contains("Cloudflare")
                            || content.contains("Host Error")
                            || !content.contains("Kỳ quay thưởng") =>
                    {
                        app_log(format!("⚠️  File lỗi nội dung: {id_str}.txt → xóa"));
                        let _ = tokio::fs::remove_file(&fpath).await;
                        missing.push(id_str);
                    }
                    _ => {}
                }
            }
        }

        if !missing.is_empty() {
            app_log(format!("❌ Thiếu {} file!", missing.len()));
            tokio::fs::write(missing_file(), missing.join("\n") + "\n")
                .await
                .unwrap();
            app_log(format!("📝 Đã ghi → {}", missing_file().display()));
            for id in &missing {
                app_log(format!("   #{id}"));
            }
        } else {
            app_log("✅ Đầy đủ! Không thiếu file nào.");
            if missing_file().exists() {
                let _ = tokio::fs::remove_file(missing_file()).await;
                app_log("🗑️  Đã xóa missing.txt cũ.");
            }
        }
    });
}

// ─── PARSER CLI ──────────────────────────────────────────────────────────────

fn run_parser_cli() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        app_log(format!("📁 Data dir: {}", app_dir().display()));
        app_log(format!("📂 Đang parse {} ...", raw_dir().display()));

        let mut entries: Vec<String> = vec![];
        match tokio::fs::read_dir(raw_dir()).await {
            Ok(mut dir) => {
                while let Some(e) = dir.next_entry().await.unwrap() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.ends_with(".txt") {
                        entries.push(name);
                    }
                }
            }
            Err(e) => {
                app_log_err(format!("❌ {e}"));
                return;
            }
        }
        entries.sort();

        let mut results: Vec<String> = vec![];
        let mut failed: Vec<String> = vec![];

        for name in &entries {
            let fpath = raw_dir().join(name);
            match tokio::fs::read_to_string(&fpath).await {
                Ok(content) => match parse_content(&content) {
                    Some(line) => results.push(line),
                    None => failed.push(name.clone()),
                },
                Err(e) => {
                    app_log_err(format!("❌ {name}: {e}"));
                    failed.push(name.clone());
                }
            }
        }

        results.sort();
        tokio::fs::write(output_file(), results.join("\n") + "\n")
            .await
            .unwrap();
        app_log(format!(
            "\n🎉 HOÀN TẤT! {} dòng → '{}'",
            results.len(),
            output_file().display()
        ));
        if !failed.is_empty() {
            app_log(format!(
                "⚠️  Thất bại ({}): {}",
                failed.len(),
                failed[..5.min(failed.len())].join(", ")
            ));
        }
    });
}

fn parse_content(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().map(|l| l.trim()).collect();
    let re_h = regex::Regex::new(r"#(\d+)\s+ngày\s+(\d{2}/\d{2}/\d{4})").unwrap();
    let re_n = regex::Regex::new(r"^\d{12}$").unwrap();

    let (ky, ng) = lines
        .iter()
        .find_map(|l| re_h.captures(l).map(|c| (c[1].to_string(), c[2].to_string())))?;

    let so = lines
        .iter()
        .find(|l| re_n.is_match(l))
        .map(|l| l.to_string())
        .unwrap_or_default();

    let mut gt = "0".to_string();
    let mut j = "0".to_string();
    let mut g1 = "0".to_string();
    let mut g2 = "0".to_string();
    let mut g3 = "0".to_string();

    for line in &lines {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() < 2 {
            continue;
        }
        if line.starts_with("Jackpot") {
            j = p[p.len() - 2].to_string();
            gt = p[p.len() - 1].to_string();
        } else if line.starts_with("Giải Nhất") {
            g1 = p[p.len() - 2].to_string();
        } else if line.starts_with("Giải Nhì") {
            g2 = p[p.len() - 2].to_string();
        } else if line.starts_with("Giải Ba") {
            g3 = p[p.len() - 2].to_string();
            break;
        }
    }

    let c = |s: &str| s.replace('.', "");
    Some(format!(
        "#{}-{}-{}-{}-{}-{}-{}-{}",
        ky, ng, so, gt, c(&j), c(&g1), c(&g2), c(&g3)
    ))
}
