mod config;
mod jira;
mod tui;

use config::Config;
use tui::{Color, Widget};

struct App {
    jira: jira::Jira,
    sprints: Vec<jira::Sprint>,
    issues: Vec<jira::Issue>,
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

        let jira = jira::Jira::new(&user, &token, host);

        let sprints = jira.get_board_active_and_future_sprints(&board_id);
        let issues = jira.get_backlog_issues(&board_id);

        let terminal = tui::Terminal::new();

        let area = terminal.area();
        let (left, mut right) = area.split_horizontally_at(0.2);
        let (mut top, mut botton) = left.split_vertically();

        top.set_title(Some("[ Sprints ]".into()));
        let sprint_list = sprints.iter().map(|sprint| sprint.name.clone()).collect();
        let sprints_tui = top.item_list(
            sprint_list,
            tui::VerticalAlignment::Top,
            tui::HorizontalAlignment::Center,
        );

        let issues_table: Vec<Vec<String>> = issues
            .iter()
            .take(botton.height() - 2)
            .map(|issue| vec![issue.name.clone(), issue.fields.status.clone()])
            .collect();

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
            tui::VerticalAlignment::Center,
            tui::HorizontalAlignment::Left,
        );

        let tui = Tui {
            terminal,
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

    fn select_issue(&mut self, index: usize) {
        self.tui.issues.set_selected(Some(index));
        self.tui
            .issue_details
            .change_text(self.issues[index].fields.description.clone())
    }

    fn render(&mut self) {
        self.tui.sprints.render(&mut self.tui.terminal);
        self.tui.issues.render(&mut self.tui.terminal);
        self.tui.issue_details.render(&mut self.tui.terminal);

        self.tui.terminal.flush();
    }
}

struct Tui {
    terminal: tui::Terminal,
    sprints: tui::ItemList,
    issues: tui::Table,
    issue_details: tui::Text,
}

fn main() {
    let config = config::configuration().unwrap();

    let mut app = App::from_config(config);

    loop {
        app.select_issue(0);
        app.render();

        std::thread::sleep(std::time::Duration::from_secs(1));

        app.select_issue(7);
        app.render();

        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
