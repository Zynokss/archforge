//! Keyboard-driven terminal shell for the existing Arch Forge operations.
#[path = "tui_backend.rs"]
mod backend;
use backend::{Action, Scan};
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const PAGES: [&str; 8] = [
    "Overview",
    "Packages",
    "Cleanup",
    "Diagnostics",
    "Build loadout",
    "Capture loadout",
    "Import loadout",
    "Wiki",
];
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pane {
    Inactive,
    Nav,
    Categories,
    Items,
    Selection,
}
#[derive(Default)]
struct PageState {
    cursor: usize,
    offset: usize,
    query: String,
    category: usize,
    filter: usize,
    source_filter: usize,
    sort: usize,
}
#[derive(Clone)]
struct Package {
    name: String,
    title: String,
    category: String,
    description: String,
    source: String,
}
const SORTS: &[&str] = &["Installed first", "Name A–Z", "Name Z–A"];
const SOURCES: &[&str] = &["All sources", "Repository", "Foreign/local", "AUR recipes"];
fn catalog_package(name: &str, source: &str, fallback: &str) -> Package {
    type Metadata = BTreeMap<String, (String, String, String)>;
    static CATALOG: std::sync::OnceLock<Metadata> = std::sync::OnceLock::new();
    let catalog = CATALOG.get_or_init(|| {
        include_str!("../data/loadouts/catalog.tsv")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let fields: Vec<_> = l.split('|').collect();
                (fields.len() == 8).then(|| {
                    (
                        fields[1].into(),
                        (fields[2].into(), fields[0].into(), fields[3].into()),
                    )
                })
            })
            .collect()
    });
    let data = crate::package_variants::resolve(catalog.keys().map(String::as_str), name)
        .and_then(|actual| catalog.get(actual));
    Package {
        name: name.into(),
        title: data.map(|d| d.0.clone()).unwrap_or_else(|| name.into()),
        category: data
            .map(|d| d.1.clone())
            .unwrap_or_else(|| "Uncategorized".into()),
        description: data.map(|d| d.2.clone()).unwrap_or_else(|| fallback.into()),
        source: source.into(),
    }
}
#[derive(Clone, Copy)]
enum InputKind {
    Search,
    OutputSearch,
    Capture,
    Import,
    Export,
    Custom,
}
enum Dialog {
    Help(usize),
    Input(InputKind, String),
    Review {
        action: Action,
        reviewed: bool,
        focus: usize,
        scroll: usize,
    },
}
enum Message {
    Scan(Box<Scan>),
    Metrics(crate::system::Metrics),
    Output(Action, Result<String, String>),
    Saved(Result<String, String>),
    Imported(Result<(Vec<String>, usize), String>),
}
struct App {
    page: usize,
    nav: usize,
    pane: Pane,
    pages: [PageState; 8],
    scan: Scan,
    variants: BTreeMap<String, String>,
    scanning: bool,
    metrics: Option<crate::system::Metrics>,
    selected: BTreeSet<String>,
    packages: Vec<Package>,
    build: Vec<Package>,
    category_cache: BTreeMap<usize, Vec<String>>,
    dialog: Option<Dialog>,
    status: String,
    output: String,
    output_title: String,
    output_open: bool,
    output_query: String,
    output_scroll: usize,
    output_horizontal: u16,
    output_height: usize,
    summary_scroll: usize,
    busy: bool,
    updates: Option<usize>,
    update_names: BTreeSet<String>,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
    execute: Option<Action>,
    last_update: Option<Instant>,
    quit: bool,
    no_color: bool,
}
impl App {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let build = crate::cli::catalog()
            .into_iter()
            .map(|f| Package {
                name: f[1].into(),
                title: f[2].into(),
                category: f[0].into(),
                description: f[3].into(),
                source: f[5].into(),
            })
            .collect();
        Self {
            page: 0,
            nav: 0,
            pane: Pane::Items,
            pages: std::array::from_fn(|_| PageState::default()),
            scan: Scan::default(),
            variants: BTreeMap::new(),
            scanning: true,
            metrics: None,
            selected: BTreeSet::new(),
            packages: vec![],
            build,
            category_cache: BTreeMap::new(),
            dialog: None,
            status: "Scanning packages and hardware…".into(),
            output: String::new(),
            output_title: "Command output".into(),
            output_open: false,
            output_query: String::new(),
            output_scroll: 0,
            output_horizontal: 0,
            output_height: 1,
            summary_scroll: 0,
            busy: false,
            updates: None,
            update_names: BTreeSet::new(),
            tx,
            rx,
            execute: None,
            last_update: None,
            quit: false,
            no_color: std::env::var_os("NO_COLOR").is_some(),
        }
    }
    fn rescan(&mut self) {
        if self.scanning {
            return;
        }
        self.scanning = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Message::Scan(Box::new(backend::scan())));
        });
    }
    fn refresh_catalog(&mut self) {
        self.variants = crate::package_variants::alias_index(&self.scan.installed);
        self.packages = self
            .scan
            .available
            .iter()
            .chain(self.scan.installed.keys())
            .map(|n| self.installed_name(n).to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|n| {
                catalog_package(
                    &n,
                    if self.scan.available.contains(&n) {
                        "repo"
                    } else if self.scan.repository_error.is_some() {
                        "unknown"
                    } else {
                        "foreign"
                    },
                    "Package from this system's configured databases.",
                )
            })
            .collect();
        self.packages
            .sort_by_key(|p| (!self.scan.installed.contains_key(&p.name), p.name.clone()));
        self.normalize_selection();
        self.cache_categories();
    }
    fn installed_name<'a>(&'a self, name: &'a str) -> &'a str {
        if self.scan.installed.contains_key(name) {
            name
        } else {
            self.variants
                .get(crate::package_variants::base_name(name))
                .map(String::as_str)
                .unwrap_or(name)
        }
    }
    fn normalize_selection(&mut self) {
        self.selected = std::mem::take(&mut self.selected)
            .into_iter()
            .map(|n| self.installed_name(&n).to_owned())
            .collect();
    }
    fn poll(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Message::Scan(scan) => {
                    if scan.inventory.error.is_some() {
                        self.update_names.clear();
                        self.updates = None;
                    } else {
                        self.update_names.retain(|name| {
                            scan.installed.contains_key(name)
                                && scan.installed.get(name) == self.scan.installed.get(name)
                        });
                        if self.updates.is_some() {
                            self.updates = Some(self.update_names.len());
                        }
                    }
                    self.scan = *scan;
                    self.scanning = false;
                    self.last_update = Some(Instant::now());
                    self.status = if let Some(e) = &self.scan.inventory.error {
                        format!("Package scan failed: {e}. Press r to retry.")
                    } else if let Some(e) = &self.scan.repository_error {
                        format!(
                            "Installed packages scanned; repository catalog unavailable: {e}. Press r to retry."
                        )
                    } else {
                        format!("Ready · {} installed packages", self.scan.installed.len())
                    };
                    self.refresh_catalog();
                }
                Message::Metrics(metrics) => self.metrics = Some(metrics),
                Message::Output(action, result) => {
                    self.busy = false;
                    let ok = result.is_ok();
                    let text = result.unwrap_or_else(|e| e);
                    if action.program.ends_with("checkupdates") && ok {
                        self.update_names = text
                            .lines()
                            .filter_map(|l| {
                                let f: Vec<_> = l.split_whitespace().collect();
                                (f.len() == 4 && f[2] == "->").then(|| f[0].to_owned())
                            })
                            .collect();
                        self.updates = Some(self.update_names.len());
                    }
                    self.output = backend::clean_output(text.as_bytes());
                    self.output_title = action.title;
                    self.output_scroll = 0;
                    self.output_query.clear();
                    self.status = if ok {
                        "Completed · o opens the log"
                    } else {
                        "Command failed · inspect output; r rescans system state"
                    }
                    .into();
                }
                Message::Saved(result) => {
                    self.status = result.unwrap_or_else(|e| format!("Could not save: {e}"))
                }
                Message::Imported(result) => match result {
                    Ok((names, skipped)) => {
                        self.selected = names.into_iter().collect();
                        self.normalize_selection();
                        let extra:Vec<_>=self.selected.iter().filter(|n|!self.build.iter().any(|p|&p.name==*n)).map(|n|catalog_package(n,"repo","Imported repository package; availability is checked before installation.")).collect();
                        self.build.extend(extra);
                        self.cache_categories();
                        self.page = 4;
                        self.nav = 4;
                        self.pane = Pane::Items;
                        self.status = format!(
                            "Imported {} repository entries; {skipped} AUR/foreign entries excluded. Dotfiles were not applied.",
                            self.selected.len()
                        );
                    }
                    Err(e) => self.status = format!("Import failed: {e}"),
                },
            }
        }
    }
    fn package_page(&self) -> bool {
        self.page == 1 || self.page == 4
    }
    fn source(&self) -> &[Package] {
        if self.page == 4 {
            &self.build
        } else {
            &self.packages
        }
    }
    fn cache_categories(&mut self) {
        for (page, source) in [(1, &self.packages), (4, &self.build)] {
            let selected = self
                .category_cache
                .get(&page)
                .and_then(|c| c.get(self.pages[page].category))
                .cloned();
            let mut categories = vec!["All categories".into()];
            categories.extend(
                source
                    .iter()
                    .map(|p| p.category.clone())
                    .collect::<BTreeSet<_>>(),
            );
            self.pages[page].category = selected
                .and_then(|name| categories.iter().position(|c| c == &name))
                .unwrap_or(0);
            self.category_cache.insert(page, categories);
        }
    }
    fn categories(&self) -> Vec<String> {
        self.category_cache
            .get(&if self.page == 4 { 4 } else { 1 })
            .cloned()
            .unwrap_or_else(|| vec!["All categories".into()])
    }
    fn package_source<'a>(&'a self, package: &'a Package) -> &'a str {
        let actual = self.installed_name(&package.name);
        if self.scan.installed.contains_key(actual) {
            if self.scan.repository_error.is_some() {
                "unknown"
            } else if self.scan.available.contains(actual) {
                "repo"
            } else {
                "foreign"
            }
        } else if self.scan.available.contains(&package.name) {
            "repo"
        } else {
            &package.source
        }
    }
    fn visible_packages(&self) -> Vec<usize> {
        let st = &self.pages[self.page];
        let categories = self.categories();
        let category = categories.get(st.category);
        let query = st.query.to_lowercase();
        let visible: Vec<_> = self
            .source()
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let actual = self.installed_name(&p.name);
                let installed = self.scan.installed.contains_key(actual);
                let matches_filter = match st.filter {
                    1 => installed,
                    2 => !installed,
                    3 => self.update_names.contains(actual),
                    _ => true,
                };
                matches_filter
                    && match st.source_filter {
                        1 => self.package_source(p) == "repo",
                        2 => self.package_source(p) == "foreign",
                        3 => self.package_source(p) == "aur",
                        _ => true,
                    }
                    && (st.category == 0 || category == Some(&p.category))
                    && (query.is_empty()
                        || format!("{} {} {} {}", actual, p.name, p.title, p.description)
                            .to_lowercase()
                            .contains(&query))
            })
            .map(|(i, _)| i)
            .collect();
        // A catalog entry and an exact-name addition may resolve to one installed package.
        // Keep the exact row when available, without showing duplicate variants.
        let mut unique = BTreeMap::new();
        for index in visible {
            let package = &self.source()[index];
            let actual = self.installed_name(&package.name);
            if !unique.contains_key(actual) || actual == package.name {
                unique.insert(actual, index);
            }
        }
        let mut visible: Vec<_> = unique.into_values().collect();
        visible.sort_by(|a, b| {
            let (a, b) = (&self.source()[*a], &self.source()[*b]);
            let names = || {
                self.installed_name(&a.name)
                    .cmp(self.installed_name(&b.name))
            };
            match st.sort {
                1 => names(),
                2 => names().reverse(),
                _ => {
                    let installed = |p: &Package| {
                        self.scan
                            .installed
                            .contains_key(self.installed_name(&p.name))
                    };
                    installed(b).cmp(&installed(a)).then_with(names)
                }
            }
        });
        visible
    }

    fn wiki_entries(&self) -> Vec<usize> {
        let q = self.pages[7].query.to_lowercase();
        crate::wiki_data::ENTRIES
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.topic != 7
                    && !e.command.starts_with("glxinfo ")
                    && format!(
                        "{} {} {} {}",
                        crate::wiki_data::TOPICS[e.topic].title,
                        e.title,
                        e.description,
                        e.command
                    )
                    .to_lowercase()
                    .contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }
    fn actions(&self) -> Vec<Action> {
        match self.page {
            0 => vec![
                backend::diagnostics().remove(0),
                backend::check_updates(),
                Action::new(
                    "Build a loadout",
                    "Choose useful terminal tools and review a bulk installation.",
                    "",
                    &[],
                    false,
                ),
                Action::new(
                    "Review cleanup",
                    "Preview cached packages and archived logs before removal.",
                    "",
                    &[],
                    false,
                ),
            ],
            2 => backend::cleanup(),
            3 => backend::diagnostics(),
            _ => vec![],
        }
    }
    fn len(&self) -> usize {
        if self.output_open {
            return self.output_lines().len();
        }
        if self.package_page() {
            self.visible_packages().len()
        } else if self.page == 7 {
            self.wiki_entries().len()
        } else if self.page == 5 || self.page == 6 {
            1
        } else {
            self.actions().len()
        }
    }
    fn active_package(&self) -> Option<Package> {
        let rows = self.visible_packages();
        rows.get(self.pages[self.page].cursor)
            .map(|i| self.source()[*i].clone())
    }
    fn pending_action(&self) -> Option<Action> {
        if self.package_page() {
            if !self.selected.is_empty() {
                backend::install_plan(&self.selected, &self.scan).ok()
            } else {
                self.active_package().map(|p| {
                    let actual = self.installed_name(&p.name);
                    Action::new(
                        "Package information",
                        "Shows metadata for this package. Read-only.",
                        "/usr/bin/pacman",
                        &[
                            if self.scan.installed.contains_key(actual) {
                                "-Qi"
                            } else {
                                "-Si"
                            },
                            "--",
                            actual,
                        ],
                        false,
                    )
                })
            }
        } else {
            self.actions().get(self.pages[self.page].cursor).cloned()
        }
    }
    fn read_action(&mut self, action: Action) {
        if self.busy {
            self.status = "Another inspection is running. You can keep browsing.".into();
            return;
        }
        self.busy = true;
        self.output_open = true;
        self.output_title = action.title.clone();
        self.output.clear();
        self.output_query.clear();
        self.output_scroll = 0;
        self.status = "Running read-only inspection…".into();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = action.read();
            let _ = tx.send(Message::Output(action, result));
        });
    }
    fn review(&mut self, action: Action) {
        self.dialog = Some(Dialog::Review {
            action,
            reviewed: false,
            focus: 0,
            scroll: 0,
        });
    }
    fn plan_install(&mut self) {
        match backend::install_plan(&self.selected, &self.scan) {
            Ok(a) => self.review(a),
            Err(e) => self.status = e,
        }
    }
    fn open(&mut self) {
        if self.pane == Pane::Nav {
            self.page = self.nav;
            self.pane = Pane::Items;
            self.output_open = false;
            return;
        }
        if self.output_open {
            return;
        }
        if self.pane == Pane::Categories {
            return;
        }
        if self.pane == Pane::Selection {
            self.plan_install();
            return;
        }
        if self.package_page() {
            if let Some(p) = self.active_package() {
                let actual = self.installed_name(&p.name);
                let a = Action::new(
                    &format!("{} · package information", p.title),
                    "Reads exact package metadata.",
                    "/usr/bin/pacman",
                    &[
                        if self.scan.installed.contains_key(actual) {
                            "-Qi"
                        } else {
                            "-Si"
                        },
                        "--",
                        actual,
                    ],
                    false,
                );
                self.review(a);
            }
            return;
        }
        match self.page {
            0 => match self.pages[0].cursor {
                0 => self.review(backend::diagnostics().remove(0)),
                1 => self.review(backend::check_updates()),
                2 => {
                    self.page = 4;
                    self.nav = 4;
                }
                _ => {
                    self.page = 2;
                    self.nav = 2;
                }
            },
            2 | 3 => {
                if let Some(action) = self.actions().get(self.pages[self.page].cursor).cloned() {
                    self.review(action)
                }
            }
            5 => self.dialog = Some(Dialog::Input(InputKind::Capture, String::new())),
            6 => self.dialog = Some(Dialog::Input(InputKind::Import, String::new())),
            7 => {
                if let Some(i) = self.wiki_entries().get(self.pages[7].cursor) {
                    let e = &crate::wiki_data::ENTRIES[*i];
                    self.output = format!(
                        "{}\n\n{}\n\n{}\n\n{}\n\n{}\n\nReference only; replace UPPERCASE placeholders. Nothing has run.",
                        e.title,
                        e.command,
                        e.description,
                        e.effect,
                        crate::wiki_data::TOPICS[e.topic].url
                    );
                    self.output_title = "Wiki reference".into();
                    self.output_open = true;
                    self.output_scroll = 0;
                    self.output_query.clear();
                }
            }
            _ => (),
        }
    }
    fn output_lines(&self) -> Vec<(usize, &str)> {
        self.output
            .lines()
            .enumerate()
            .filter(|(_, l)| {
                self.output_query.is_empty()
                    || l.to_lowercase().contains(&self.output_query.to_lowercase())
            })
            .collect()
    }
    fn move_cursor(&mut self, delta: isize) {
        if self.output_open && self.pane != Pane::Nav {
            let len = self.output_lines().len();
            self.output_scroll = self
                .output_scroll
                .saturating_add_signed(delta)
                .min(len.saturating_sub(self.output_height));
            return;
        }
        if matches!(self.page, 5 | 6) && self.pane == Pane::Items {
            self.pages[self.page].offset =
                self.pages[self.page].offset.saturating_add_signed(delta);
            return;
        }
        if self.pane == Pane::Selection {
            self.summary_scroll = self
                .summary_scroll
                .saturating_add_signed(delta)
                .min(self.selected.len().saturating_sub(1));
            return;
        }
        match self.pane {
            Pane::Nav => self.nav = self.nav.saturating_add_signed(delta).min(7),
            Pane::Categories => {
                let count = self.categories().len();
                let st = &mut self.pages[self.page];
                st.category = st
                    .category
                    .saturating_add_signed(delta)
                    .min(count.saturating_sub(1));
                st.cursor = 0;
                st.offset = 0;
            }
            _ => {
                let len = self.len();
                let st = &mut self.pages[self.page];
                st.cursor = st
                    .cursor
                    .saturating_add_signed(delta)
                    .min(len.saturating_sub(1));
            }
        }
    }
    fn switch_pane(&mut self, back: bool) {
        let panes = if self.package_page() && !self.output_open {
            vec![Pane::Nav, Pane::Categories, Pane::Items, Pane::Selection]
        } else {
            vec![Pane::Nav, Pane::Items]
        };
        let i = panes.iter().position(|p| *p == self.pane).unwrap_or(0);
        self.pane = panes[(i + if back { panes.len() - 1 } else { 1 }) % panes.len()];
    }
    fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        if let Some(dialog) = self.dialog.take() {
            match dialog {
                Dialog::Help(mut scroll) => {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => scroll = scroll.saturating_sub(1),
                        KeyCode::Down | KeyCode::Char('j') => scroll = scroll.saturating_add(1),
                        KeyCode::PageDown => scroll = scroll.saturating_add(5),
                        KeyCode::PageUp => scroll = scroll.saturating_sub(5),
                        _ => (),
                    }
                    if !matches!(
                        key.code,
                        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Enter
                    ) {
                        self.dialog = Some(Dialog::Help(scroll));
                    }
                }
                Dialog::Input(kind, mut text) => {
                    match key.code {
                        KeyCode::Esc => return,
                        KeyCode::Enter => {
                            self.submit_input(kind, text);
                            return;
                        }
                        KeyCode::Backspace => {
                            text.pop();
                        }
                        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if text.len() < 4096 {
                                text.push(c);
                            }
                        }
                        _ => (),
                    };
                    self.dialog = Some(Dialog::Input(kind, text));
                }
                Dialog::Review {
                    action,
                    mut reviewed,
                    mut focus,
                    mut scroll,
                } => {
                    if key.code == KeyCode::Esc {
                        return;
                    }
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => scroll = scroll.saturating_sub(1),
                        KeyCode::Down | KeyCode::Char('j') => scroll = scroll.saturating_add(1),
                        KeyCode::PageUp => scroll = scroll.saturating_sub(5),
                        KeyCode::PageDown => scroll = scroll.saturating_add(5),
                        KeyCode::Home => scroll = 0,
                        KeyCode::Tab | KeyCode::BackTab | KeyCode::Left | KeyCode::Right => {
                            focus = 1 - focus
                        }
                        KeyCode::Char(' ') if focus == 0 => reviewed = !reviewed,
                        KeyCode::Enter if focus == 0 => reviewed = !reviewed,
                        KeyCode::Enter if focus == 1 && reviewed => {
                            self.execute = Some(action);
                            return;
                        }
                        _ => (),
                    };
                    self.dialog = Some(Dialog::Review {
                        action,
                        reviewed,
                        focus,
                        scroll,
                    });
                }
            }
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.dialog = Some(Dialog::Help(0)),
            KeyCode::Esc => {
                if self.output_open {
                    self.output_open = false;
                } else {
                    self.pane = Pane::Nav;
                }
            }
            KeyCode::Tab => self.switch_pane(false),
            KeyCode::BackTab => self.switch_pane(true),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::PageDown => self.move_cursor(10),
            KeyCode::PageUp => self.move_cursor(-10),
            KeyCode::Home => self.move_cursor(-1000000),
            KeyCode::End => self.move_cursor(1000000),
            KeyCode::Left if self.output_open => {
                self.output_horizontal = self.output_horizontal.saturating_sub(8)
            }
            KeyCode::Right if self.output_open => {
                self.output_horizontal = self.output_horizontal.saturating_add(8)
            }
            KeyCode::Enter => self.open(),
            KeyCode::Char('/') => {
                let (kind, q) = if self.output_open {
                    (InputKind::OutputSearch, self.output_query.clone())
                } else {
                    (InputKind::Search, self.pages[self.page].query.clone())
                };
                self.dialog = Some(Dialog::Input(kind, q));
            }
            KeyCode::Char('o') => {
                if !self.output.is_empty() || self.busy {
                    self.output_open = !self.output_open;
                    self.pane = Pane::Items;
                } else {
                    self.status = "No command output yet. Choose an inspection first.".into();
                }
            }
            KeyCode::Char('r') => self.rescan(),
            KeyCode::Char('u') => self.review(backend::check_updates()),
            KeyCode::Char('U') => self.review(backend::update()),
            KeyCode::Char('t') if self.package_page() && !self.output_open => {
                let st = &mut self.pages[self.page];
                st.source_filter = (st.source_filter + 1) % SOURCES.len();
                st.cursor = 0;
                st.offset = 0;
            }
            KeyCode::Char('S') if self.package_page() && !self.output_open => {
                let st = &mut self.pages[self.page];
                st.sort = (st.sort + 1) % SORTS.len();
                st.cursor = 0;
                st.offset = 0;
            }
            KeyCode::Char('R') if self.package_page() && !self.output_open => {
                let st = &mut self.pages[self.page];
                st.query.clear();
                st.category = 0;
                st.filter = 0;
                st.source_filter = 0;
                st.cursor = 0;
                st.offset = 0;
                self.status = "Filters reset · package selection preserved".into();
            }
            KeyCode::Char('e') if self.output_open => {
                self.dialog = Some(Dialog::Input(InputKind::Export, String::new()))
            }
            KeyCode::Char('c') if self.output_open => {
                self.status = copy(&self.output).unwrap_or_else(|e| e);
            }
            KeyCode::Char(' ')
                if self.package_page() && !self.output_open && self.pane == Pane::Items =>
            {
                if let Some(p) = self.active_package() {
                    let actual = self.installed_name(&p.name).to_owned();
                    if !self.selected.remove(&actual) {
                        if self.selected.len() < 5000 {
                            self.selected.insert(actual);
                        } else {
                            self.status =
                                "Selection limit reached (5,000). Narrow the selection.".into();
                        }
                    }
                }
            }
            KeyCode::Char('a') if self.package_page() && !self.output_open => {
                let rows = self.visible_packages();
                if rows.len() > 5000 {
                    self.status = "More than 5,000 matches. Narrow the category or search before bulk selection.".into();
                    return;
                }
                let names: Vec<_> = rows
                    .iter()
                    .map(|i| self.installed_name(&self.source()[*i].name).to_owned())
                    .collect();
                if names.iter().all(|n| self.selected.contains(n)) {
                    for n in names {
                        self.selected.remove(&n);
                    }
                } else {
                    self.selected.extend(names);
                }
                self.status = format!("{} selected", self.selected.len());
            }
            KeyCode::Char('x') if self.package_page() && !self.output_open => self.selected.clear(),
            KeyCode::Char('f') if self.package_page() && !self.output_open => {
                let st = &mut self.pages[self.page];
                st.filter = (st.filter + 1) % 4;
                st.cursor = 0;
                st.offset = 0;
            }
            KeyCode::Char('i') if self.package_page() && !self.output_open => self.plan_install(),
            KeyCode::Char('d') if self.package_page() && !self.output_open => {
                match backend::removal(&self.selected, &self.scan) {
                    Ok(a) => self.review(a),
                    Err(e) => self.status = e,
                }
            }
            KeyCode::Char('+') if self.package_page() && !self.output_open => {
                self.dialog = Some(Dialog::Input(InputKind::Custom, String::new()))
            }
            KeyCode::Char('s') if self.package_page() && !self.output_open => {
                self.dialog = Some(Dialog::Input(InputKind::Export, String::new()))
            }
            KeyCode::Char(c @ '1'..='8') => {
                self.page = (c as u8 - b'1') as usize;
                self.nav = self.page;
                self.output_open = false;
                self.pane = Pane::Items;
            }
            _ => (),
        }
    }
    fn submit_input(&mut self, kind: InputKind, text: String) {
        match kind {
            InputKind::Search => {
                if !self.package_page() && self.page != 7 {
                    self.page = 1;
                    self.nav = 1;
                    self.pane = Pane::Items;
                }
                self.pages[self.page].query = text;
                self.pages[self.page].cursor = 0;
                self.pages[self.page].offset = 0;
            }
            InputKind::OutputSearch => {
                self.output_query = text;
                self.output_scroll = 0;
            }
            InputKind::Capture => {
                if text.is_empty() {
                    return;
                }
                let tx = self.tx.clone();
                self.status = "Capturing package names…".into();
                std::thread::spawn(move || {
                    let _ = tx.send(Message::Saved(backend::capture(&text)));
                });
            }
            InputKind::Import => {
                if text.is_empty() {
                    return;
                }
                let tx = self.tx.clone();
                self.status = "Reading loadout profile…".into();
                std::thread::spawn(move || {
                    let _ = tx.send(Message::Imported(backend::import(&text)));
                });
            }
            InputKind::Export => {
                if text.is_empty() {
                    return;
                }
                let content = if self.output_open {
                    self.output.clone()
                } else {
                    match backend::selection_profile(&self.selected, &self.scan) {
                        Ok(content) => content,
                        Err(e) => {
                            self.status = e;
                            return;
                        }
                    }
                };
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let result =
                        backend::write_new(&text, &content).map(|()| format!("Saved {text}"));
                    let _ = tx.send(Message::Saved(result));
                });
            }
            InputKind::Custom => match crate::cli::names(&text) {
                Ok(names) => {
                    for n in names {
                        if !self.build.iter().any(|p| p.name == n) {
                            self.build.push(catalog_package(&n,"repo","Exact-name addition. Repository availability is checked before installation."));
                        }
                        self.selected.insert(n);
                    }
                    self.normalize_selection();
                    self.cache_categories();
                    self.status = "Added to selection. Review the plan with i.".into();
                }
                Err(e) => self.status = e,
            },
        }
    }
}

struct ScreenGuard;
impl ScreenGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        if let Err(e) = execute!(io::stdout(), EnterAlternateScreen, Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(e);
        }
        Ok(Self)
    }
    fn leave(&self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
    }
    fn resume(&self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)
    }
}
impl Drop for ScreenGuard {
    fn drop(&mut self) {
        self.leave();
    }
}
pub(crate) fn run() -> io::Result<()> {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
        previous(info);
    }));
    let done = Arc::new(AtomicBool::new(false));
    let mut signal_ids = vec![];
    for signal in [
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
        signal_hook::consts::SIGINT,
    ] {
        signal_ids.push(signal_hook::flag::register(signal, done.clone())?);
    }
    struct Signals(Vec<signal_hook::SigId>);
    impl Drop for Signals {
        fn drop(&mut self) {
            for id in &self.0 {
                signal_hook::low_level::unregister(*id);
            }
        }
    }
    let _signals = Signals(signal_ids);
    let guard = ScreenGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut app = App::new();
    app.scanning = false;
    app.rescan();
    let tx = app.tx.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    std::thread::spawn(move || {
        let mut sampler = crate::system::Sampler::default();
        while !worker_stop.load(Ordering::Relaxed) {
            if tx.send(Message::Metrics(sampler.sample())).is_err() {
                break;
            }
            for _ in 0..20 {
                if worker_stop.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
    struct Stop(Arc<AtomicBool>);
    impl Drop for Stop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }
    let _stop = Stop(stop);
    while !app.quit && !done.load(Ordering::Relaxed) {
        app.poll();
        terminal.draw(|frame| draw(frame, &mut app))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(k) => app.key(k),
                Event::Resize(_, _) => {}
                _ => (),
            }
        }
        if let Some(action) = app.execute.take() {
            if !action.admin {
                app.read_action(action);
                continue;
            }
            let log = tempfile::NamedTempFile::new()?;
            guard.leave();
            println!(
                "Arch Forge · {}\n{}\n\n{}\n\nInteractive session — Ctrl+C cancels the command.\n",
                action.title,
                action.description,
                action.command()
            );
            let result = crate::cli::execute_reviewed(
                &action.program,
                &action.args,
                action.admin,
                Some(log.path()),
            );
            let mut bytes = vec![];
            let read = std::fs::File::open(log.path())?
                .take(5 * 1024 * 1024)
                .read_to_end(&mut bytes);
            let clean = backend::clean_output(&bytes);
            app.output = format!(
                "$ {}\n\n{}\n\n{}",
                action.command(),
                clean,
                match result {
                    Ok(status) =>
                        format!("Command finished: {status}. Review the log before retrying."),
                    Err(e) => e,
                }
            );
            if let Err(e) = read {
                app.output
                    .push_str(&format!("\nCould not read complete transcript: {e}"));
            }
            if bytes.len() >= 5 * 1024 * 1024 {
                app.output
                    .push_str("\n[Transcript display limited to 5 MiB.]");
            }
            app.updates = None;
            app.update_names.clear();
            app.output_open = true;
            app.output_title = action.title;
            app.output_scroll = 0;
            app.output_query.clear();
            app.status = "Session ended · output retained; / searches, e exports".into();
            app.rescan();
            guard.resume()?;
            terminal.clear()?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Colors {
    bg: Color,
    text: Color,
    muted: Color,
    border: Color,
    amber: Color,
    green: Color,
    red: Color,
    cyan: Color,
    purple: Color,
}
impl Colors {
    fn new(plain: bool) -> Self {
        if plain {
            return Self {
                bg: Color::Reset,
                text: Color::Reset,
                muted: Color::Reset,
                border: Color::Reset,
                amber: Color::Reset,
                green: Color::Reset,
                red: Color::Reset,
                cyan: Color::Reset,
                purple: Color::Reset,
            };
        }
        Self {
            bg: Color::Rgb(17, 23, 28),
            text: Color::Rgb(227, 232, 239),
            muted: Color::Rgb(157, 172, 188),
            border: Color::Rgb(75, 91, 105),
            amber: Color::Rgb(255, 190, 76),
            green: Color::Rgb(118, 205, 135),
            red: Color::Rgb(245, 116, 111),
            cyan: Color::Rgb(123, 195, 220),
            purple: Color::Rgb(190, 154, 229),
        }
    }
    fn focused(self) -> Style {
        if self.amber == Color::Reset {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else {
            Style::default()
                .fg(self.bg)
                .bg(self.amber)
                .add_modifier(Modifier::BOLD)
        }
    }
    fn border(self, title: &str, focus: bool) -> Block<'_> {
        Block::default()
            .padding(ratatui::widgets::Padding::horizontal(1))
            .borders(Borders::ALL)
            .title(format!(" {title} "))
            .border_style(Style::default().fg(if focus { self.amber } else { self.border }))
            .title_style(
                Style::default()
                    .fg(if focus { self.amber } else { self.muted })
                    .add_modifier(Modifier::BOLD),
            )
    }
}
fn command_line(command: &str, c: Colors) -> Line<'static> {
    let mut spans = vec![];
    for (i, word) in command.split_whitespace().enumerate() {
        let unquoted = word.trim_matches('\'');
        let color = if word == "sudo" {
            c.purple
        } else if unquoted.starts_with('-') {
            c.amber
        } else if i == 0 || word.starts_with('/') || i == 1 && command.starts_with("sudo ") {
            c.cyan
        } else {
            c.text
        };
        spans.push(Span::styled(format!("{word} "), Style::default().fg(color)));
    }
    Line::from(spans)
}
fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let c = Colors::new(app.no_color);
    frame.render_widget(
        Block::default().style(Style::default().bg(c.bg).fg(c.text)),
        area,
    );
    if area.width < 45 || area.height < 16 {
        frame.render_widget(Paragraph::new("Arch Forge\n\nTerminal too small. Resize to at least 45 columns × 16 rows.\n\nq: quit").wrap(Wrap{trim:false}),area);
        return;
    }
    let original_pane = app.pane;
    if app.dialog.is_some() {
        app.pane = Pane::Inactive;
    }
    let outer = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(area);
    let session = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Console / SSH".into());
    let mut header = vec![
        Span::styled(
            " //  ARCH FORGE ",
            Style::default().fg(c.amber).add_modifier(Modifier::BOLD),
        ),
        Span::styled("| Terminal workbench", Style::default().fg(c.muted)),
    ];
    if area.width >= 110 {
        header.push(Span::styled(
            format!("             Arch Linux · {session}  |  user session"),
            Style::default().fg(c.muted),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(header)).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(c.border)),
        ),
        outer[0],
    );
    let cols = Layout::horizontal([
        Constraint::Length(if area.width >= 100 {
            24
        } else if area.width >= 75 {
            22
        } else {
            16
        }),
        Constraint::Min(1),
    ])
    .spacing(1)
    .split(outer[1]);
    let nav = cols[0];
    let block = c.border("WORKSPACE", app.pane == Pane::Nav);
    let inner = block.inner(nav);
    frame.render_widget(block, nav);
    let navrows = Layout::vertical([
        Constraint::Length(8),
        Constraint::Min(0),
        Constraint::Length(if inner.height > 19 { 7 } else { 0 }),
    ])
    .split(inner);
    let items: PVec = PAGES
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let compact = [
                "Overview",
                "Packages",
                "Cleanup",
                "Diagnostics",
                "Build loadout",
                "Capture",
                "Import",
                "Wiki",
            ];
            let p = if nav.width < 22 { compact[i] } else { p };
            ListItem::new(format!(" {}  {p}", i + 1)).style(if app.page == i {
                Style::default().fg(c.amber).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(c.text)
            })
        })
        .collect();
    let mut state = ListState::default().with_selected(Some(if app.pane == Pane::Nav {
        app.nav
    } else {
        app.page
    }));
    frame.render_stateful_widget(
        List::new(items).highlight_style(if app.pane == Pane::Nav {
            c.focused()
        } else {
            Style::default().fg(c.amber).add_modifier(Modifier::BOLD)
        }),
        navrows[0],
        &mut state,
    );
    if navrows[2].height > 0 {
        frame.render_widget(
            Paragraph::new("SHORTCUTS\n\n/  Search\n?  Help\nq  Quit\nTab  Switch pane")
                .style(Style::default().fg(c.muted)),
            navrows[2],
        );
    }
    if app.output_open {
        draw_output(frame, cols[1], app, c);
    } else {
        draw_workspace(frame, cols[1], app, c);
    }
    let help = if app.dialog.is_some() {
        "Tab / Left / Right: focus  Space: toggle  Enter: confirm  Esc: cancel"
    } else if app.output_open {
        "↑↓ / j k: scroll  PgUp/PgDn  /: search  c: copy  e: export  Esc: back  q: quit"
    } else if app.package_page() {
        "↑↓ / j k: move  Tab: pane  Space: select  a: bulk  f: filter  i: install  d: remove  ?: help"
    } else {
        "↑↓ / j k: move  Enter: open  Tab: pane  /: search  o: log  r: rescan  ?: help  q: quit"
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(help, Style::default().fg(c.muted)),
            Line::styled(
                &app.status,
                Style::default().fg(if app.busy || app.scanning {
                    c.amber
                } else {
                    c.muted
                }),
            ),
        ])
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(c.border)),
        ),
        outer[2],
    );
    app.pane = original_pane;
    if app.dialog.is_some() {
        draw_dialog(frame, app, c);
    }
}
type PVec<'a> = Vec<ListItem<'a>>;
fn draw_workspace(frame: &mut Frame, area: Rect, app: &mut App, c: Colors) {
    let parts = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(if area.height >= 25 { 5 } else { 3 }),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                if app.page == 0 {
                    "System overview"
                } else {
                    PAGES[app.page]
                },
                Style::default().fg(c.amber).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if app.scanning {
                    "   Scanning…"
                } else if app.last_update.is_some() {
                    "   System scanned"
                } else {
                    "   Not checked"
                },
                Style::default().fg(c.muted),
            ),
        ])),
        parts[0],
    );
    if app.package_page() {
        draw_packages(frame, parts[1], app, c);
    } else if app.page == 0 {
        draw_overview(frame, parts[1], app, c);
    } else if app.page == 7 {
        draw_wiki(frame, parts[1], app, c);
    } else if app.page == 5 || app.page == 6 {
        let (caption, body) = if app.page == 5 {
            (
                "Capture this system",
                "Save explicitly installed package names to a desktop-compatible JSON profile.\n\nNo dotfiles, passwords, repository URLs or personal files are included. Foreign/local packages are marked for separate source review.\n\nKeep the exported profile on another drive. Existing files are never overwritten.\n\n[ Enter: choose a new output file ]",
            )
        } else {
            (
                "Import package profile",
                "Load a desktop or terminal JSON profile.\n\nRepository packages are placed in the selection for review. Already-installed packages are skipped. AUR/foreign entries and dotfiles are excluded.\n\nImporting never installs anything. Review the selection in Build loadout before applying.\n\n[ Enter: choose a profile file ]",
            )
        };
        let block = c.border(caption, app.pane == Pane::Items);
        let inner = block.inner(parts[1]);
        let lines = wrapped(body, inner.width as usize);
        let offset = app.pages[app.page]
            .offset
            .min(lines.len().saturating_sub(inner.height as usize));
        app.pages[app.page].offset = offset;
        frame.render_widget(
            Paragraph::new(lines.join("\n"))
                .scroll((offset as u16, 0))
                .block(block),
            parts[1],
        );
    } else {
        draw_actions(frame, parts[1], app, c);
    }
    let (action, extra) = if app.page == 7 {
        app.wiki_entries()
            .get(app.pages[7].cursor)
            .map(|i| {
                let e = &crate::wiki_data::ENTRIES[*i];
                (
                    e.command.to_owned(),
                    format!(
                        "{} · {} · Reference only; Enter opens details",
                        e.effect,
                        crate::wiki_data::TOPICS[e.topic].title
                    ),
                )
            })
            .unwrap_or_default()
    } else if let Some(a) = app.pending_action() {
        (a.command(), a.description)
    } else {
        (
            "Select an operation to preview its command.".into(),
            "Nothing has run.".into(),
        )
    };
    let mut lines = vec![command_line(&action, c)];
    if parts[2].height >= 5 {
        lines.push(Line::styled(extra, Style::default().fg(c.muted)));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(c.border("COMMAND PREVIEW · NOT RUN", false)),
        parts[2],
    );
}
fn draw_overview(frame: &mut Frame, area: Rect, app: &mut App, c: Colors) {
    let compact = area.width < 75 || area.height < 23;
    let rows = Layout::vertical([
        Constraint::Length(if compact { 6 } else { 7 }),
        Constraint::Length(if compact { 0 } else { 10 }),
        Constraint::Min(3),
    ])
    .spacing(1)
    .split(area);
    let meters = Layout::horizontal([Constraint::Ratio(1, 3); 3])
        .spacing(1)
        .split(rows[0]);
    let m = app.metrics.as_ref();
    let cpu = m.and_then(|m| m.cpu);
    let mem = m.filter(|m| m.total_memory > 0);
    let disk = m.filter(|m| m.total_disk > 0);
    let readings = [
        (
            "CPU",
            cpu.map(|v| format!("{v:.0}%"))
                .unwrap_or_else(|| "Unavailable".into()),
            cpu,
            m.and_then(|m| m.temperature)
                .map(|t| format!("Temp: {t:.0} °C"))
                .unwrap_or_else(|| "Temp: Unavailable".into()),
        ),
        (
            "MEMORY",
            mem.map(|m| {
                format!(
                    "{:.1} / {:.1} GiB",
                    m.used_memory as f64 / 1073741824.,
                    m.total_memory as f64 / 1073741824.
                )
            })
            .unwrap_or_else(|| "Unavailable".into()),
            mem.map(|m| m.memory),
            mem.map(|m| format!("{:.0}% used", m.memory))
                .unwrap_or_default(),
        ),
        (
            "ROOT STORAGE",
            disk.map(|m| {
                format!(
                    "{:.1} / {:.1} GiB",
                    m.used_disk as f64 / 1073741824.,
                    m.total_disk as f64 / 1073741824.
                )
            })
            .unwrap_or_else(|| "Unavailable".into()),
            disk.map(|m| 100. * m.used_disk as f64 / m.total_disk as f64),
            disk.map(|m| {
                format!(
                    "{} free",
                    crate::system::bytes(m.total_disk.saturating_sub(m.used_disk))
                )
            })
            .unwrap_or_default(),
        ),
    ];
    for (i, (title, value, ratio, detail)) in readings.into_iter().enumerate() {
        let b = c.border(title, false);
        let inner = b.inner(meters[i]);
        frame.render_widget(b, meters[i]);
        let row = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);
        frame.render_widget(
            Paragraph::new(value).style(
                Style::default()
                    .fg(if i == 2 { c.amber } else { c.green })
                    .add_modifier(Modifier::BOLD),
            ),
            row[0],
        );
        frame.render_widget(
            Paragraph::new(detail).style(Style::default().fg(c.muted)),
            row[1],
        );
        if let Some(r) = ratio {
            frame.render_widget(
                Gauge::default()
                    .ratio((r / 100.).clamp(0., 1.))
                    .label(format!("{r:.0}%"))
                    .gauge_style(
                        Style::default()
                            .fg(if i == 2 { c.amber } else { c.green })
                            .bg(c.bg),
                    ),
                row[2],
            );
        }
    }
    if !compact {
        let cols = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
            .spacing(1)
            .split(rows[1]);
        let inv = &app.scan.inventory;
        let session =
            std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "Console / SSH".into());
        let unknown = |s: &str| {
            if s.is_empty() {
                "Unavailable".to_owned()
            } else {
                s.to_owned()
            }
        };
        let details = format!(
            "Processor  {}\nGraphics   {}\nKernel     {}\nSession    {}\nUptime     {}\nPackages   {}",
            unknown(&inv.cpu),
            unknown(&inv.gpu),
            unknown(&inv.kernel),
            session,
            m.map(|m| format!("{}h {}m", m.uptime / 3600, m.uptime % 3600 / 60))
                .unwrap_or_else(|| "Unavailable".into()),
            if inv.error.is_some() {
                "Unavailable".into()
            } else if app.scanning && app.scan.installed.is_empty() {
                "Scanning…".into()
            } else {
                app.scan.installed.len().to_string()
            }
        );
        frame.render_widget(
            Paragraph::new(details)
                .wrap(Wrap { trim: false })
                .block(c.border("SYSTEM DETAILS", false)),
            cols[0],
        );
        let failed = inv
            .failed_services
            .map(|n| format!("{n} failed system service{}", if n == 1 { "" } else { "s" }))
            .unwrap_or_else(|| "Service status: Unavailable".into());
        let updates = app
            .updates
            .map(|n| format!("{n} repository updates"))
            .unwrap_or_else(|| "Updates: Not checked".into());
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    failed,
                    Style::default().fg(if inv.failed_services.is_some_and(|n| n > 0) {
                        c.red
                    } else {
                        c.muted
                    }),
                ),
                Line::raw(""),
                Line::raw(updates),
                Line::raw(""),
                Line::styled(
                    "Choose a quick action below.\nAUR updates are not checked.",
                    Style::default().fg(c.muted),
                ),
            ])
            .wrap(Wrap { trim: false })
            .block(c.border("ATTENTION", false)),
            cols[1],
        );
    }
    draw_actions(frame, rows[2], app, c);
}
fn draw_actions(frame: &mut Frame, area: Rect, app: &mut App, c: Colors) {
    let actions = app.actions();
    let items: PVec = actions
        .iter()
        .map(|a| {
            ListItem::new(vec![
                Line::raw(a.title.clone()),
                Line::styled(
                    format!(
                        "{} · {}",
                        if a.admin {
                            "Requires review + authentication"
                        } else {
                            "Read-only"
                        },
                        a.description
                    ),
                    Style::default().fg(c.muted),
                ),
            ])
        })
        .collect();
    let st = &mut app.pages[app.page];
    let mut state = ListState::default()
        .with_offset(st.offset)
        .with_selected((!items.is_empty()).then_some(st.cursor.min(items.len().saturating_sub(1))));
    frame.render_stateful_widget(
        List::new(items)
            .block(c.border(
                if app.page == 0 {
                    "QUICK ACTIONS"
                } else {
                    "OPERATIONS"
                },
                app.pane == Pane::Items,
            ))
            .highlight_style(if app.pane == Pane::Items {
                c.focused()
            } else {
                Style::default().add_modifier(Modifier::BOLD)
            }),
        area,
        &mut state,
    );
    st.offset = state.offset();
}
fn draw_packages(frame: &mut Frame, area: Rect, app: &mut App, c: Colors) {
    let wide = area.width >= 100;
    let cols = if wide {
        Layout::horizontal([
            Constraint::Length(20),
            Constraint::Min(25),
            Constraint::Length(27),
        ])
        .spacing(1)
        .split(area)
    } else {
        Layout::horizontal([
            Constraint::Length(if app.pane == Pane::Categories { 18 } else { 0 }),
            Constraint::Min(15),
            Constraint::Length(0),
        ])
        .split(area)
    };
    if cols[0].width > 0 {
        let categories = app.categories();
        let mut state = ListState::default().with_selected(Some(app.pages[app.page].category));
        frame.render_stateful_widget(
            List::new(
                categories
                    .iter()
                    .map(|s| ListItem::new(s.clone()))
                    .collect::<PVec>(),
            )
            .block(c.border("CATEGORIES", app.pane == Pane::Categories))
            .highlight_style(if app.pane == Pane::Categories {
                c.focused()
            } else {
                Style::default().fg(c.amber)
            }),
            cols[0],
            &mut state,
        );
    }
    let view = app.visible_packages();
    let s = &app.pages[app.page];
    let filters = ["All statuses", "Installed", "Not installed", "Updates"];
    let listparts = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(if area.height >= 16 { 3 } else { 0 }),
        Constraint::Length(2),
    ])
    .split(cols[1]);
    let cat = app
        .categories()
        .get(s.category)
        .cloned()
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(format!(
            "/ {} | {} | {}\n{} matches · {} selected · {} · {}{}",
            if s.query.is_empty() {
                "Search packages"
            } else {
                &s.query
            },
            cat,
            filters[s.filter],
            view.len(),
            app.selected.len(),
            SOURCES[s.source_filter],
            SORTS[s.sort],
            if s.filter == 3 && app.updates.is_none() {
                " · Updates not checked"
            } else {
                ""
            }
        ))
        .style(Style::default().fg(c.muted))
        .wrap(Wrap { trim: false }),
        listparts[0],
    );
    let viewport = listparts[1].height.saturating_sub(2).max(1) as usize;
    let (cursor, offset) = {
        let st = &mut app.pages[app.page];
        st.cursor = st.cursor.min(view.len().saturating_sub(1));
        if st.cursor < st.offset {
            st.offset = st.cursor;
        }
        if st.cursor >= st.offset + viewport {
            st.offset = st.cursor + 1 - viewport;
        }
        st.offset = st.offset.min(view.len().saturating_sub(viewport));
        (st.cursor, st.offset)
    };
    let items: PVec = view
        .iter()
        .skip(offset)
        .take(viewport)
        .map(|i| {
            let p = &app.source()[*i];
            let name = app.installed_name(&p.name);
            let status = if app.scan.inventory.error.is_some() {
                "Unavailable"
            } else if app.scanning && app.scan.installed.is_empty() {
                "Scanning"
            } else if app.update_names.contains(name) {
                "Update"
            } else if app.scan.installed.contains_key(name) {
                "Installed"
            } else {
                "Missing"
            };
            let color = if status == "Installed" {
                c.green
            } else if status == "Update" {
                c.amber
            } else {
                c.muted
            };
            let mut spans = vec![Span::styled(
                if app.selected.contains(name) {
                    "[x] "
                } else {
                    "[ ] "
                },
                Style::default().fg(c.amber),
            )];
            if cols[1].width >= 64 {
                spans.push(Span::raw(format!(
                    "{:<29}",
                    name.chars().take(28).collect::<String>()
                )));
                spans.push(Span::styled(
                    format!("{status:<12}"),
                    Style::default().fg(color),
                ));
                let source = app.package_source(p);
                spans.push(Span::styled(source, Style::default().fg(c.muted)));
            } else {
                spans.push(Span::raw(format!(
                    "{:<21}",
                    name.chars().take(20).collect::<String>()
                )));
                spans.push(Span::styled(status, Style::default().fg(color)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let mut state = ListState::default()
        .with_selected((!view.is_empty()).then_some(cursor.saturating_sub(offset)));
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.scanning {
                "Scanning packages… You can keep navigating."
            } else {
                "No matching packages. R resets search, category, status and source filters."
            })
            .wrap(Wrap { trim: false })
            .block(c.border("PACKAGES", app.pane == Pane::Items)),
            listparts[1],
        );
    } else {
        frame.render_stateful_widget(
            List::new(items)
                .block(c.border(
                    if cols[1].width >= 64 {
                        "PACKAGE                         STATUS      SOURCE"
                    } else {
                        "PACKAGES · STATUS"
                    },
                    app.pane == Pane::Items,
                ))
                .highlight_style(if app.pane == Pane::Items {
                    c.focused()
                } else {
                    Style::default().add_modifier(Modifier::BOLD)
                }),
            listparts[1],
            &mut state,
        );
    }
    if listparts[2].height > 0
        && let Some(index) = view.get(cursor)
    {
        let package = &app.source()[*index];
        let actual = app.installed_name(&package.name);
        let version = app
            .scan
            .installed
            .get(actual)
            .map(String::as_str)
            .unwrap_or("Not installed");
        frame.render_widget(
            Paragraph::new(format!(
                "{actual} · {version} · {}\n{}",
                app.package_source(package),
                package.description
            ))
            .style(Style::default().fg(c.muted))
            .wrap(Wrap { trim: false }),
            listparts[2],
        );
    }
    frame.render_widget(Paragraph::new("f status · t source · S sort · R reset filters\nSpace select · a bulk · i install · d remove · s save").style(Style::default().fg(c.muted)),listparts[3]);
    if wide {
        draw_selection(frame, cols[2], app, c);
    } else if app.pane == Pane::Selection {
        let popup = Rect::new(cols[1].x, cols[1].y, cols[1].width, cols[1].height);
        frame.render_widget(Clear, popup);
        draw_selection(frame, popup, app, c);
    }
}
fn draw_selection(frame: &mut Frame, area: Rect, app: &App, c: Colors) {
    let installed = app
        .selected
        .iter()
        .filter(|n| app.scan.installed.contains_key(app.installed_name(n)))
        .count();
    let missing = app.selected.len() - installed;
    let mut lines = vec![
        Line::styled(
            format!("{} selected", app.selected.len()),
            Style::default().fg(c.amber),
        ),
        Line::raw(format!("To install: {missing}")),
        Line::raw(format!("Already installed: {installed}")),
        Line::raw("Installed entries are skipped."),
        Line::raw(""),
    ];
    for n in app
        .selected
        .iter()
        .skip(app.summary_scroll)
        .take(area.height.saturating_sub(12) as usize)
    {
        lines.push(Line::raw(n.clone()));
    }
    if app.selected.len() > area.height.saturating_sub(12) as usize {
        lines.push(Line::styled(
            "… more selected packages",
            Style::default().fg(c.muted),
        ));
    }
    lines.extend([
        Line::raw(""),
        Line::styled(
            "[ Review & install ]",
            if app.pane == Pane::Selection {
                c.focused()
            } else {
                Style::default().fg(c.amber)
            },
        ),
        Line::raw("i: review  d: uninstall"),
        Line::raw("s: save  x: clear"),
    ]);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(c.border("YOUR LOADOUT", app.pane == Pane::Selection)),
        area,
    );
}
fn draw_wiki(frame: &mut Frame, area: Rect, app: &mut App, c: Colors) {
    let cols = Layout::horizontal([
        Constraint::Percentage(if area.width >= 85 { 48 } else { 100 }),
        Constraint::Min(0),
    ])
    .spacing(1)
    .split(area);
    let indices = app.wiki_entries();
    let items: PVec = indices
        .iter()
        .map(|i| {
            let e = &crate::wiki_data::ENTRIES[*i];
            ListItem::new(vec![
                Line::raw(e.title),
                Line::styled(
                    crate::wiki_data::TOPICS[e.topic].title,
                    Style::default().fg(c.muted),
                ),
            ])
        })
        .collect();
    let st = &mut app.pages[7];
    st.cursor = st.cursor.min(indices.len().saturating_sub(1));
    let mut state = ListState::default()
        .with_offset(st.offset)
        .with_selected((!indices.is_empty()).then_some(st.cursor));
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new("No commands match. / clears or changes the search.")
                .wrap(Wrap { trim: false })
                .block(c.border("WIKI", app.pane == Pane::Items)),
            cols[0],
        );
    } else {
        frame.render_stateful_widget(
            List::new(items)
                .block(c.border("COMMAND REFERENCE · / SEARCH", app.pane == Pane::Items))
                .highlight_style(if app.pane == Pane::Items {
                    c.focused()
                } else {
                    Style::default()
                }),
            cols[0],
            &mut state,
        );
    }
    st.offset = state.offset();
    if cols[1].width > 0
        && let Some(i) = indices.get(st.cursor)
    {
        let e = &crate::wiki_data::ENTRIES[*i];
        let t = &crate::wiki_data::TOPICS[e.topic];
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    e.title,
                    Style::default().fg(c.amber).add_modifier(Modifier::BOLD),
                ),
                Line::raw(""),
                Line::raw(e.description),
                Line::raw(""),
                Line::raw(e.effect),
                Line::raw(""),
                Line::styled(t.tip, Style::default().fg(c.muted)),
                Line::raw(""),
                Line::styled(t.url, Style::default().fg(c.cyan)),
                Line::raw(""),
                Line::raw(
                    "Enter: open full, scrollable reference. Nothing executes from the Wiki.",
                ),
            ])
            .wrap(Wrap { trim: false })
            .block(c.border("GUIDE", false)),
            cols[1],
        );
    }
}
fn draw_output(frame: &mut Frame, area: Rect, app: &mut App, c: Colors) {
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(format!(
            "{} · / {}",
            app.output_title,
            if app.output_query.is_empty() {
                "Find in output"
            } else {
                &app.output_query
            }
        ))
        .style(Style::default().fg(c.amber).add_modifier(Modifier::BOLD)),
        rows[0],
    );
    app.output_height = rows[1].height.saturating_sub(2) as usize;
    let matching = app.output_lines();
    let max = matching
        .len()
        .saturating_sub(rows[1].height.saturating_sub(2) as usize);
    let scroll = app.output_scroll.min(max);
    let mut lines = vec![];
    for (index, line) in matching
        .iter()
        .skip(scroll)
        .take(rows[1].height.saturating_sub(2) as usize)
    {
        let lower = line.to_lowercase();
        let color = if lower.contains("error") || lower.contains("failed") {
            c.red
        } else if lower.contains("warning") {
            c.amber
        } else {
            c.text
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{:>5}  ", index + 1), Style::default().fg(c.muted)),
            Span::styled((*line).to_owned(), Style::default().fg(color)),
        ]));
    }
    if app.busy && app.output.is_empty() {
        lines.push(Line::raw(
            "Running inspection… Navigation remains available; o returns to the workspace.",
        ));
    } else if matching.is_empty() {
        lines.push(Line::raw(if app.output.is_empty() {
            "No command output yet."
        } else {
            "No matching lines. / changes or clears the search."
        }));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .scroll((0, app.output_horizontal))
            .block(c.border("COMMAND OUTPUT", app.pane != Pane::Nav)),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "{} matching lines · row {} · PgUp/PgDn scroll · e export · c terminal clipboard",
            matching.len(),
            scroll + 1
        ))
        .style(Style::default().fg(c.muted)),
        rows[2],
    );
}
fn wrapped(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = vec![];
    for source in text.split('\n') {
        if source.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in source.split_whitespace() {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
                lines.push(std::mem::take(&mut line));
            }
            for ch in word.chars() {
                if line.chars().count() >= width {
                    lines.push(std::mem::take(&mut line));
                }
                line.push(ch);
            }
            line.push(' ');
        }
        lines.push(line.trim_end().to_owned());
    }
    lines
}
fn review_lines(action: &Action, width: usize, c: Colors) -> Vec<Line<'static>> {
    let mut lines: Vec<_> = wrapped(&action.description, width)
        .into_iter()
        .map(Line::raw)
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        if action.admin {
            "Fresh administrator authentication required."
        } else {
            "Read-only inspection; no administrator authentication."
        },
        Style::default().fg(c.amber),
    ));
    lines.push(Line::raw(""));
    lines.push(Line::styled("COMMAND TO RUN", Style::default().fg(c.muted)));
    lines.extend(
        wrapped(&action.command(), width)
            .into_iter()
            .map(|s| command_line(&s, c)),
    );
    lines.push(Line::raw(""));
    lines.extend(wrapped("Interactive operations pause the TUI and use a real terminal. Sudo handles passwords. Only terminal output is recorded; input streams are never logged.",width).into_iter().map(|s|Line::styled(s,Style::default().fg(c.muted))));
    lines
}
fn draw_dialog(frame: &mut Frame, app: &mut App, c: Colors) {
    let whole = frame.area();
    let width = whole.width.saturating_sub(4).min(100);
    let height = match app.dialog {
        Some(Dialog::Help(_)) => 24,
        Some(Dialog::Review { .. }) => 20,
        _ => 8,
    }
    .min(whole.height.saturating_sub(2));
    let area = Rect::new(
        (whole.width - width) / 2,
        (whole.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default().style(Style::default().bg(c.bg).fg(c.text)),
        area,
    );
    match app.dialog.as_mut().unwrap() {
        Dialog::Help(scroll) => {
            let block = c.border("HELP · ↑↓ scroll · Esc closes", true);
            let inner = block.inner(area);
            let lines = wrapped(
                "Navigation\nArrow keys or j/k: move · Enter: open · Esc: back\nTab / Shift+Tab: pane · 1–8: workspace\nPgUp / PgDn / Home / End: scroll\n\nPackages / loadouts\nSpace: toggle · a: select/clear matches\nf: status · t: source · /: search · +: exact names\nS: sort · R: reset filters (keeps selection)\ni: review install · d: review removal\ns: export selection · x: clear selection\n\nLogs\no: open/close · /: search · Left/Right: pan\nc: terminal clipboard · e: export\n\nReview\nUp/Down: scroll details · Tab: focus\nSpace: review checkbox · Enter: execute\nAuthentication uses a real terminal.\n\nr: rescan · u: check updates · U: full upgrade · q: quit",
                inner.width as usize,
            );
            *scroll = (*scroll).min(lines.len().saturating_sub(inner.height as usize));
            frame.render_widget(
                Paragraph::new(lines.join("\n"))
                    .scroll(((*scroll).min(u16::MAX as usize) as u16, 0))
                    .block(block),
                area,
            );
        }
        Dialog::Input(kind, text) => {
            let title = match kind {
                InputKind::Search => "Search workspace",
                InputKind::OutputSearch => "Find in output",
                InputKind::Capture => "Capture · new JSON filename",
                InputKind::Import => "Import JSON profile · filename",
                InputKind::Export => "Export · new filename",
                InputKind::Custom => "Add exact package names",
            };
            let block = c.border(title, true);
            let inner = block.inner(area);
            frame.render_widget(block, area);
            let tail: String = text
                .chars()
                .rev()
                .take(inner.width.saturating_sub(1) as usize)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            frame.render_widget(
                Paragraph::new(format!(
                    "{tail}\n\nEnter: apply · Esc: cancel\nFiles are never overwritten."
                ))
                .wrap(Wrap { trim: false }),
                inner,
            );
            frame.set_cursor_position((inner.x + tail.chars().count() as u16, inner.y));
        }
        Dialog::Review {
            action,
            reviewed,
            focus,
            scroll,
        } => {
            let block = c.border("REVIEW · NOTHING HAS RUN", true);
            let inner = block.inner(area);
            frame.render_widget(block, area);
            let rows = Layout::vertical([
                Constraint::Length(2),
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(2),
            ])
            .split(inner);
            frame.render_widget(
                Paragraph::new(action.title.clone())
                    .style(Style::default().fg(c.amber).add_modifier(Modifier::BOLD))
                    .wrap(Wrap { trim: false }),
                rows[0],
            );
            let lines = review_lines(action, inner.width as usize, c);
            let offset = (*scroll).min(lines.len().saturating_sub(rows[1].height as usize));
            frame.render_widget(
                Paragraph::new(lines).scroll((offset.min(u16::MAX as usize) as u16, 0)),
                rows[1],
            );
            frame.render_widget(
                Paragraph::new(
                    "↑↓ / PgUp/PgDn scroll details · Tab switches controls · Esc cancels",
                )
                .style(Style::default().fg(c.muted)),
                rows[2],
            );
            let controls =
                Layout::horizontal([Constraint::Percentage(53), Constraint::Percentage(47)])
                    .split(rows[3]);
            frame.render_widget(
                Paragraph::new(format!(
                    "{} I reviewed the command",
                    if *reviewed { "[x]" } else { "[ ]" }
                ))
                .wrap(Wrap { trim: false })
                .style(if *focus == 0 {
                    c.focused()
                } else {
                    Style::default().fg(c.text)
                }),
                controls[0],
            );
            frame.render_widget(
                Paragraph::new(if *reviewed {
                    if action.admin {
                        "[ Authenticate & run ]"
                    } else {
                        "[ Run inspection ]"
                    }
                } else {
                    "[ Review to enable ]"
                })
                .wrap(Wrap { trim: false })
                .style(if *focus == 1 && *reviewed {
                    c.focused()
                } else {
                    Style::default().fg(c.muted)
                }),
                controls[1],
            );
        }
    }
}

fn copy(text: &str) -> Result<String, String> {
    if std::env::var("TERM").ok().as_deref() == Some("linux") {
        return Err("Linux console has no OSC 52 clipboard. Use e to export.".into());
    }
    if text.len() > 1024 * 1024 {
        return Err("Output exceeds the clipboard limit. Use e to export the full log.".into());
    }
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in text.as_bytes().chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    write!(io::stdout(), "\x1b]52;c;{out}\x07")
        .and_then(|()| io::stdout().flush())
        .map_err(|e| e.to_string())?;
    Ok(
        "Clipboard request sent. Emulator support/permission is required; e exports to a file."
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }
    fn fixture() -> App {
        let mut app = App::new();
        app.scanning = false;
        app.scan.inventory.cpu = "Test processor".into();
        app.scan.inventory.gpu = "Test graphics adapter".into();
        app.scan.inventory.kernel = "test-kernel".into();
        app.scan.inventory.failed_services = Some(1);
        app.scan.installed.insert("git".into(), "1.0".into());
        app.scan.installed.insert("lutris-git".into(), "1.0".into());
        for i in 0..100 {
            app.scan.available.insert(format!("test-package-{i:03}"));
        }
        app.scan.available.insert("git".into());
        app.refresh_catalog();
        app
    }
    #[test]
    fn package_variants_inherit_categories_and_inspect_actual_names() {
        let mut app = fixture();
        app.scan
            .installed
            .insert("firefox-bin".into(), "1.0".into());
        app.scan
            .installed
            .insert("obs-studio-git".into(), "1.0".into());
        app.refresh_catalog();
        assert_eq!(
            app.packages
                .iter()
                .find(|p| p.name == "firefox-bin")
                .unwrap()
                .category,
            "Browsers"
        );
        assert_eq!(
            app.packages
                .iter()
                .find(|p| p.name == "obs-studio-git")
                .unwrap()
                .category,
            "Media"
        );
        app.page = 1;
        app.pages[1].query = "firefox".into();
        let action = app.pending_action().unwrap();
        assert!(action.args.contains(&"-Qi".into()));
        assert!(action.args.contains(&"firefox-bin".into()));
    }
    #[test]
    fn navigation_bulk_selection_review_and_pane_scroll_are_preserved() {
        let mut a = fixture();
        a.key(key(KeyCode::Char('2')));
        assert_eq!(a.page, 1);
        a.key(key(KeyCode::Char('a')));
        assert_eq!(a.selected.len(), a.visible_packages().len());
        a.selected.remove("lutris-git");
        a.key(key(KeyCode::Char('i')));
        assert!(a.execute.is_none());
        assert!(matches!(
            a.dialog,
            Some(Dialog::Review {
                reviewed: false,
                ..
            })
        ));
        // Enter on the unchecked run control cannot execute.
        a.key(key(KeyCode::Tab));
        a.key(key(KeyCode::Enter));
        assert!(a.execute.is_none());
        a.key(key(KeyCode::BackTab));
        a.key(key(KeyCode::Char(' ')));
        a.key(key(KeyCode::Tab));
        a.key(key(KeyCode::Enter));
        let plan = a.execute.take().unwrap();
        assert!(plan.admin);
        assert!(plan.args.contains(&"-Syu".into()));
        assert!(!plan.args.contains(&"git".into()));
        a.key(key(KeyCode::End));
        let cursor = a.pages[1].cursor;
        let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
        terminal.draw(|f| draw(f, &mut a)).unwrap();
        let offset = a.pages[1].offset;
        assert!(
            offset > 0,
            "large package lists must scroll within the pane"
        );
        a.key(key(KeyCode::Char('8')));
        a.key(key(KeyCode::Char('2')));
        assert_eq!(a.pages[1].cursor, cursor);
        terminal.draw(|f| draw(f, &mut a)).unwrap();
        assert_eq!(a.pages[1].offset, offset);
        assert!(!a.selected.is_empty());
        a.pane = Pane::Selection;
        a.move_cursor(5);
        assert_eq!(a.summary_scroll, 5);
        a.pane = Pane::Categories;
        a.move_cursor(1);
        assert_eq!(a.pages[1].category, 1);
        assert_eq!(a.pages[1].cursor, 0);
        a.key(key(KeyCode::Char('f')));
        assert_eq!(a.pages[1].filter, 1);
    }
    #[test]
    fn all_screens_resize_and_show_loading_empty_failure_and_review_states() {
        let mut a = fixture();
        for (w, h) in [(160, 50), (100, 32), (80, 24), (60, 20), (45, 16), (30, 10)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            for page in 0..8 {
                a.page = page;
                a.nav = page;
                terminal.draw(|f| draw(f, &mut a)).unwrap();
            }
            a.page = 2;
            a.review(backend::cleanup()[1].clone());
            terminal.draw(|f| draw(f, &mut a)).unwrap();
            a.dialog = None;
            a.output_open = true;
            a.busy = true;
            a.output.clear();
            terminal.draw(|f| draw(f, &mut a)).unwrap();
            a.busy = false;
            a.output = "error: mock command failed\nNo packages were changed.".into();
            terminal.draw(|f| draw(f, &mut a)).unwrap();
            a.output_open = false;
            a.page = 1;
            a.pages[1].query = "no-matching-package".into();
            terminal.draw(|f| draw(f, &mut a)).unwrap();
            a.pages[1].query.clear();
        }
    }
    #[test]
    fn output_search_scroll_and_no_color_are_terminal_safe() {
        let mut a = fixture();
        a.output = (0..200)
            .map(|i| format!("line {i}: {}", if i % 10 == 0 { "warning" } else { "ok" }))
            .collect::<Vec<_>>()
            .join("\n");
        a.output_open = true;
        a.output_height = 10;
        a.move_cursor(100000);
        assert_eq!(a.output_scroll, 190);
        a.move_cursor(-10);
        assert_eq!(a.output_scroll, 180);
        a.submit_input(InputKind::OutputSearch, "warning".into());
        assert_eq!(a.output_lines().len(), 20);
        assert_eq!(a.output_scroll, 0);
        a.key(key(KeyCode::Right));
        assert_eq!(a.output_horizontal, 8);
        a.no_color = true;
        let mut t = Terminal::new(TestBackend::new(120, 35)).unwrap();
        t.draw(|f| draw(f, &mut a)).unwrap();
        for cell in t.backend().buffer().content() {
            assert_eq!(cell.fg, Color::Reset);
            assert_eq!(cell.bg, Color::Reset);
        }
        assert_eq!(
            backend::clean_output(b"\x1b[31merror\x1b[0m\x1b]52;c;secret\x07\n"),
            "error\n"
        );
    }
    #[test]
    fn variants_filter_by_actual_source_and_export_actual_identity() {
        let mut app = fixture();
        app.scan.available.insert("lutris".into());
        app.build.push(catalog_package("lutris", "repo", ""));
        app.selected = BTreeSet::from(["lutris".into(), "lutris-git".into(), "git".into()]);
        app.refresh_catalog();
        assert_eq!(
            app.selected,
            BTreeSet::from(["lutris-git".into(), "git".into()])
        );
        assert_eq!(
            app.packages
                .iter()
                .filter(|p| p.name.starts_with("lutris"))
                .count(),
            1
        );
        app.page = 4;
        app.pages[4].query = "lutris-git".into();
        app.pages[4].source_filter = 2;
        let visible = app.visible_packages();
        assert_eq!(visible.len(), 1);
        assert_eq!(app.package_source(&app.source()[visible[0]]), "foreign");
        let mut terminal = Terminal::new(TestBackend::new(160, 50)).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("lutris-git · 1.0 · foreign"));
        app.pages[4].source_filter = 1;
        assert!(app.visible_packages().is_empty());
        app.pages[4].source_filter = 3;
        assert!(app.visible_packages().is_empty());
        let profile: serde_json::Value = serde_json::from_str(
            &backend::selection_profile(
                &BTreeSet::from(["lutris".into(), "lutris-git".into(), "git".into()]),
                &app.scan,
            )
            .unwrap(),
        )
        .unwrap();
        let packages = profile["packages"].as_array().unwrap();
        assert_eq!(packages.len(), 2);
        assert!(
            packages
                .iter()
                .any(|p| p["name"] == "lutris-git" && p["source"] == "foreign")
        );
        assert!(
            packages
                .iter()
                .any(|p| p["name"] == "git" && p["source"] == "repo")
        );
        app.scan.repository_error = Some("offline database".into());
        let actual = app.build.iter().find(|p| p.name == "lutris").unwrap();
        assert_eq!(app.package_source(actual), "unknown");
        assert!(
            backend::selection_profile(&BTreeSet::from(["bad;name".into()]), &app.scan).is_err()
        );
    }
    #[test]
    fn sorting_and_reset_are_per_page_and_preserve_selection() {
        let mut app = fixture();
        app.page = 1;
        app.key(key(KeyCode::Char('S')));
        let names = |app: &App| {
            app.visible_packages()
                .iter()
                .map(|i| app.source()[*i].name.clone())
                .collect::<Vec<_>>()
        };
        assert!(names(&app).windows(2).all(|p| p[0] <= p[1]));
        app.key(key(KeyCode::Char('S')));
        assert!(names(&app).windows(2).all(|p| p[0] >= p[1]));
        app.selected.insert("git".into());
        app.pages[1].query = "does-not-exist".into();
        app.pages[1].category = 1;
        app.pages[1].filter = 2;
        app.pages[1].source_filter = 2;
        assert!(app.visible_packages().is_empty());
        app.key(key(KeyCode::Char('R')));
        assert!(!app.visible_packages().is_empty());
        assert_eq!(app.pages[1].category, 0);
        assert_eq!(app.pages[1].filter, 0);
        assert_eq!(app.pages[1].source_filter, 0);
        assert_eq!(app.pages[1].sort, 2);
        assert!(app.selected.contains("git"));
        app.page = 4;
        assert_eq!(app.pages[4].sort, 0);
    }
    #[test]
    fn rescan_preserves_category_identity_and_new_rows_reuse_metadata() {
        let mut app = fixture();
        app.page = 1;
        app.pages[1].category = app.categories().iter().position(|c| c == "Gaming").unwrap();
        app.scan
            .installed
            .insert("firefox-bin".into(), "1.0".into());
        app.refresh_catalog();
        assert_eq!(app.categories()[app.pages[1].category], "Gaming");
        app.page = 4;
        app.submit_input(InputKind::Custom, "obs-studio-git".into());
        let package = app
            .build
            .iter()
            .find(|p| p.name == "obs-studio-git")
            .unwrap();
        assert_eq!(package.category, "Media");
        assert_eq!(package.title, "OBS Studio");
        app.scan.installed.remove("lutris-git");
        app.refresh_catalog();
        app.page = 1;
        assert_eq!(app.pages[1].category, 0);
    }
    #[test]
    fn rescan_drops_update_flags_for_replaced_or_removed_versions() {
        let mut app = fixture();
        app.update_names = BTreeSet::from(["git".into(), "lutris-git".into()]);
        app.updates = Some(2);
        let mut scan = Scan::default();
        scan.installed.insert("git".into(), "2.0".into());
        scan.installed.insert("lutris-git".into(), "1.0".into());
        app.tx.send(Message::Scan(Box::new(scan))).unwrap();
        app.poll();
        assert_eq!(app.update_names, BTreeSet::from(["lutris-git".into()]));
        assert_eq!(app.updates, Some(1));
    }
    #[test]
    fn planning_never_runs_commands_and_rejects_unknown_targets() {
        let scan = fixture().scan;
        let selected = BTreeSet::from(["test-package-001".into(), "git".into()]);
        let plan = backend::install_plan(&selected, &scan).unwrap();
        assert!(!plan.args.contains(&"git".into()));
        assert!(backend::install_plan(&BTreeSet::from(["--noconfirm".into()]), &scan).is_err());
        assert!(backend::removal(&BTreeSet::from(["test-package-001".into()]), &scan).is_err());
        let remove = backend::removal(&BTreeSet::from(["lutris".into()]), &scan).unwrap();
        assert_eq!(remove.args, vec!["-R", "--", "lutris-git"]);
    }
}

#[cfg(test)]
mod live_render {
    use super::*;
    #[test]
    #[ignore = "Local read-only visual verification; writes only /tmp artifacts"]
    fn snapshot_actual_readings() {
        let mut app = App::new();
        app.scan = backend::scan();
        app.scanning = false;
        app.refresh_catalog();
        let mut sampler = crate::system::Sampler::default();
        sampler.sample();
        std::thread::sleep(Duration::from_millis(150));
        app.metrics = Some(sampler.sample());
        app.status = "Ready · local visual verification".into();
        app.no_color = false;
        app.last_update = Some(Instant::now());
        for (width, height, page, name) in [
            (160, 50, 0, "overview"),
            (160, 50, 4, "loadout"),
            (80, 24, 0, "compact"),
            (80, 24, 4, "compact-loadout"),
            (160, 50, 7, "wiki"),
        ] {
            app.page = page;
            app.nav = page;
            app.pane = Pane::Items;
            let mut t = Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            t.draw(|f| draw(f, &mut app)).unwrap();
            let cells:Vec<_>=t.backend().buffer().content().iter().map(|cell|serde_json::json!({"text":cell.symbol(),"fg":format!("{:?}",cell.fg),"bg":format!("{:?}",cell.bg),"style":format!("{:?}",cell.modifier)})).collect();
            std::fs::write(
                format!("/tmp/archforge-tui-{name}.json"),
                serde_json::json!({"width":width,"height":height,"cells":cells}).to_string(),
            )
            .unwrap();
        }
    }
}
