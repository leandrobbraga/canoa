use std::time::UNIX_EPOCH;

use crate::jira::{Issue, Jira, Sprint};
use crate::tui::{self, Color, CommonWidget, Terminal, Widget};
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
pub struct State {
    pub sprints: Vec<Sprint>,
    pub issues: Vec<Vec<Issue>>,
}

impl State {
    pub fn new(jira: &Jira, board_id: &str) -> State {
        std::thread::scope(|scope| {
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

            State { sprints, issues }
        })
    }
}

pub struct App {
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
    logs: tui::ItemList,
}

impl App {
    pub fn new(terminal: Terminal, initial_state: State) -> App {
        let rendering_region = terminal.rendering_region();

        let (top, mut logs) = rendering_region.split_horizontally_percentage(0.9);

        let (left, mut issue_description) = top.split_vertically_at_percentage(0.40);
        let (mut sprints, mut issues) = left.split_horizontally_percentage(0.2);

        sprints.set_title(Some("[ 1 ] Sprints ".into()));
        sprints.set_border(Some(Color::Default));
        let sprints = sprints.item_list();

        issues.set_title(Some("[ 2 ] Issues ".into()));
        issues.set_border(Some(Color::Default));
        let issues = issues.table();

        issue_description.set_title(Some("[ 3 ] Description ".into()));
        issue_description.set_border(Some(Color::Default));
        let issue_description = issue_description.text();

        logs.set_title(Some("Logs".into()));
        logs.set_border(Some(Color::Default));
        let logs = logs.item_list();

        let mut ui = App {
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
            logs,
        };

        // We need to do the initial sync to show the data into the terminal
        ui.sync_state();
        ui.sprints.set_selected(Some(ui.active_sprint));
        ui.sprints.set_border(Some(Color::Green));

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
        let current_sprint_id = self.state.sprints[self.active_sprint].id;
        let current_issue_id = &self.state.issues[self.active_sprint][self.active_issue].id;

        self.active_sprint = state
            .sprints
            .iter()
            .position(|sprint| sprint.id == current_sprint_id)
            .unwrap_or(0);

        self.active_issue = state.issues[self.sprint_offset]
            .iter()
            .position(|issue| &issue.id == current_issue_id)
            .unwrap_or(0);

        self.state = state;

        let time_elapsed_since_unix_epoch = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let secs_until_now = time_elapsed_since_unix_epoch % (24 * 60 * 60);

        let hours = secs_until_now / (60 * 60);
        let minutes = secs_until_now % (60 * 60) / 60;
        let seconds = secs_until_now % 60;

        let logs_max_count = self.logs.usable_size().height;
        let log_items = self.logs.get_items_mut();
        if log_items.len() >= logs_max_count {
            log_items.swap_remove(0);
        }
        log_items.push(format!(
            "{hours:0>2}:{minutes:0>2}:{seconds:0>2} INFO: Synced state"
        ));

        self.sync_state();

        match self.active_window {
            Window::Description => (),
            Window::Issues => self.issues.set_selected(Some(self.active_issue)),
            Window::Sprints => self.sprints.set_selected(Some(self.active_sprint)),
        }
    }

    pub fn sync_state(&mut self) {
        self.sync_issues_window();
        self.sync_issue_description_window();
        self.sync_sprints_window();
    }

    pub fn sync_issues_window(&mut self) {
        let issues_table = self.state.issues[self.active_sprint][self.issue_offset..]
            .iter()
            .take(self.issues.usable_size().height)
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
        self.issue_description.set_text(
            self.state.issues[self.active_sprint][self.active_issue]
                .fields
                .description
                .clone(),
        );
    }

    pub fn sync_sprints_window(&mut self) {
        let sprints_list = self.state.sprints[self.sprint_offset..]
            .iter()
            .take(self.sprints.usable_size().height)
            .map(|sprint| sprint.name.clone())
            .collect();

        self.sprints.change_list(sprints_list);
    }

    pub fn render(&mut self) {
        self.sprints.render(&mut self.terminal.buffer);
        self.issues.render(&mut self.terminal.buffer);
        self.issue_description.render(&mut self.terminal.buffer);
        self.logs.render(&mut self.terminal.buffer);

        self.terminal.draw();
    }

    pub fn select_sprints_window(&mut self) {
        self.unselect_windows();
        self.active_window = Window::Sprints;
        self.sprints.set_border(Some(Color::Green));
        self.sprints.set_selected(Some(self.active_sprint));
    }

    pub fn select_issues_window(&mut self) {
        self.unselect_windows();
        self.active_window = Window::Issues;
        self.issues.set_border(Some(Color::Green));
        self.issues.set_selected(Some(self.active_issue));
    }

    pub fn select_issue_description_window(&mut self) {
        self.unselect_windows();
        self.active_window = Window::Description;
        self.issue_description.set_border(Some(Color::Green));
    }

    fn unselect_windows(&mut self) {
        match self.active_window {
            Window::Sprints => {
                self.issue_description.set_border(Some(Color::Default));
                self.sprints.set_selected(None);
            }
            Window::Issues => {
                self.issues.set_border(Some(Color::Default));
                self.issues.set_selected(None);
            }
            Window::Description => self.issue_description.set_border(Some(Color::Default)),
        };
    }

    pub fn move_issue_selection_down(&mut self) {
        if self.active_issue >= self.state.issues[self.active_sprint].len() - 1 {
            return;
        };

        self.active_issue += 1;

        if self.active_issue - self.issue_offset >= self.issues.usable_size().height {
            self.issue_offset += 1;
            self.sync_issues_window();
        }

        self.issues
            .set_selected(Some(self.active_issue - self.issue_offset));

        self.sync_issue_description_window();
    }

    pub fn move_issue_selection_up(&mut self) {
        if self.active_issue == 0 {
            return;
        };

        self.active_issue -= 1;

        if self.active_issue - self.issue_offset >= self.issues.usable_size().height {
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

        if self.active_sprint - self.sprint_offset >= self.sprints.usable_size().height {
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

        if self.active_sprint - self.sprint_offset >= self.sprints.usable_size().height {
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
    Description,
    Issues,
    Sprints,
}

// TODO: The issue name is cut when it's too long, it might be useful to add it in the description
//       screen somehow
// TODO: Add '/' to filter issues or sprints
// TODO: Add scrolling to description
