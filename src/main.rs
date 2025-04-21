mod app;
mod config;
mod jira;
pub mod tui;

use std::sync::mpsc;

use app::{App, State, Window};
use config::Config;
use jira::Jira;
use tui::Terminal;

const CTRL_C: u8 = 3;

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

    let terminal = Terminal::try_new().unwrap();
    let mut inputs = terminal.tty().unwrap();
    let jira = Jira::new(&user, &token, host);

    let initial_state = match App::load_state() {
        Some(state) => state,
        None => State::new(&jira, &board_id),
    };

    let mut ui = App::new(terminal, initial_state);

    let (sender, receiver) = mpsc::sync_channel(0);

    // This thread updates the state in the background
    let state_sender = sender.clone();
    std::thread::spawn(move || {
        loop {
            let state = State::new(&jira, &board_id);
            state_sender.send(Event::State(state)).unwrap();
            std::thread::sleep(std::time::Duration::from_secs(30))
        }
    });

    // This thread receive user input in the background
    std::thread::spawn(move || {
        loop {
            let Some(input) = inputs.next().map(|input| input.unwrap()) else {
                break;
            };
            sender.send(Event::Input(input)).unwrap();
        }
    });

    loop {
        ui.render();

        match receiver.recv().unwrap() {
            Event::State(state) => ui.update_state(state),
            Event::Input(input) => {
                // Commands that are independent to the active_window
                match input {
                    b'1' => ui.select_sprints_window(),
                    b'2' => ui.select_issues_window(),
                    b'3' => ui.select_issue_description_window(),
                    b'q' | CTRL_C => {
                        ui.save_state();
                        break;
                    }
                    _ => (),
                };

                // Window-specific commands.
                match ui.active_window {
                    Window::Issues => match input {
                        b'j' => ui.move_issue_selection_down(),
                        b'k' => ui.move_issue_selection_up(),
                        // b'/' => ui.select_filtering_window()
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
