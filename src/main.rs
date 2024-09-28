mod config;
mod jira;
mod tui;

use config::Config;
use jira::{Issue, Jira, Sprint};
use tui::{Color, Terminal, Widget};

// FIXME: Perform better error handling instead of unwrapping everything.
struct App {
    state: State,
    tui: Tui,
}

struct State {
    active_sprint: usize,
    active_issue: usize,

    active_window: Window,

    sprints: Vec<Sprint>,
    issues: Vec<Vec<Issue>>,
    backlog: Vec<Issue>,
}

impl State {
    fn fetch(jira: &Jira, board_id: &str) -> State {
        let sprints = jira.get_board_active_and_future_sprints(board_id);

        let (issues, backlog) = std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(sprints.len());

            for sprint in sprints.iter() {
                let handle = scope.spawn(|| jira.get_sprint_issues(board_id, sprint.id));
                handles.push(handle);
            }

            let backlog = scope
                .spawn(|| jira.get_backlog_issues(board_id))
                .join()
                .unwrap();

            let issues = handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect();

            (issues, backlog)
        });

        State {
            active_sprint: 0,
            active_issue: 0,
            active_window: Window::Issues,
            sprints,
            issues,
            backlog,
        }
    }
}

impl App {
    fn from_config(config: config::Config) -> Self {
        let Config {
            user,
            token,
            board_id,
            host,
        } = config;

        let jira = Jira::new(&user, &token, host);
        let state = State::fetch(&jira, &board_id);
        let terminal = Terminal::try_new().unwrap();
        let tui = Tui::construct(terminal, &state);

        App { state, tui }
    }

    fn move_issue_selection_down(&mut self) {
        // FIXME: Deal with scrolling, currently is panicking
        if self.state.active_issue >= self.state.issues[self.state.active_sprint].len() - 1 {
            return;
        }
        self.state.active_issue += 1;
        self.sync_issues_selection();
    }

    fn move_issue_selection_up(&mut self) {
        if self.state.active_issue == 0 {
            return;
        }
        self.state.active_issue -= 1;
        self.sync_issues_selection();
    }

    fn sync_issues_selection(&mut self) {
        self.tui.issues.set_selected(Some(self.state.active_issue));
        self.tui.issue_description.change_text(
            self.state.issues[self.state.active_sprint][self.state.active_issue]
                .fields
                .description
                .clone(),
        );
    }

    fn input(&self) -> std::io::Result<std::io::Bytes<std::fs::File>> {
        self.tui.terminal.tty()
    }

    fn select_sprints(&mut self) {
        match self.state.active_window {
            Window::Description => self.tui.issue_description.set_border_color(Color::Default),
            Window::Issues => {
                self.tui.issues.set_border_color(Color::Default);
                self.tui.issues.set_selected(None);
            }
            Window::Sprints => return,
        };

        self.state.active_window = Window::Sprints;
        self.tui.sprints.set_border_color(Color::Green);
        self.tui
            .sprints
            .set_selected(Some(self.state.active_sprint));
    }

    fn select_issues(&mut self) {
        match self.state.active_window {
            Window::Description => self.tui.issue_description.set_border_color(Color::Default),
            Window::Sprints => {
                self.tui.sprints.set_border_color(Color::Default);
                self.tui.sprints.set_selected(None);
            }
            Window::Issues => return,
        };

        self.state.active_window = Window::Issues;
        self.tui.issues.set_border_color(Color::Green);
        self.tui.issues.set_selected(Some(self.state.active_issue));
    }

    fn select_issue_description(&mut self) {
        match self.state.active_window {
            Window::Sprints => {
                self.tui.issue_description.set_border_color(Color::Default);
                self.tui.sprints.set_selected(None);
            }
            Window::Issues => {
                self.tui.issues.set_border_color(Color::Default);
                self.tui.issues.set_selected(None);
            }
            Window::Description => return,
        };

        self.state.active_window = Window::Description;
        self.tui.issue_description.set_border_color(Color::Green);
    }

    fn move_sprint_selection_down(&mut self) {
        // FIXME: Deal with scrolling, currently is panicking
        // TODO: Deal with backlog
        if self.state.active_sprint >= self.state.sprints.len() - 1 {
            return;
        }

        self.state.active_issue = 0;
        self.sync_issues_selection();
        self.state.active_sprint += 1;

        let issues_table: Vec<Vec<String>> = self.state.issues[self.state.active_sprint]
            .iter()
            .take(self.tui.issues.inner_size().height)
            .map(|issue| {
                vec![
                    issue.name.clone(),
                    issue.fields.status.clone(),
                    issue.fields.kind.clone(),
                    format_assignee(issue.fields.assignee.clone()),
                    issue.fields.summary.clone(),
                ]
            })
            .collect();

        self.tui.issues.change_table(issues_table);

        self.tui
            .sprints
            .set_selected(Some(self.state.active_sprint))
    }

    fn move_sprint_selection_up(&mut self) {
        // FIXME: Deal with scrolling, currently is panicking
        if self.state.active_sprint == 0 {
            return;
        }

        self.state.active_issue = 0;
        self.sync_issues_selection();
        self.state.active_sprint -= 1;

        let issues_table: Vec<Vec<String>> = self.state.issues[self.state.active_sprint]
            .iter()
            .take(self.tui.issues.inner_size().height)
            .map(|issue| {
                vec![
                    issue.name.clone(),
                    issue.fields.status.clone(),
                    issue.fields.kind.clone(),
                    format_assignee(issue.fields.assignee.clone()),
                    issue.fields.summary.clone(),
                ]
            })
            .collect();

        self.tui.issues.change_table(issues_table);

        self.tui
            .sprints
            .set_selected(Some(self.state.active_sprint))
    }
}

struct Tui {
    terminal: Terminal,
    sprints: tui::ItemList,
    issues: tui::Table,
    issue_description: tui::Text,
}

impl Tui {
    fn construct(terminal: Terminal, state: &State) -> Tui {
        let rendering_region = terminal.rendering_region();
        let (left, mut right) = rendering_region.split_vertically_at(0.40);
        let (mut top, mut botton) = left.split_hotizontally_at(0.2);

        top.set_title(Some("[ 1 ] Sprints ".into()));
        let mut sprint_list: Vec<_> = state
            .sprints
            .iter()
            .map(|sprint| sprint.name.clone())
            .collect();
        sprint_list.push("Backlog".into());
        let mut sprints = top.item_list(
            sprint_list,
            tui::VerticalAlignment::Top,
            tui::HorizontalAlignment::Left,
        );
        sprints.set_selected(Some(0));

        // TODO: Add scrolling
        let issues_table: Vec<Vec<String>> = state.issues[0]
            .iter()
            .take(botton.inner_size().height)
            .map(|issue| {
                vec![
                    issue.name.clone(),
                    issue.fields.status.clone(),
                    issue.fields.kind.clone(),
                    format_assignee(issue.fields.assignee.clone()),
                    issue.fields.summary.clone(),
                ]
            })
            .collect();

        botton.set_title(Some("[ 2 ] Issues ".into()));
        let mut issues = botton.table(
            issues_table,
            tui::VerticalAlignment::Top,
            tui::HorizontalAlignment::Left,
        );
        issues.set_border_color(Color::Green);
        issues.set_selected(Some(0));

        let description = state.issues[0][0]
            .fields
            .description
            .clone()
            .unwrap_or("This place will contain the selected issue details.".into());
        right.set_title(Some("[ 3 ] Description ".into()));
        let issue_description = right.text(
            description,
            tui::VerticalAlignment::Top,
            tui::HorizontalAlignment::Left,
        );

        Tui {
            terminal,
            sprints,
            issues,
            issue_description,
        }
    }

    fn render(&mut self) {
        self.sprints.render(&mut self.terminal.buffer);
        self.issues.render(&mut self.terminal.buffer);
        self.issue_description.render(&mut self.terminal.buffer);

        self.terminal.draw();
    }
}

#[derive(Clone, Copy)]
enum Window {
    Issues,
    Description,
    Sprints,
}

fn main() {
    let config = config::configuration().unwrap();

    let mut app = App::from_config(config);
    let mut inputs = app.input().unwrap();

    loop {
        app.tui.render();

        let Some(input) = inputs.next().map(|input| input.unwrap()) else {
            break;
        };

        // TODO: Allow filtering issues by who is assigned to it
        match input {
            // General commands
            b'1' => app.select_sprints(),
            b'2' => app.select_issues(),
            b'3' => app.select_issue_description(),
            b'q' => break,

            // Sprints commands
            b'j' if matches!(app.state.active_window, Window::Sprints) => {
                app.move_sprint_selection_down()
            }
            b'k' if matches!(app.state.active_window, Window::Sprints) => {
                app.move_sprint_selection_up()
            }

            // Issue list commands
            b'j' if matches!(app.state.active_window, Window::Issues) => {
                app.move_issue_selection_down()
            }
            b'k' if matches!(app.state.active_window, Window::Issues) => {
                app.move_issue_selection_up()
            }
            _ => (),
        };
    }
}

fn format_assignee(assignee: Option<String>) -> String {
    if let Some(assignee) = assignee {
        return assignee
            .split(" ")
            .flat_map(|s| s.chars().nth(0))
            .take(3)
            .collect();
    }

    String::new()
}

// TODO: Add color to the issue status
