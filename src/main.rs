mod app;
mod commands;
mod config;
mod jira;
mod tui;

use std::sync::mpsc;

use app::{State, Ui, Window};
use config::Config;
use jira::Jira;
use tui::Terminal;

enum Event {
    State(State),
    Input(u8),
}

fn main() {
    let Config {
        user,
        token,
        board_id,
        host,
    } = config::configuration().unwrap();
    let mut terminal = Terminal::try_new().unwrap();
    let mut inputs = terminal.tty().unwrap();
    let mut ui = Ui::new(terminal.rendering_region());

    let (sender, receiver) = mpsc::sync_channel(0);

    // This thread updates the state in the background
    let state_sender = sender.clone();
    std::thread::spawn(move || {
        let jira = Jira::new(&user, &token, host);
        loop {
            let state = State::new(&jira, &board_id);
            state_sender.send(Event::State(state)).unwrap();
            std::thread::sleep(std::time::Duration::from_secs(30))
        }
    });

    // This thread receive user input in the background
    std::thread::spawn(move || loop {
        let Some(input) = inputs.next().map(|input| input.unwrap()) else {
            break;
        };
        sender.send(Event::Input(input)).unwrap();
    });

    loop {
        ui.render(&mut terminal.buffer);
        terminal.draw();

        match receiver.recv().unwrap() {
            Event::State(state) => ui.update_state(state),
            Event::Input(input) => {
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
                        b'j' => ui.move_issue_selection_down(),
                        b'k' => ui.move_issue_selection_up(),
                        _ => (),
                    },
                    Window::Sprints => match input {
                        b'j' => ui.move_sprint_selection_down(),
                        b'k' => ui.move_sprint_selection_up(),
                        _ => (),
                    },
                    _ => (),
                };
            }
        }
    }
}

// TODO: Allow filtering issues by who is assigned to it
// FIXME: Perform better error handling instead of unwrapping everything.
