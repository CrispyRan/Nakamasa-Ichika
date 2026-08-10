
//! # TUI 仪表盘模块
//!
//! 提供三框终端仪表盘：系统信息 + CPU 走势图 | 资源监控 | 启动日志 | 命令行。
//! 基于 ratatui + crossterm，樱花粉色主题，兼容 Termux/Android 环境。

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::{
    fs,
    io,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

// ============================================================================
// 日志缓冲区（同前）
// ============================================================================

const MAX_LOG_LINES: usize = 2000;

pub struct LogBuffer {
    lines: Mutex<Vec<String>>,
}

impl LogBuffer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { lines: Mutex::new(Vec::with_capacity(MAX_LOG_LINES)) })
    }

    fn push_line(&self, line: String) {
        let mut guard = self.lines.lock().unwrap();
        if guard.len() >= MAX_LOG_LINES { guard.remove(0); }
        guard.push(line);
    }

    pub fn get_lines(&self, max_count: usize) -> Vec<String> {
        let guard = self.lines.lock().unwrap();
        let start = if guard.len() > max_count { guard.len() - max_count } else { 0 };
        guard[start..].to_vec()
    }

    pub fn len(&self) -> usize {
        self.lines.lock().unwrap().len()
    }

    pub fn push_bytes(&self, buf: &[u8]) {
        let s = String::from_utf8_lossy(buf);
        for line in s.split('\n') {
            let t = line.trim_end_matches('\r');
            if !t.is_empty() { self.push_line(t.to_string()); }
        }
    }
}

#[allow(dead_code)] // 保留：TUI日志写入器
pub struct LogWriter {
    pub buffer: Arc<LogBuffer>,
}

impl io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.push_bytes(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

impl Clone for LogWriter {
    fn clone(&self) -> Self { Self { buffer: self.buffer.clone() } }
}

// ============================================================================
// 系统信息采集（同前）
// ============================================================================

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct SystemInfo {
    pub os: String, pub kernel: String, pub hostname: String,
    pub cpu_model: String, pub cpu_cores: usize,
    pub architecture: String, pub rust_version: String, pub app_version: String,
}

#[derive(Clone, Debug, Default)]
pub struct ResourceUsage {
    pub cpu_percent: f64, pub memory_total_mb: u64, pub memory_used_mb: u64,
    pub memory_percent: f64, pub disk_total_mb: u64, pub disk_used_mb: u64,
    pub disk_percent: f64, pub pid: u32, pub uptime_secs: u64,
}

pub fn collect_system_info() -> SystemInfo {
    let os = "Android".to_string();
    let kernel = read_first_line("/proc/version")
        .map(|s| s.split_whitespace().nth(2).unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let hostname = hostname::get().map(|h| h.to_string_lossy().to_string()).unwrap_or_else(|_| "localhost".to_string());
    let cpu_model = read_file_contains("/proc/cpuinfo", "Hardware")
        .or_else(|| read_file_contains("/proc/cpuinfo", "model name"))
        .or_else(|| read_file_contains("/proc/cpuinfo", "Processor"))
        .unwrap_or_else(|| "unknown".to_string());
    let cpu_cores = count_lines_containing("/proc/cpuinfo", "processor");
    let architecture = std::env::consts::ARCH.to_string();
    let rust_version = format!("{} (compiled)", option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("1.85"));
    let app_version = env!("CARGO_PKG_VERSION").to_string();
    SystemInfo { os, kernel, hostname, cpu_model, cpu_cores, architecture, rust_version, app_version }
}

pub fn collect_resource_usage() -> ResourceUsage {
    let mem_total = read_meminfo_field("MemTotal").unwrap_or(1);
    let mem_avail = read_meminfo_field("MemAvailable").unwrap_or(0);
    let mem_used = mem_total.saturating_sub(mem_avail);
    let memory_total_mb = mem_total / 1024;
    let memory_used_mb = mem_used / 1024;
    let memory_percent = if mem_total > 0 { (mem_used as f64 / mem_total as f64) * 100.0 } else { 0.0 };
    let (d_total, d_used) = get_disk_usage(Path::new("/data"));
    let disk_percent = if d_total > 0 { (d_used as f64 / d_total as f64) * 100.0 } else { 0.0 };
    let pid = std::process::id();
    let uptime_secs = read_first_line("/proc/uptime")
        .ok().and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
        .map(|s| s as u64).unwrap_or(0);
    ResourceUsage {
        cpu_percent: get_cpu_percent(),
        memory_total_mb, memory_used_mb, memory_percent,
        disk_total_mb: d_total, disk_used_mb: d_used, disk_percent,
        pid, uptime_secs,
    }
}

#[allow(dead_code)] // 保留：TUI数据目录获取
fn get_data_dir() -> &'static Path { Path::new("/data") }
fn read_first_line(path: &str) -> io::Result<String> {
    let c = fs::read_to_string(path)?;
    Ok(c.lines().next().unwrap_or("").trim().to_string())
}
fn read_file_contains(path: &str, prefix: &str) -> Option<String> {
    let c = fs::read_to_string(path).ok()?;
    for line in c.lines() {
        let t = line.trim();
        if let Some(val) = t.strip_prefix(prefix) {
            return Some(val.trim_start_matches(':').trim().to_string());
        }
    }
    None
}
fn count_lines_containing(path: &str, pref: &str) -> usize {
    fs::read_to_string(path).ok().map(|c| c.lines().filter(|l| l.trim().starts_with(pref)).count()).unwrap_or(1)
}
fn read_meminfo_field(field: &str) -> Option<u64> {
    let c = fs::read_to_string("/proc/meminfo").ok()?;
    for line in c.lines() {
        if let Some(val) = line.strip_prefix(field) {
            return val.split_whitespace().next()?.parse::<u64>().ok();
        }
    }
    None
}
fn get_disk_usage(path: &Path) -> (u64, u64) {
    #[cfg(unix)]
    {
        use nix::sys::statvfs::statvfs;
        match statvfs(path) {
            Ok(s) => {
                let bs = s.block_size();
                let total = s.blocks() * bs / 1024 / 1024;
                let free = s.blocks_free() * bs / 1024 / 1024;
                (total, total.saturating_sub(free))
            }
            Err(_) => (0, 0),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        (0, 0)
    }
}
fn get_cpu_percent() -> f64 {
    // Android 上 /proc/stat 无权限，用 top 命令读取
    // 全局单后台线程持续更新 CPU 值，主线程仅原子读取
    use std::sync::atomic::AtomicU64;
    use std::sync::OnceLock;

    static CPU: AtomicU64 = AtomicU64::new(0);
    static INIT: OnceLock<()> = OnceLock::new();

    INIT.get_or_init(|| {
        std::thread::spawn(|| loop {
            let output = match std::process::Command::new("top")
                .args(["-b", "-n", "2", "-d", "1"])
                .output()
            {
                Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
                Err(_) => {
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
            };
            // 从后往前找 CPU 行（第二个样本 = 1s delta）
            for line in output.lines().rev() {
                if line.contains("%cpu") && !line.contains("ARGS") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 5 {
                        let total_s = parts[0].trim_end_matches("%cpu");
                        let idle_s = parts[4].trim_end_matches("%idle");
                        if let (Ok(total), Ok(idle)) =
                            (total_s.parse::<f64>(), idle_s.parse::<f64>())
                            && total > 0.0 {
                                let pct = ((total - idle) / total) * 100.0;
                                CPU.store(pct.to_bits(), std::sync::atomic::Ordering::Relaxed);
                                break;
                            }
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(4));
        });
    });

    f64::from_bits(CPU.load(std::sync::atomic::Ordering::Relaxed))
}

// ============================================================================
// 樱花粉色主题 & TUI 状态
// ============================================================================

const PINK_PRIMARY: Color = Color::Rgb(255, 182, 193);
const PINK_HOT: Color = Color::Rgb(255, 105, 180);
const PINK_DEEP: Color = Color::Rgb(219, 112, 147);
const PINK_ACCENT: Color = Color::Rgb(255, 20, 147);
const PINK_SOFT: Color = Color::Rgb(255, 218, 225);
const GREEN_LEAF: Color = Color::Rgb(144, 238, 144);
const GOLD_WARM: Color = Color::Rgb(255, 215, 0);

const CPU_HISTORY_MAX: usize = 60;

#[derive(Clone, PartialEq)]
pub(crate) enum CommandMode { Normal, Input }

pub struct TuiApp {
    pub system_info: SystemInfo,
    pub resource: ResourceUsage,
    pub log_buffer: Arc<LogBuffer>,
    pub scroll_offset: usize,
    pub auto_scroll: bool,
    pub should_quit: bool,
    pub last_refresh: Instant,
    pub command_mode: CommandMode,
    pub command_buffer: String,
    pub command_cursor: usize,
    pub command_history: Vec<String>,
    pub messages: Vec<(String, Color)>,
    pub cpu_history: Vec<f64>,
    pub terminal_height: u16,
    pub terminal_width: u16,
    #[allow(dead_code)] // 保留：TUI仪表盘可展开CPU图表状态
    pub cpu_chart_expanded: bool,
}

impl TuiApp {
    pub fn new(log_buffer: Arc<LogBuffer>) -> Self {
        let si = collect_system_info();
        let r = collect_resource_usage();
        let mut cpu_h = Vec::with_capacity(CPU_HISTORY_MAX);
        cpu_h.push(r.cpu_percent);
        Self {
            system_info: si, resource: r, log_buffer,
            scroll_offset: 0, auto_scroll: true, should_quit: false,
            last_refresh: Instant::now(),
            command_mode: CommandMode::Normal,
            command_buffer: String::new(), command_cursor: 0,
            command_history: Vec::new(), messages: Vec::new(),
            cpu_history: cpu_h, terminal_height: 30, terminal_width: 80,
            cpu_chart_expanded: false,
        }
    }

    pub fn add_message(&mut self, msg: &str, color: Color) {
        self.messages.push((msg.to_string(), color));
        if self.messages.len() > 5 { self.messages.remove(0); }
    }

    pub fn refresh(&mut self) {
        self.resource = collect_resource_usage();
        self.cpu_history.push(self.resource.cpu_percent);
        if self.cpu_history.len() > CPU_HISTORY_MAX { self.cpu_history.remove(0); }
        self.last_refresh = Instant::now();
    }

    pub fn execute_command(&mut self, cmd: &str) {
        let t = cmd.trim();
        if t.is_empty() { return; }
        self.command_history.push(t.to_string());
        match t.to_lowercase().as_str() {
            "quit" | "exit" | "stop" | "q" => self.should_quit = true,
            "help" | "?" => self.add_message("命令: quit/exit/stop/q(退出), help(帮助), clear(清屏), echo(回显)", PINK_DEEP),
            "clear" | "cls" => self.command_buffer.clear(),
            "refresh" | "r" => { self.system_info = collect_system_info(); self.refresh(); self.add_message("已刷新", GREEN_LEAF); }
            o => {
                if let Some(ech) = o.strip_prefix("echo ") { self.add_message(ech, PINK_SOFT); }
                else { self.add_message(&format!("未知: {}", t), Color::Red); }
            }
        }
        self.command_buffer.clear(); self.command_cursor = 0; self.command_mode = CommandMode::Normal;
    }

    pub fn uptime_str(&self) -> String {
        let s = self.resource.uptime_secs;
        let h = s / 3600; let m = (s % 3600) / 60; let sec = s % 60;
        if h > 0 { format!("{}h {:02}m {:02}s", h, m, sec) }
        else if m > 0 { format!("{}m {:02}s", m, sec) }
        else { format!("{}s", sec) }
    }

    fn format_mb(mb: u64) -> String {
        if mb > 1024 { format!("{:.1} GB", mb as f64 / 1024.0) } else { format!("{} MB", mb) }
    }

    fn render_cpu_sparkline(&self, width: usize) -> String {
        if self.cpu_history.is_empty() || width == 0 { return String::new(); }
        let w = width.min(self.cpu_history.len());
        let max_val = self.cpu_history.iter().cloned().fold(f64::NEG_INFINITY, f64::max).max(1.0);
        let chars = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let data = &self.cpu_history[self.cpu_history.len().saturating_sub(w)..];
        data.iter().map(|v| { let idx = ((*v / max_val) * 8.0) as usize; chars[idx.min(8)] }).collect()
    }
}

// ============================================================================
// 渲染函数 — OpenCode 风格樱花粉终端
// ============================================================================

/// 渲染整个界面 — OpenCode 风格：Logo 横幅 + 状态栏 + 日志区 + 命令栏
pub fn render(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut TuiApp) -> io::Result<()> {
    terminal.draw(|f| {
        let area = f.area();
        app.terminal_height = area.height;
        app.terminal_width = area.width;

        let h = area.height;
        let bar_h = 3u16.min(h / 5).max(2);

        // Logo 横幅 + 分隔 + 状态栏 + 分隔 + 日志 + 命令栏
        let logo_h = 3u16.min(h / 8).max(2);
        let status_h = 3u16.min(h / 8).max(2);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(logo_h),           // Logo 横幅
                Constraint::Length(1),                // 分隔线
                Constraint::Length(status_h),         // 状态栏
                Constraint::Length(1),                // 分隔线
                Constraint::Min(1),                   // 日志区域
                Constraint::Length(bar_h),            // 命令栏
            ])
            .split(area);

        // 消息提示（浮动在日志区顶部）
        if !app.messages.is_empty() && chunks[4].height > 2 {
            let msg = app.messages.iter().map(|(m, _)| m.as_str()).collect::<Vec<&str>>().join(" | ");
            let mc = app.messages.last().map(|(_, c)| *c).unwrap_or(PINK_PRIMARY);
            let mp = Paragraph::new(Line::from(Span::styled(msg, Style::default().fg(mc))))
                .block(Block::default().borders(Borders::NONE));
            let ma = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(chunks[4])[0];
            f.render_widget(mp, ma);
        }

        render_logo_banner(f, chunks[0], app);
        render_separator(f, chunks[1]);
        render_status_bar(f, chunks[2], app);
        render_separator(f, chunks[3]);
        render_log_panel(f, chunks[4], app);
        render_command_bar(f, chunks[5], app);
    })?;
    Ok(())
}

/// OpenCode 风格 Logo 横幅 — 樱花粉 Nakamasa-Ichika 品牌区
fn render_logo_banner(f: &mut Frame, area: Rect, app: &TuiApp) {
    let si = &app.system_info;

    // Logo行：Nakamasa-Ichika 品牌 + 装饰
    let logo_line = Line::from(vec![
        Span::styled("  ❀ ", Style::default().fg(PINK_SOFT)),
        Span::styled("Nakamasa-Ichika", Style::default().fg(PINK_ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(" ❀  ", Style::default().fg(PINK_SOFT)),
        Span::styled(format!("v{}", si.app_version), Style::default().fg(GOLD_WARM)),
    ]);

    // 状态行：构建 + 架构 + 主机名（类似 OpenCode 的 "build | model | path"）
    let info_line = Line::from(vec![
        Span::styled("  build ", Style::default().fg(PINK_DEEP)),
        Span::styled("| ", Style::default().fg(PINK_SOFT)),
        Span::styled(&si.rust_version, Style::default().fg(PINK_PRIMARY)),
        Span::styled(" | ", Style::default().fg(PINK_SOFT)),
        Span::styled(&si.hostname, Style::default().fg(PINK_HOT)),
    ]);

    let lines = vec![logo_line, info_line];
    let para = Paragraph::new(Text::from(lines))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(PINK_PRIMARY))
            .title(" 🐱 樱花猫娘 ")
            .title_style(Style::default().fg(PINK_ACCENT).add_modifier(Modifier::BOLD)));
    f.render_widget(para, area);
}

/// 分隔线
fn render_separator(f: &mut Frame, area: Rect) {
    let sep = Line::from(Span::styled(
        "─".repeat(area.width as usize),
        Style::default().fg(PINK_SOFT),
    ));
    f.render_widget(Paragraph::new(Text::from(vec![sep])), area);
}

/// 紧凑状态栏 — CPU 走势 + 内存 + 磁盘 + 进程信息
fn render_status_bar(f: &mut Frame, area: Rect, app: &TuiApp) {
    let r = &app.resource;

    // CPU 行：走势图 + 百分比
    let spark_w = (area.width as usize).saturating_sub(20).min(40).max(4);
    let spark = app.render_cpu_sparkline(spark_w);

    let cpu_color = if r.cpu_percent > 80.0 { Color::Red } else { PINK_ACCENT };
    let cpu_line = Line::from(vec![
        Span::styled(" CPU ", Style::default().fg(PINK_HOT).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{:5.1}% ", r.cpu_percent), Style::default().fg(cpu_color)),
        Span::styled(spark, Style::default().fg(PINK_ACCENT)),
    ]);

    // 内存 + 磁盘 + 进程一行
    let mem_color = if r.memory_percent > 80.0 { Color::Red } else { PINK_DEEP };
    let disk_color = if r.disk_percent > 80.0 { Color::Red } else { PINK_DEEP };

    let mem_str = format!("{} {:.0}%/{}",
        TuiApp::format_mb(r.memory_used_mb), r.memory_percent,
        TuiApp::format_mb(r.memory_total_mb));
    let disk_str = format!("{:>3.0}% {:>3.0}/{:>3.0}GB",
        r.disk_percent, r.disk_used_mb as f64 / 1024.0, r.disk_total_mb as f64 / 1024.0);

    let stats_line = Line::from(vec![
        Span::styled(" MEM ", Style::default().fg(PINK_HOT).add_modifier(Modifier::BOLD)),
        Span::styled(&mem_str, Style::default().fg(mem_color)),
        Span::styled("  DISK ", Style::default().fg(PINK_HOT).add_modifier(Modifier::BOLD)),
        Span::styled(&disk_str, Style::default().fg(disk_color)),
    ]);

    // PID + Uptime + Logs 行
    let pid_line = Line::from(vec![
        Span::styled(" PID ", Style::default().fg(PINK_HOT).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{}", r.pid), Style::default().fg(GOLD_WARM)),
        Span::styled("  Uptime ", Style::default().fg(PINK_HOT).add_modifier(Modifier::BOLD)),
        Span::styled(app.uptime_str(), Style::default().fg(PINK_PRIMARY)),
        Span::styled("  Logs ", Style::default().fg(PINK_HOT).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{}", app.log_buffer.len()), Style::default().fg(GOLD_WARM)),
        Span::styled("  刷新 ", Style::default().fg(PINK_HOT).add_modifier(Modifier::BOLD)),
        Span::styled("每 2s", Style::default().fg(PINK_PRIMARY)),
    ]);

    let lines = vec![cpu_line, stats_line, pid_line];
    let para = Paragraph::new(Text::from(lines))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(PINK_PRIMARY)));
    f.render_widget(para, area);
}

/// 日志面板
fn render_log_panel(f: &mut Frame, area: Rect, app: &mut TuiApp) {
    let msg_overhead = if app.messages.is_empty() { 1 } else { 2 };
    let log_h = (area.height as usize).saturating_sub(msg_overhead);
    let logs = app.log_buffer.get_lines(log_h.max(1));
    let items: Vec<ListItem> = logs.iter().map(|line| {
        let style = if line.contains("ERROR") || line.contains("error") { Style::default().fg(Color::Red) }
        else if line.contains("WARN") || line.contains("warn") { Style::default().fg(GOLD_WARM) }
        else if line.contains("INFO") || line.contains("info") { Style::default().fg(GREEN_LEAF) }
        else if line.contains("DEBUG") || line.contains("debug") { Style::default().fg(PINK_DEEP) }
        else { Style::default().fg(PINK_SOFT) };
        ListItem::new(Line::from(Span::styled(line.clone(), style)))
    }).collect();

    let scroll = if app.auto_scroll { "◇自动" } else { "◇手动↑↓" };
    let title = format!(" ❀ 日志 {} ❀ ", scroll);
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL)
            .border_style(Style::default().fg(PINK_PRIMARY))
            .title(title)
            .title_style(Style::default().fg(PINK_HOT).add_modifier(Modifier::BOLD)));
    f.render_widget(list, area);
}

/// 底部命令栏 — OpenCode 风格输入框
fn render_command_bar(f: &mut Frame, area: Rect, app: &mut TuiApp) {
    let (text, color, title) = match app.command_mode {
        CommandMode::Normal => (
            format!(" [Q]退出 [R]刷新 [F]滚动 [/]命令 | {}",
                chrono::Local::now().format("%H:%M:%S")),
            PINK_PRIMARY,
            " ⌨ 樱花命令 ",
        ),
        CommandMode::Input => (
            format!(" > {} ▌", app.command_buffer),
            GOLD_WARM,
            " ✎ 输入命令 ",
        ),
    };
    let line = Line::from(Span::styled(text, Style::default().fg(color)));
    let bar = Paragraph::new(Text::from(vec![line]))
        .block(Block::default().borders(Borders::ALL)
            .border_style(Style::default().fg(PINK_PRIMARY))
            .title(title)
            .title_style(Style::default().fg(PINK_ACCENT).add_modifier(Modifier::BOLD)));
    f.render_widget(bar, area);
}

// ============================================================================
// 主入口
// ============================================================================

pub struct TuiGuard {
    running: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

#[allow(dead_code)] // 保留：TuiGuard 空实例用于占位
impl TuiGuard {
    pub fn start(log_buffer: Arc<LogBuffer>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();
        let handle = Some(std::thread::spawn(move || {
            if let Err(e) = run_tui_inner(log_buffer, r) {
                eprintln!("[TUI] 异常: {}", e);
            }
        }));
        Self { running, handle }
    }

    pub fn empty() -> Self {
        Self { running: Arc::new(AtomicBool::new(false)), handle: None }
    }
}

impl Drop for TuiGuard {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            self.running.store(false, Ordering::Relaxed);
            let _ = h.join();
        }
    }
}

fn run_tui_inner(log_buffer: Arc<LogBuffer>, running: Arc<AtomicBool>) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = TuiApp::new(log_buffer);
    let refresh_interval = Duration::from_secs(2);

    let result = (|| -> io::Result<()> {
        while running.load(Ordering::Relaxed) && !app.should_quit {
            if app.last_refresh.elapsed() >= refresh_interval {
                app.refresh();
            }
            render(&mut terminal, &mut app)?;

            if crossterm::event::poll(Duration::from_millis(100))? {
                match crossterm::event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        // Ctrl+C 在任何模式下都退出
                        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
                            app.should_quit = true;
                        } else {
                        match &app.command_mode {
                            CommandMode::Normal => match key.code {
                                KeyCode::Char('q') | KeyCode::Char('Q') => app.should_quit = true,
                                KeyCode::Char('r') | KeyCode::Char('R') => {
                                    app.system_info = collect_system_info(); app.refresh();
                                }
                                KeyCode::Char('f') | KeyCode::Char('F') => app.auto_scroll = !app.auto_scroll,
                                KeyCode::Char('/') | KeyCode::Char(':') => {
                                    app.command_mode = CommandMode::Input;
                                    app.command_buffer.clear(); app.command_cursor = 0;
                                }
                                KeyCode::Up if !app.auto_scroll => { app.scroll_offset = app.scroll_offset.saturating_add(1); }
                                KeyCode::Down if !app.auto_scroll && app.scroll_offset > 0 => { app.scroll_offset -= 1; }
                                KeyCode::PageUp => app.scroll_offset = app.scroll_offset.saturating_add(10),
                                KeyCode::PageDown => app.scroll_offset = app.scroll_offset.saturating_sub(10),
                                _ => {}
                            },
                            CommandMode::Input => match key.code {
                                KeyCode::Enter => { let cmd = app.command_buffer.clone(); app.execute_command(&cmd); }
                                KeyCode::Esc => { app.command_mode = CommandMode::Normal; app.command_buffer.clear(); app.command_cursor = 0; }
                                KeyCode::Backspace if app.command_cursor > 0 => { app.command_buffer.remove(app.command_cursor - 1); app.command_cursor -= 1; }
                                KeyCode::Delete if app.command_cursor < app.command_buffer.len() => { app.command_buffer.remove(app.command_cursor); }
                                KeyCode::Left if app.command_cursor > 0 => { app.command_cursor -= 1; }
                                KeyCode::Right if app.command_cursor < app.command_buffer.len() => { app.command_cursor += 1; }
                                KeyCode::Home => app.command_cursor = 0,
                                KeyCode::End => app.command_cursor = app.command_buffer.len(),
                                KeyCode::Up => { if let Some(p) = app.command_history.last() { app.command_buffer = p.clone(); app.command_cursor = app.command_buffer.len(); } }
                                KeyCode::Tab => {
                                    let known = ["quit", "help", "clear", "refresh", "echo "];
                                    let p = app.command_buffer.to_lowercase();
                                    for c in &known { if c.starts_with(&p) && c.len() > p.len() { app.command_buffer = c.to_string(); app.command_cursor = app.command_buffer.len(); break; } }
                                }
                                KeyCode::Char(ch) => { app.command_buffer.insert(app.command_cursor, ch); app.command_cursor += 1; }
                                _ => {}
                            },
                        }
                        }
                    }
                    Event::Resize(w, h) => { app.terminal_width = w; app.terminal_height = h; }
                    _ => {}
                }
            }
        }
        Ok(())
    })();

    let mut restore = || -> io::Result<()> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    };

    if let Err(e) = result { let _ = restore(); return Err(e); }
    restore()?;

    let final_logs = app.log_buffer.get_lines(20);
    if !final_logs.is_empty() {
        println!("\n━━━ 最后日志 ━━━");
        for line in final_logs.iter().rev().take(10) { println!("{}", line); }
        println!("━━━━━━━━━━━━━━━\n");
    }
    Ok(())
}