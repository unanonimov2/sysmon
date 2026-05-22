use std::{
    io,
    time::{Duration, Instant},
};

use chrono::Local;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        BarChart, Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline, Wrap,
    },
    Frame, Terminal,
};
use sysinfo::{CpuExt, DiskExt, NetworkExt, NetworksExt, ProcessExt, System, SystemExt};

const HISTORY_LEN: usize = 60;
const TICK_RATE_MS: u64 = 1000;

struct App {
    system: System,
    cpu_history: Vec<u64>,
    mem_history: Vec<u64>,
    selected_tab: usize,
    scroll_offset: usize,
    last_tick: Instant,
    uptime_secs: u64,
    net_recv_history: Vec<u64>,
    net_sent_history: Vec<u64>,
    prev_recv: u64,
    prev_sent: u64,
}

impl App {
    fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        App {
            system,
            cpu_history: vec![0; HISTORY_LEN],
            mem_history: vec![0; HISTORY_LEN],
            selected_tab: 0,
            scroll_offset: 0,
            last_tick: Instant::now(),
            uptime_secs: 0,
            net_recv_history: vec![0; HISTORY_LEN],
            net_sent_history: vec![0; HISTORY_LEN],
            prev_recv: 0,
            prev_sent: 0,
        }
    }

    fn on_tick(&mut self) {
        self.system.refresh_all();
        self.uptime_secs = self.system.uptime();

        let cpu = self.system.global_cpu_info().cpu_usage() as u64;
        self.cpu_history.remove(0);
        self.cpu_history.push(cpu);

        let mem_used = self.system.used_memory();
        let mem_total = self.system.total_memory();
        let mem_pct = if mem_total > 0 { mem_used * 100 / mem_total } else { 0 };
        self.mem_history.remove(0);
        self.mem_history.push(mem_pct);

        let (recv, sent) = self
            .system
            .networks()
            .iter()
            .fold((0u64, 0u64), |(r, s), (_, d)| {
                (r + d.received(), s + d.transmitted())
            });
        let delta_recv = recv.saturating_sub(self.prev_recv);
        let delta_sent = sent.saturating_sub(self.prev_sent);
        self.prev_recv = recv;
        self.prev_sent = sent;

        self.net_recv_history.remove(0);
        self.net_recv_history.push(delta_recv / 1024);
        self.net_sent_history.remove(0);
        self.net_sent_history.push(delta_sent / 1024);
    }

    fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    app.on_tick();

    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = TICK_RATE_MS
            .saturating_sub(app.last_tick.elapsed().as_millis() as u64);

        if event::poll(Duration::from_millis(timeout))? {
            if let Event::Key(key) = event::read()? {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                    (KeyCode::Tab, _) | (KeyCode::Right, _) => {
                        app.selected_tab = (app.selected_tab + 1) % 3;
                        app.scroll_offset = 0;
                    }
                    (KeyCode::BackTab, _) | (KeyCode::Left, _) => {
                        app.selected_tab = app.selected_tab.checked_sub(1).unwrap_or(2);
                        app.scroll_offset = 0;
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('j'), _) => app.scroll_down(),
                    (KeyCode::Up, _) | (KeyCode::Char('k'), _) => app.scroll_up(),
                    _ => {}
                }
            }
        }

        if app.last_tick.elapsed() >= Duration::from_millis(TICK_RATE_MS) {
            app.on_tick();
            app.last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let size = f.size();

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(size);

    draw_header(f, app, root[0]);
    draw_tabs(f, app, root[1]);

    match app.selected_tab {
        0 => draw_overview(f, app, root[2]),
        1 => draw_processes(f, app, root[2]),
        2 => draw_disks_net(f, app, root[2]),
        _ => {}
    }

    draw_footer(f, root[3]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let hostname = app.system.host_name().unwrap_or_else(|| "unknown".into());
    let os = app.system.long_os_version().unwrap_or_else(|| "unknown".into());
    let now = Local::now().format("%H:%M:%S").to_string();
    let uptime = format_uptime(app.uptime_secs);

    let text = Line::from(vec![
        Span::styled("  ", Style::default().fg(Color::Cyan)),
        Span::styled(
            format!("{hostname}"),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  |  "),
        Span::styled(os, Style::default().fg(Color::DarkGray)),
        Span::raw("  |  up "),
        Span::styled(uptime, Style::default().fg(Color::Yellow)),
        Span::raw("  |  "),
        Span::styled(now, Style::default().fg(Color::Green)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " SYSMON ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let para = Paragraph::new(text).block(block).alignment(Alignment::Left);
    f.render_widget(para, area);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = ["  Overview  ", "  Processes  ", "  Disk & Net  "];
    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    for (i, title) in titles.iter().enumerate() {
        if i == app.selected_tab {
            spans.push(Span::styled(
                *title,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                *title,
                Style::default().fg(Color::DarkGray),
            ));
        }
        spans.push(Span::raw(" "));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let para = Paragraph::new(Line::from(spans)).block(block);
    f.render_widget(para, area);
}

fn draw_overview(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Length(3),
            Constraint::Length(8),
        ])
        .split(cols[0]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(cols[1]);

    let cpu_pct = *app.cpu_history.last().unwrap_or(&0);
    let mem_pct = *app.mem_history.last().unwrap_or(&0);

    let cpu_color = gauge_color(cpu_pct);
    let mem_color = gauge_color(mem_pct);

    let cpu_gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(" CPU Usage ", Style::default().fg(Color::Cyan))),
        )
        .gauge_style(Style::default().fg(cpu_color))
        .percent(cpu_pct as u16)
        .label(format!("{cpu_pct}%"));
    f.render_widget(cpu_gauge, left[0]);

    let cpu_spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    " CPU History (60s) ",
                    Style::default().fg(Color::DarkGray),
                )),
        )
        .data(&app.cpu_history)
        .max(100)
        .style(Style::default().fg(cpu_color));
    f.render_widget(cpu_spark, left[1]);

    let mem_gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(" RAM Usage ", Style::default().fg(Color::Cyan))),
        )
        .gauge_style(Style::default().fg(mem_color))
        .percent(mem_pct as u16)
        .label(format!(
            "{} / {} GB  ({}%)",
            mb_to_gb(app.system.used_memory()),
            mb_to_gb(app.system.total_memory()),
            mem_pct
        ));
    f.render_widget(mem_gauge, left[2]);

    let mem_spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    " RAM History (60s) ",
                    Style::default().fg(Color::DarkGray),
                )),
        )
        .data(&app.mem_history)
        .max(100)
        .style(Style::default().fg(mem_color));
    f.render_widget(mem_spark, left[3]);

    let cpus = app.system.cpus();
    let cpu_data: Vec<(&str, u64)> = cpus
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let name: &'static str = Box::leak(format!("C{i}").into_boxed_str());
            (name, c.cpu_usage() as u64)
        })
        .collect();

    let per_cpu = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    format!(" Per-Core ({} cores) ", cpus.len()),
                    Style::default().fg(Color::Cyan),
                )),
        )
        .data(&cpu_data)
        .bar_width(3)
        .bar_gap(1)
        .max(100)
        .bar_style(Style::default().fg(Color::Cyan))
        .value_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(per_cpu, right[0]);

    let swap_used = app.system.used_swap();
    let swap_total = app.system.total_swap();
    let swap_pct = if swap_total > 0 {
        swap_used * 100 / swap_total
    } else {
        0
    };

    let info_lines = vec![
        Line::from(vec![
            Span::styled("  Kernel   : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.system.kernel_version().unwrap_or_else(|| "?".into()),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  CPU Name : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                cpus.first()
                    .map(|c| c.brand().to_string())
                    .unwrap_or_else(|| "?".into()),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Total RAM: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.1} GB", app.system.total_memory() as f64 / 1_073_741_824.0),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Swap Used: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(
                    "{} / {} MB  ({}%)",
                    swap_used / 1_048_576,
                    swap_total / 1_048_576,
                    swap_pct
                ),
                Style::default().fg(gauge_color(swap_pct)),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Processes: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.system.processes().len().to_string(),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Load avg : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                {
                    let la = app.system.load_average();
                    format!("{:.2}  {:.2}  {:.2}", la.one, la.five, la.fifteen)
                },
                Style::default().fg(Color::Yellow),
            ),
        ]),
    ];

    let info = Paragraph::new(info_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(" System Info ", Style::default().fg(Color::Cyan))),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(info, right[1]);
}

fn draw_processes(f: &mut Frame, app: &App, area: Rect) {
    let mut procs: Vec<_> = app.system.processes().values().collect();
    procs.sort_by(|a, b| {
        b.cpu_usage()
            .partial_cmp(&a.cpu_usage())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let header = Line::from(vec![
        Span::styled(
            format!("{:<8} {:<25} {:>6}  {:>10}  {}",
                "PID", "NAME", "CPU%", "MEM(MB)", "STATUS"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
    ]);

    let mut items: Vec<ListItem> = vec![ListItem::new(header)];

    for proc in procs.iter().skip(app.scroll_offset).take(area.height as usize - 4) {
        let cpu = proc.cpu_usage();
        let mem_mb = proc.memory() / 1_048_576;
        let name = proc.name();
        let pid = proc.pid();
        let status = format!("{:?}", proc.status());

        let color = if cpu > 50.0 {
            Color::Red
        } else if cpu > 20.0 {
            Color::Yellow
        } else {
            Color::White
        };

        let line = Line::from(Span::styled(
            format!("{:<8} {:<25} {:>5.1}%  {:>10}  {}",
                pid.to_string(), &name[..name.len().min(24)], cpu, mem_mb, status),
            Style::default().fg(color),
        ));
        items.push(ListItem::new(line));
    }

    let total = procs.len();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                format!(" Processes ({total}) -- up/down to scroll "),
                Style::default().fg(Color::Cyan),
            )),
    );
    f.render_widget(list, area);
}

fn draw_disks_net(f: &mut Frame, app: &App, area: Rect) {
    let halves = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let mut disk_lines: Vec<Line> = vec![Line::from(Span::styled(
        format!("{:<30} {:>10} {:>10} {:>6}  Bar", "Mount", "Used", "Total", "%"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    ))];

    for disk in app.system.disks() {
        let total = disk.total_space();
        let avail = disk.available_space();
        let used = total.saturating_sub(avail);
        let pct = if total > 0 { used * 100 / total } else { 0 };
        let bar_len = (pct as usize * 20 / 100).min(20);
        let bar = format!("[{}{}]", "#".repeat(bar_len), "-".repeat(20 - bar_len));
        let mount = disk.mount_point().to_string_lossy();

        let color = gauge_color(pct);
        disk_lines.push(Line::from(Span::styled(
            format!(
                "{:<30} {:>10} {:>10} {:>5}%  {}",
                &mount[..mount.len().min(29)],
                format_bytes(used),
                format_bytes(total),
                pct,
                bar
            ),
            Style::default().fg(color),
        )));
    }

    let disk_widget = Paragraph::new(disk_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(" Disks ", Style::default().fg(Color::Cyan))),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(disk_widget, halves[0]);

    let net_halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(halves[1]);

    let recv_max = app.net_recv_history.iter().copied().max().unwrap_or(1).max(1);
    let recv_spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    format!(
                        " Download (now: {} KB/s) ",
                        app.net_recv_history.last().unwrap_or(&0)
                    ),
                    Style::default().fg(Color::Green),
                )),
        )
        .data(&app.net_recv_history)
        .max(recv_max)
        .style(Style::default().fg(Color::Green));
    f.render_widget(recv_spark, net_halves[0]);

    let sent_max = app.net_sent_history.iter().copied().max().unwrap_or(1).max(1);
    let sent_spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    format!(
                        " Upload (now: {} KB/s) ",
                        app.net_sent_history.last().unwrap_or(&0)
                    ),
                    Style::default().fg(Color::Yellow),
                )),
        )
        .data(&app.net_sent_history)
        .max(sent_max)
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(sent_spark, net_halves[1]);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let text = Line::from(vec![
        Span::styled(" [Tab] ", Style::default().fg(Color::Cyan)),
        Span::raw("cambiar tab  "),
        Span::styled("[j/k] [up/down] ", Style::default().fg(Color::Cyan)),
        Span::raw("scroll  "),
        Span::styled("[q] ", Style::default().fg(Color::Red)),
        Span::raw("salir"),
    ]);
    let para = Paragraph::new(text).alignment(Alignment::Center);
    f.render_widget(para, area);
}

fn gauge_color(pct: u64) -> Color {
    if pct >= 85 { Color::Red }
    else if pct >= 60 { Color::Yellow }
    else { Color::Green }
}

fn mb_to_gb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_073_741_824.0)
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.0} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    }
}

fn format_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}h {m:02}m {s:02}s")
}