mod app;
mod config;
mod jira;
mod tui;

use app::{App, Window};

fn main() {
    let config = config::configuration().unwrap();

    let mut app = App::from_config(config);
    let mut inputs = app.input().unwrap();

    loop {
        app.ui.render();

        let Some(input) = inputs.next().map(|input| input.unwrap()) else {
            break;
        };

        // Commands that are independent to the active_window
        match input {
            b'1' => app.select_sprints_window(),
            b'2' => app.select_issues_window(),
            b'3' => app.select_issue_description_window(),
            b'q' => break,
            _ => (),
        };

        // Window-specific commands
        match app.state.active_window {
            Window::Issues => match input {
                b'j' => app.move_issue_selection_down(),
                b'k' => app.move_issue_selection_up(),
                _ => (),
            },
            Window::Sprints => match input {
                b'j' => app.move_sprint_selection_down(),
                b'k' => app.move_sprint_selection_up(),
                _ => (),
            },
            _ => (),
        };
    }
}

// TODO: Add color to the issue status
// TODO: Allow filtering issues by who is assigned to it
// FIXME: Perform better error handling instead of unwrapping everything.
