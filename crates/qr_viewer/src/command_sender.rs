pub enum UICommand {
    Save,
}

pub struct CommandSender {
    ui_sender: std::sync::mpsc::Sender<UICommand>,
}

impl CommandSender {
    pub fn send_ui(&self, command: UICommand) {
        self.ui_sender.send(command).ok();
    }
}

pub struct CommandReceiver {
    ui_receiver: std::sync::mpsc::Receiver<UICommand>,
}

impl CommandReceiver {
    pub fn receive_ui(&self) -> Option<UICommand> {
        self.ui_receiver.try_recv().ok()
    }
}

pub fn command_channel() -> (CommandSender, CommandReceiver) {
    let (ui_sender, ui_receiver) = std::sync::mpsc::channel();
    (CommandSender { ui_sender }, CommandReceiver { ui_receiver })
}
