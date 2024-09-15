mod config;
mod jira;
mod tui;

use config::Config;
use jira::{Issue, Jira, Sprint};
use tui::{Color, Widget};

// FIXME: Perform better error handling instead of unwrapping everything.
struct App {
    jira: Jira,

    sprints: Vec<Sprint>,
    issues: Vec<Issue>,
    active_sprint: usize,
    active_issue: usize,

    tui: Tui,
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

        let sprints = jira.get_board_active_and_future_sprints(&board_id);
        let issues = jira.get_backlog_issues(&board_id);

        let tui = tui::Terminal::try_new().unwrap();

        let area = tui.area();
        let (left, mut right) = area.split_horizontally_at(0.2);
        let (mut top, mut botton) = left.split_vertically();

        top.set_title(Some("[ Sprints ]".into()));
        let sprint_list = sprints.iter().map(|sprint| sprint.name.clone()).collect();
        let sprints_tui = top.item_list(
            sprint_list,
            tui::VerticalAlignment::Top,
            tui::HorizontalAlignment::Center,
        );

        // TODO: Add scrolling
        let issues_table: Vec<Vec<String>> = issues
            .iter()
            .take(botton.height() - 2)
            .map(|issue| vec![issue.name.clone(), issue.fields.status.clone()])
            .collect();

        // TODO: Include more informatio about the issue such as who is assigned to the task
        botton.set_title(Some("[ Issues ]".into()));
        let mut issues_tui = botton.table(
            issues_table,
            tui::VerticalAlignment::Top,
            tui::HorizontalAlignment::Center,
        );
        issues_tui.set_border_color(Color::Green);
        issues_tui.set_selected(Some(0));

        right.set_title(Some(format!("[ {} ]", issues[0].name)));
        let description = issues[0]
            .fields
            .description
            .clone()
            .unwrap_or("This place will contain the selected issue details.".into());
        let issue_details_tui = right.text(
            description,
            tui::VerticalAlignment::Top,
            tui::HorizontalAlignment::Left,
        );

        let tui = Tui {
            tui,
            sprints: sprints_tui,
            issues: issues_tui,
            issue_details: issue_details_tui,
        };

        App {
            jira,
            sprints,
            issues,
            active_sprint: 0,
            active_issue: 0,
            tui,
        }
    }

    fn move_issue_selection_down(&mut self) {
        // FIXME: Deal with scrolling, currently is panicking
        if self.active_issue >= self.issues.len() {
            return;
        }

        self.active_issue += 1;

        self.sync_selection();
    }

    fn move_issue_selection_up(&mut self) {
        if self.active_issue == 0 {
            return;
        }

        self.active_issue -= 1;

        self.sync_selection();
    }

    fn sync_selection(&mut self) {
        self.tui.issues.set_selected(Some(self.active_issue));
        self.tui
            .issue_details
            .change_text(self.issues[self.active_issue].fields.description.clone());
        self.tui.issue_details.set_title(Some(format!(
            "[ {} ]",
            self.issues[self.active_issue].name.clone()
        )));
    }

    fn input(&self) -> std::io::Result<std::io::Bytes<std::fs::File>> {
        self.tui.tui.tty()
    }
}

struct Tui {
    tui: tui::Terminal,
    sprints: tui::ItemList,
    issues: tui::Table,
    issue_details: tui::Text,
}

impl Tui {
    fn render(&mut self) {
        self.sprints.render(&mut self.tui);
        self.issues.render(&mut self.tui);
        self.issue_details.render(&mut self.tui);

        self.tui.draw();
    }
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

        // TODO: Add possibility to change from issues to sprint view with (1) and (2)
        // TODO: Allow filtering issues by who is assigned to it
        match input {
            b'j' => app.move_issue_selection_down(),
            b'k' => app.move_issue_selection_up(),
            b'q' => break,
            _ => (),
        };
    }
}
