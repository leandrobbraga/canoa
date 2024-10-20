mod app;
mod commands;
mod config;
mod jira;
mod tui;

use app::{State, Ui, Window};
use config::Config;
use jira::Jira;
use tui::Terminal;

fn main() {
    let Config {
        user,
        token,
        board_id,
        host,
    } = config::configuration().unwrap();

    let jira = Jira::new(&user, &token, host);
    let state = State::new(&jira, &board_id);
    let mut terminal = Terminal::try_new().unwrap();
    let mut inputs = terminal.tty().unwrap();
    let mut ui = Ui::new(terminal.rendering_region());

    ui.initial_state(&state);

    loop {
        ui.render(&mut terminal.buffer);
        terminal.draw();

        let Some(input) = inputs.next().map(|input| input.unwrap()) else {
            break;
        };

        // Commands that are independent to the active_window
        match input {
            b'1' => ui.select_sprints_window(),
            b'2' => ui.select_issues_window(),
            b'3' => ui.select_issue_description_window(),
            b'q' => break,
            _ => (),
        };

        // Window-specific commands.
        match ui.active_window {
            Window::Issues => match input {
                b'j' => ui.move_issue_selection_down(&state),
                b'k' => ui.move_issue_selection_up(&state),
                _ => (),
            },
            Window::Sprints => match input {
                b'j' => ui.move_sprint_selection_down(&state),
                b'k' => ui.move_sprint_selection_up(&state),
                _ => (),
            },
            _ => (),
        };
    }
}

// TODO: Add color to the issue status
// TODO: Allow filtering issues by who is assigned to it
// FIXME: Perform better error handling instead of unwrapping everything.
