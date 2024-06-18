#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExplorationSettings {
    pub state_mutation: StateMutationSettings,
}

impl ExplorationSettings {
    pub fn new() -> Self {
        ExplorationSettings {
            state_mutation: StateMutationSettings::new(),
        }
    }
}

impl Default for ExplorationSettings {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct StateMutationSettings {
    pub number_of_tests: u16,
    pub min_length: u8,
    pub max_length: u8,
    pub query_operation_ids: Vec<usize>,
    pub selected_query_operation: Option<usize>,
}

impl StateMutationSettings {
    pub fn new() -> Self {
        Self {
            number_of_tests: 100,
            min_length: 1,
            max_length: 2,
            query_operation_ids: vec![],
            selected_query_operation: None,
        }
    }

    pub fn remove_selected_query_operation(&mut self) {
        if let Some(id) = self.selected_query_operation {
            self.query_operation_ids.remove(id);
            if id > 0 {
                self.selected_query_operation = Some(id - 1);
            } else {
                self.selected_query_operation = None;
            }
        }
    }
}

impl Default for StateMutationSettings {
    fn default() -> Self {
        Self::new()
    }
}
