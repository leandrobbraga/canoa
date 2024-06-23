mod config;
mod jira;
mod tui;

use config::Config;
use tui::Widget;

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

    let mut terminal = tui::Terminal::new(120, 60);
    let area = terminal.area();
    let (left, right) = area.split_horizontally_at(0.3);
    let (top, botton) = left.split_vertically();

    let mut sprint_list = vec!["Sprints".into()];
    sprint_list.extend(sprints.into_iter().map(|sprint| sprint.name));
    let sprints_tui = top.item_list(
        sprint_list,
        tui::VerticalAlignment::Center,
        tui::HorizontalAlignment::Center,
    );

    let mut issues_table = vec![vec!["Issue".into(), "Status".into()]];
    issues_table.extend(
        issues
            .into_iter()
            .take(10)
            .map(|issue| vec![issue.name, issue.fields.status]),
    );
    let issues_tui = botton.table(
        issues_table,
        tui::VerticalAlignment::Center,
        tui::HorizontalAlignment::Center,
    );

    let issue_details_tui = right.text(
        "This place will contain the selected issue details.".into(),
        tui::VerticalAlignment::Center,
        tui::HorizontalAlignment::Center,
    );

    sprints_tui.render(&mut terminal);
    issues_tui.render(&mut terminal);
    issue_details_tui.render(&mut terminal);

    terminal.render();

    std::thread::sleep(std::time::Duration::from_secs(10));
}
