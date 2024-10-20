use crate::jira::{Issue, Jira, Sprint};
use crate::tui::{self, Buffer, Color, RenderingRegion, Widget};

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
    pub active_window: Window,
    sprint_offset: usize,
    active_sprint: usize,
    issue_offset: usize,
    active_issue: usize,

    sprints: tui::ItemList,
    issues: tui::Table,
    issue_description: tui::Text,
}

impl Ui {
    pub fn new(rendering_region: RenderingRegion) -> Ui {
        let (left, mut right) = rendering_region.split_vertically_at(0.40);
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

        Ui {
            active_sprint: 0,
            sprint_offset: 0,
            active_issue: 0,
            issue_offset: 0,
            active_window: Window::Sprints,
            sprints,
            issues,
            issue_description,
        }
    }

    pub fn initial_state(&mut self, state: &State) {
        self.active_sprint = 0;
        self.sprint_offset = 0;
        self.active_issue = 0;
        self.issue_offset = 0;
        self.active_window = Window::Sprints;

        self.sync_issues_window(state);
        self.sync_issue_description_window(state);
        self.sync_sprints_window(state);
        self.sprints.set_selected(Some(self.active_sprint));
        self.sprints.set_border_color(Color::Green);
    }

    pub fn sync_issues_window(&mut self, state: &State) {
        let issues_table = state.issues[self.active_sprint][self.issue_offset..]
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

    pub fn sync_issue_description_window(&mut self, state: &State) {
        self.issue_description.change_text(
            state.issues[self.active_sprint][self.active_issue]
                .fields
                .description
                .clone(),
        );
    }

    pub fn sync_sprints_window(&mut self, state: &State) {
        let sprints_list = state.sprints[self.sprint_offset..]
            .iter()
            .take(self.sprints.inner_size().height)
            .map(|sprint| sprint.name.clone())
            .collect();

        self.sprints.change_list(sprints_list);
    }

    pub fn render(&mut self, buffer: &mut Buffer) {
        self.sprints.render(buffer);
        self.issues.render(buffer);
        self.issue_description.render(buffer);
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

    pub fn move_issue_selection_down(&mut self, state: &State) {
        if self.active_issue >= state.issues[self.active_sprint].len() - 1 {
            return;
        };

        self.active_issue += 1;

        if self.active_issue - self.issue_offset >= self.issues.inner_size().height {
            self.issue_offset += 1;
            self.sync_issues_window(state);
        }

        self.issues
            .set_selected(Some(self.active_issue - self.issue_offset));

        self.sync_issue_description_window(state);
    }

    pub fn move_issue_selection_up(&mut self, state: &State) {
        if self.active_issue <= 0 {
            return;
        };

        self.active_issue -= 1;

        if self.active_issue - self.issue_offset >= self.issues.inner_size().height {
            self.issue_offset -= 1;
            self.sync_issues_window(state);
        }

        self.issues
            .set_selected(Some(self.active_issue - self.issue_offset));
        self.sync_issue_description_window(state);
    }

    pub fn move_sprint_selection_down(&mut self, state: &State) {
        if self.active_sprint >= state.sprints.len() - 1 {
            return;
        }

        self.active_sprint += 1;
        self.active_issue = 0;

        if self.active_sprint - self.sprint_offset >= self.sprints.inner_size().height {
            self.sprint_offset += 1;
            self.sync_sprints_window(state);
        }

        self.sprints
            .set_selected(Some(self.active_sprint - self.sprint_offset));

        self.sync_issues_window(state);
        self.sync_issue_description_window(state);
    }

    pub fn move_sprint_selection_up(&mut self, state: &State) {
        if self.active_sprint == 0 {
            return;
        }

        self.active_issue = 0;
        self.active_sprint -= 1;

        if self.active_sprint - self.sprint_offset >= self.sprints.inner_size().height {
            self.sprint_offset -= 1;
            self.sync_sprints_window(state);
        }

        self.sprints
            .set_selected(Some(self.active_sprint - self.sprint_offset));

        self.sync_issues_window(state);
        self.sync_issue_description_window(state);
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
// TODO: Add scrolling to description
