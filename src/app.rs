use crate::jira::{Issue, Jira, Sprint};
use crate::tui::{self, Color, Terminal, Widget};
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
pub struct State {
    pub sprints: Vec<Sprint>,
    pub issues: Vec<Vec<Issue>>,
}

impl State {
    pub fn new(jira: &Jira, board_id: &str) -> State {
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

        State { sprints, issues }
    }
}

pub struct Ui {
    terminal: Terminal,

    pub active_window: Window,
    sprint_offset: usize,
    active_sprint: usize,
    issue_offset: usize,
    active_issue: usize,

    state: State,

    sprints: tui::ItemList,
    issues: tui::Table,
    issue_description: tui::Text,
}

impl Ui {
    pub fn new(terminal: Terminal, initial_state: State) -> Ui {
        let (left, mut right) = terminal.rendering_region().split_vertically_at(0.40);
        let (mut top, mut botton) = left.split_hotizontally_at(0.2);

        top.set_title(Some("[ 1 ] Sprints ".into()));
        let sprints = top.item_list(
            Default::default(),
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

        let mut ui = Ui {
            terminal,
            active_sprint: 0,
            sprint_offset: 0,
            active_issue: 0,
            issue_offset: 0,
            state: initial_state,
            active_window: Window::Sprints,
            sprints,
            issues,
            issue_description,
        };

        // We need to do the initial sync to show the data into the terminal
        ui.sync_state();

        ui
    }

    pub fn load_state() -> Option<State> {
        let home_directory = std::env::var("HOME").unwrap();
        let Ok(file) = std::fs::File::open(format!("{home_directory}/.canoa.json")) else {
            return None;
        };
        let file = std::io::BufReader::new(file);
        let state = serde_json::from_reader(file).unwrap();
        Some(state)
    }

    pub fn save_state(&self) {
        let home_directory = std::env::var("HOME").unwrap();
        let file = std::fs::File::create(format!("{home_directory}/.canoa.json")).unwrap();
        let file = std::io::BufWriter::new(file);
        serde_json::to_writer(file, &self.state).unwrap();
    }

    pub fn update_state(&mut self, state: State) {
        let current_sprint_id = self.state.sprints[self.sprint_offset].id;
        let current_issue_id = &self.state.issues[self.sprint_offset][self.issue_offset].id;

        self.sprint_offset = state
            .sprints
            .iter()
            .position(|sprint| sprint.id == current_sprint_id)
            .unwrap_or(0);

        self.issue_offset = self.state.issues[self.sprint_offset]
            .iter()
            .position(|issue| &issue.id == current_issue_id)
            .unwrap_or(0);

        self.state = state;

        self.sync_state();
    }

    pub fn sync_state(&mut self) {
        self.sync_issues_window();
        self.sync_issue_description_window();
        self.sync_sprints_window();
        self.sprints.set_selected(Some(self.active_sprint));
        self.sprints.set_border_color(Color::Green);
    }

    pub fn sync_issues_window(&mut self) {
        let issues_table = self.state.issues[self.active_sprint][self.issue_offset..]
            .iter()
            .take(self.issues.inner_size().height)
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

        self.issues.change_table(issues_table);
    }

    pub fn sync_issue_description_window(&mut self) {
        self.issue_description.change_text(
            self.state.issues[self.active_sprint][self.active_issue]
                .fields
                .description
                .clone(),
        );
    }

    pub fn sync_sprints_window(&mut self) {
        let sprints_list = self.state.sprints[self.sprint_offset..]
            .iter()
            .take(self.sprints.inner_size().height)
            .map(|sprint| sprint.name.clone())
            .collect();

        self.sprints.change_list(sprints_list);
    }

    pub fn render(&mut self) {
        self.sprints.render(&mut self.terminal.buffer);
        self.issues.render(&mut self.terminal.buffer);
        self.issue_description.render(&mut self.terminal.buffer);

        self.terminal.draw();
    }

    pub fn select_sprints_window(&mut self) {
        match self.active_window {
            Window::Description => self.issue_description.set_border_color(Color::Default),
            Window::Issues => {
                self.issues.set_border_color(Color::Default);
                self.issues.set_selected(None);
            }
            Window::Sprints => return,
        };

        self.active_window = Window::Sprints;
        self.sprints.set_border_color(Color::Green);
        self.sprints.set_selected(Some(self.active_sprint));
    }

    pub fn select_issues_window(&mut self) {
        match self.active_window {
            Window::Description => self.issue_description.set_border_color(Color::Default),
            Window::Sprints => {
                self.sprints.set_border_color(Color::Default);
                self.sprints.set_selected(None);
            }
            Window::Issues => return,
        };
        self.active_window = Window::Issues;
        self.issues.set_border_color(Color::Green);
        self.issues.set_selected(Some(self.active_issue));
    }

    pub fn select_issue_description_window(&mut self) {
        match self.active_window {
            Window::Sprints => {
                self.issue_description.set_border_color(Color::Default);
                self.sprints.set_selected(None);
            }
            Window::Issues => {
                self.issues.set_border_color(Color::Default);
                self.issues.set_selected(None);
            }
            Window::Description => return,
        };

        self.active_window = Window::Description;
        self.issue_description.set_border_color(Color::Green);
    }

    pub fn move_issue_selection_down(&mut self) {
        if self.active_issue >= self.state.issues[self.active_sprint].len() - 1 {
            return;
        };

        self.active_issue += 1;

        if self.active_issue - self.issue_offset >= self.issues.inner_size().height {
            self.issue_offset += 1;
            self.sync_issues_window();
        }

        self.issues
            .set_selected(Some(self.active_issue - self.issue_offset));

        self.sync_issue_description_window();
    }

    pub fn move_issue_selection_up(&mut self) {
        if self.active_issue <= 0 {
            return;
        };

        self.active_issue -= 1;

        if self.active_issue - self.issue_offset >= self.issues.inner_size().height {
            self.issue_offset -= 1;
            self.sync_issues_window();
        }

        self.issues
            .set_selected(Some(self.active_issue - self.issue_offset));
        self.sync_issue_description_window();
    }

    pub fn move_sprint_selection_down(&mut self) {
        if self.active_sprint >= self.state.sprints.len() - 1 {
            return;
        }

        self.active_sprint += 1;
        self.active_issue = 0;

        if self.active_sprint - self.sprint_offset >= self.sprints.inner_size().height {
            self.sprint_offset += 1;
            self.sync_sprints_window();
        }

        self.sprints
            .set_selected(Some(self.active_sprint - self.sprint_offset));

        self.sync_issues_window();
        self.sync_issue_description_window();
    }

    pub fn move_sprint_selection_up(&mut self) {
        if self.active_sprint == 0 {
            return;
        }

        self.active_issue = 0;
        self.active_sprint -= 1;

        if self.active_sprint - self.sprint_offset >= self.sprints.inner_size().height {
            self.sprint_offset -= 1;
            self.sync_sprints_window();
        }

        self.sprints
            .set_selected(Some(self.active_sprint - self.sprint_offset));

        self.sync_issues_window();
        self.sync_issue_description_window();
    }
}

#[derive(Clone, Copy)]
pub enum Window {
    Issues,
    Description,
    Sprints,
}

// TODO: Sync the State every so often
// TODO: The issue name is cut when it's too long, it might be useful to add it in the description
//       screen somehow
// TODO: Add '/' to filter issues or sprints
// TODO: Add scrolling to description
