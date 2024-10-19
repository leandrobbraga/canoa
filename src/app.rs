use crate::config::Config;
use crate::jira::{Issue, Jira, Sprint};
use crate::tui::{self, Color, Terminal, Widget};

pub struct App {
    pub state: AppState,
    pub ui: Ui,
}

pub struct AppState {
    pub active_window: Window,

    active_sprint: usize,
    active_issue: usize,

    sprints: Vec<Sprint>,
    issues: Vec<Vec<Issue>>,
}

impl AppState {
    fn get(jira: &Jira, board_id: &str) -> AppState {
        let (sprints, issues) = std::thread::scope(|scope| {
            let backlog = scope.spawn(|| jira.get_backlog_issues(board_id));
            let mut sprints = jira.get_board_active_and_future_sprints(board_id);

            let mut handles = Vec::with_capacity(sprints.len());

            for sprint in &sprints {
                let id = sprint.id;
                let handle = scope.spawn(move || jira.get_sprint_issues(board_id, id));
                handles.push(handle);
            }

            sprints.push(Sprint {
                id: 0,
                name: "Backlog".into(),
            });

            handles.push(backlog);

            let issues = handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect();

            (sprints, issues)
        });

        AppState {
            active_sprint: 0,
            active_issue: 0,
            active_window: Window::Sprints,
            sprints,
            issues,
        }
    }
}

impl App {
    pub fn from_config(config: Config) -> Self {
        let Config {
            user,
            token,
            board_id,
            host,
        } = config;

        let jira = Jira::new(&user, &token, host);
        let state = AppState::get(&jira, &board_id);
        let terminal = Terminal::try_new().unwrap();
        let ui = Ui::new(terminal, &state);

        let mut app = App { state, ui };
        app.ui_initial_state();
        app
    }

    fn ui_initial_state(&mut self) {
        self.ui.sprints.set_selected(Some(self.state.active_sprint));
        self.ui.sprints.set_border_color(Color::Green);
        self.sync_ui_issues_window();
        self.sync_ui_issue_description_window();
    }

    pub fn move_issue_selection_down(&mut self) {
        if (self.state.active_issue >= self.state.issues[self.state.active_sprint].len() - 1)
            || (self.state.active_issue >= self.ui.issues.inner_size().height - 1)
        {
            return;
        }
        self.state.active_issue += 1;
        self.ui.issues.set_selected(Some(self.state.active_issue));
        self.sync_ui_issue_description_window();
    }

    pub fn move_issue_selection_up(&mut self) {
        if self.state.active_issue == 0 {
            return;
        }
        self.state.active_issue -= 1;
        self.ui.issues.set_selected(Some(self.state.active_issue));
        self.sync_ui_issue_description_window();
    }

    fn sync_ui_issue_description_window(&mut self) {
        self.ui.issue_description.change_text(
            self.state.issues[self.state.active_sprint][self.state.active_issue]
                .fields
                .description
                .clone(),
        );
    }

    pub fn select_sprints_window(&mut self) {
        match self.state.active_window {
            Window::Description => self.ui.issue_description.set_border_color(Color::Default),
            Window::Issues => {
                self.ui.issues.set_border_color(Color::Default);
                self.ui.issues.set_selected(None);
            }
            Window::Sprints => return,
        };

        self.state.active_window = Window::Sprints;
        self.ui.sprints.set_border_color(Color::Green);
        self.ui.sprints.set_selected(Some(self.state.active_sprint));
    }

    pub fn select_issues_window(&mut self) {
        match self.state.active_window {
            Window::Description => self.ui.issue_description.set_border_color(Color::Default),
            Window::Sprints => {
                self.ui.sprints.set_border_color(Color::Default);
                self.ui.sprints.set_selected(None);
            }
            Window::Issues => return,
        };

        self.state.active_window = Window::Issues;
        self.ui.issues.set_border_color(Color::Green);
        self.ui.issues.set_selected(Some(self.state.active_issue));
    }

    pub fn select_issue_description_window(&mut self) {
        match self.state.active_window {
            Window::Sprints => {
                self.ui.issue_description.set_border_color(Color::Default);
                self.ui.sprints.set_selected(None);
            }
            Window::Issues => {
                self.ui.issues.set_border_color(Color::Default);
                self.ui.issues.set_selected(None);
            }
            Window::Description => return,
        };

        self.state.active_window = Window::Description;
        self.ui.issue_description.set_border_color(Color::Green);
    }

    pub fn move_sprint_selection_down(&mut self) {
        if (self.state.active_sprint >= self.state.sprints.len() - 1)
            | (self.state.active_sprint >= self.ui.sprints.inner_size().height)
        {
            return;
        }

        self.state.active_issue = 0;
        self.state.active_sprint += 1;
        self.ui.sprints.set_selected(Some(self.state.active_sprint));
        self.sync_ui_issues_window();
        self.sync_ui_issue_description_window();
    }

    pub fn move_sprint_selection_up(&mut self) {
        if self.state.active_sprint == 0 {
            return;
        }

        self.state.active_issue = 0;
        self.state.active_sprint -= 1;
        self.ui.sprints.set_selected(Some(self.state.active_sprint));
        self.sync_ui_issues_window();
        self.sync_ui_issue_description_window();
    }

    fn sync_ui_issues_window(&mut self) {
        let issues_table = self.state.issues[self.state.active_sprint]
            .iter()
            .take(self.ui.issues.inner_size().height)
            .map(|issue| {
                vec![
                    issue.name.clone(),
                    issue.fields.status.clone(),
                    issue.fields.kind.clone(),
                    issue
                        .fields
                        .assignee
                        .clone()
                        .map(|assignee| {
                            assignee
                                .split(" ")
                                .flat_map(|s| s.chars().nth(0))
                                .take(3)
                                .collect()
                        })
                        .unwrap_or_default(),
                    issue.fields.summary.clone(),
                ]
            })
            .collect();

        self.ui.issues.change_table(issues_table);
    }

    pub fn input(&self) -> std::io::Result<std::io::Bytes<std::fs::File>> {
        self.ui.terminal.tty()
    }
}

pub struct Ui {
    terminal: Terminal,
    sprints: tui::ItemList,
    issues: tui::Table,
    issue_description: tui::Text,
}

impl Ui {
    fn new(terminal: Terminal, state: &AppState) -> Ui {
        let rendering_region = terminal.rendering_region();
        let (left, mut right) = rendering_region.split_vertically_at(0.40);
        let (mut top, mut botton) = left.split_hotizontally_at(0.2);

        top.set_title(Some("[ 1 ] Sprints ".into()));
        let sprint_list: Vec<_> = state
            .sprints
            .iter()
            .map(|sprint| sprint.name.clone())
            .collect();
        let sprints = top.item_list(
            sprint_list,
            tui::VerticalAlignment::Top,
            tui::HorizontalAlignment::Left,
        );

        botton.set_title(Some("[ 2 ] Issues ".into()));
        let issues = botton.table(
            Default::default(),
            tui::VerticalAlignment::Top,
            tui::HorizontalAlignment::Left,
        );

        right.set_title(Some("[ 3 ] Description ".into()));
        let issue_description = right.text(
            Default::default(),
            tui::VerticalAlignment::Top,
            tui::HorizontalAlignment::Left,
        );

        Ui {
            terminal,
            sprints,
            issues,
            issue_description,
        }
    }

    pub fn render(&mut self) {
        self.sprints.render(&mut self.terminal.buffer);
        self.issues.render(&mut self.terminal.buffer);
        self.issue_description.render(&mut self.terminal.buffer);

        self.terminal.draw();
    }
}

#[derive(Clone, Copy)]
pub enum Window {
    Issues,
    Description,
    Sprints,
}

// TODO: Sync the AppState every so often
// TODO: The issue name is cut when it's too long, it might be useful to add it in the description
//       screen somehow
// TODO: Add '/' to filter issues or sprints
// TODO: Handle scrolling issues and sprints
