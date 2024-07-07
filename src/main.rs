mod config;
mod jira;
mod tui;

use config::Config;
use tui::{Color, Widget};

fn main() {
    let Config {
        user,
        token,
        board_id,
        host,
    }: config::Config = config::configuration().unwrap();

    let jira = jira::Jira::new(&user, &token, host);

    let sprints = jira.get_board_active_and_future_sprints(&board_id);
    let issues = jira.get_backlog_issues(&board_id);

    let first_issue = issues[0].clone();
    let second_issue = issues[7].clone();

    let mut terminal = tui::Terminal::new();
    let area = terminal.area();
    let (left, mut right) = area.split_horizontally_at(0.2);
    let (mut top, mut botton) = left.split_vertically();

    top.set_title(Some("[ Sprints ]".into()));
    let sprint_list = sprints.into_iter().map(|sprint| sprint.name).collect();
    let sprints_tui = top.item_list(
        sprint_list,
        tui::VerticalAlignment::Top,
        tui::HorizontalAlignment::Center,
    );

    let issues_table: Vec<Vec<String>> = issues
        .into_iter()
        .take(botton.height() - 2)
        .map(|issue| vec![issue.name, issue.fields.status])
        .collect();

    botton.set_title(Some("[ Issues ]".into()));
    let mut issues_tui = botton.table(
        issues_table,
        tui::VerticalAlignment::Top,
        tui::HorizontalAlignment::Center,
    );
    issues_tui.set_border_color(Color::Green);
    issues_tui.set_selected(Some(0));

    right.set_title(Some(format!("[ {} ]", first_issue.name)));
    let description = first_issue
        .fields
        .description
        .unwrap_or("This place will contain the selected issue details.".into());
    let mut issue_details_tui = right.text(
        description,
        tui::VerticalAlignment::Center,
        tui::HorizontalAlignment::Left,
    );

    sprints_tui.render(&mut terminal);
    issues_tui.render(&mut terminal);
    issue_details_tui.render(&mut terminal);

    terminal.flush();

    std::thread::sleep(std::time::Duration::from_secs(1));

    issues_tui.set_selected(Some(7));
    let mut description = second_issue
        .fields
        .description
        .unwrap_or("This place will contain the selected issue details.".into());
    issue_details_tui.swap_text(&mut description);

    sprints_tui.render(&mut terminal);
    issues_tui.render(&mut terminal);
    issue_details_tui.render(&mut terminal);

    terminal.flush();

    std::thread::sleep(std::time::Duration::from_secs(5));
}
