#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize, Clone)]
pub enum Property {
    ResponseCheck,
}

impl std::fmt::Display for Property {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResponseCheck => write!(f, "Response Check"),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Fuzzing {
    pub selected: Property,
}

impl Fuzzing {
    pub fn new() -> Self {
        Fuzzing {
            selected: Property::ResponseCheck,
        }
    }
}

impl Default for Fuzzing {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize, Clone)]
pub struct PropertySettings {
    pub operations: Vec<usize>,
    pub selected_operation: Option<usize>,
}

impl PropertySettings {
    pub fn new() -> Self {
        PropertySettings {
            operations: vec![],
            selected_operation: None,
        }
    }
}

impl Default for PropertySettings {
    fn default() -> Self {
        Self::new()
    }
}
