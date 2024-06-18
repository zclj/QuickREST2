use qr_explore::amos;
use qr_explore::behaviours;
use qr_explore::exploration_settings;
use qr_explore::explore;

use std::sync::mpsc;
use std::thread::JoinHandle;

pub struct Options {
    pub is_dry_run: bool,
}

// TODO: options and settings should be fixed
pub fn explore(
    target: &explore::Target,
    options: &Options,
    amos: &amos::AMOS,
    //process_events: fn(mpsc::Receiver<explore::Event>),
    behaviour: &behaviours::Behaviour,
    settings: &exploration_settings::StateMutationSettings,
) -> (JoinHandle<()>, mpsc::Receiver<explore::Event>) {
    let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

    let handle = qr_explore::spawn_exploration(
        target,
        options.is_dry_run,
        amos,
        exploration_log_tx.clone(),
        amos.operations.clone(),
        behaviour,
        settings,
    );

    //process_events(exploration_log_rx);
    // .join().expect("Exploration thread panicked")
    (handle, exploration_log_rx)
}
