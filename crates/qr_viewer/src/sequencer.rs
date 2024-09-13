use qr_explore::amos;
use qr_explore::{behaviours::Behaviour, exploration_settings::StateMutationSettings};

use crate::fuzzing::{Property, PropertySettings};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SequenceParameter {
    //amos_param_id: usize,
    // TODO: fix for other schema types
    pub template: String,
    pub name: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum SequenceItem {
    Operation {
        amos_op_id: usize,
        parameters: Vec<SequenceParameter>,
    },
    // TODO: there will prob. be different params etc for diff. behaviours
    Behaviour {
        behaviour: Behaviour,
        parameters: StateMutationSettings,
    },
    Fuzzer {
        property: Property,
        settings: PropertySettings,
    },
}

impl SequenceItem {
    pub fn new(id: usize, params: &[amos::Parameter]) -> Self {
        let parameters = params
            .iter()
            .map(|param| SequenceParameter {
                template: "[a-z]*".to_string(),
                name: param.name.clone(),
            })
            .collect();

        SequenceItem::Operation {
            amos_op_id: id,
            parameters,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Sequence {
    pub name: String,
    pub items: Vec<SequenceItem>,
    pub selected: usize,
    pub is_dry_run: bool,
}

impl Sequence {
    pub fn new() -> Self {
        Self {
            name: "New sequence".to_string(),
            items: vec![],
            selected: 0,
            is_dry_run: false,
        }
    }

    pub fn remove_selected(&mut self) {
        if self.selected < self.items.len() {
            self.items.remove(self.selected);

            if self.selected > 0 {
                self.selected -= 1;
            }
        }
    }
}

impl Default for Sequence {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Sequencer {
    pub sequences: Vec<Sequence>,

    pub selected_sequence_id: Option<usize>,
}

// TODO: not sure this API is good, re-evaluate..
impl Sequencer {
    pub fn new() -> Self {
        Sequencer {
            sequences: vec![],
            selected_sequence_id: None,
        }
    }

    pub fn add_new(&mut self) {
        self.sequences.push(Sequence::new())
    }

    pub fn selected_sequence(&self) -> Option<&Sequence> {
        if let Some(id) = self.selected_sequence_id {
            Some(&self.sequences[id])
        } else {
            None
        }
    }

    pub fn selected_sequence_as_mut(&mut self) -> Option<&mut Sequence> {
        if let Some(id) = self.selected_sequence_id {
            Some(&mut self.sequences[id])
        } else {
            None
        }
    }

    pub fn push_item_to_selected(&mut self, item: SequenceItem) {
        if let Some(id) = self.selected_sequence_id {
            self.sequences[id].items.push(item);
        }
    }

    pub fn remove_selected(&mut self) {
        if let Some(selected) = self.selected_sequence_id {
            self.sequences.remove(selected);
            if selected > 0 {
                self.selected_sequence_id = Some(selected - 1);
            } else {
                self.selected_sequence_id = None;
            }
        }
    }
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}
