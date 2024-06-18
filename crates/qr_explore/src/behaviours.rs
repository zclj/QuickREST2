#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize, Clone)]
pub enum Behaviour {
    ResponseEquality,
    ResponseInequality,
    StateMutation,
    StateIdentity,
    Property,
}

impl Behaviour {
    // TODO: implement Display
    pub fn presentation(&self) -> String {
        match self {
            Behaviour::ResponseEquality => "Response equality".to_string(),
            Behaviour::ResponseInequality => "Response inequality".to_string(),
            Behaviour::StateMutation => "State mutation".to_string(),
            Behaviour::StateIdentity => "State identity".to_string(),
            Behaviour::Property => "Response Check".to_string(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct BehaviourControl {
    pub selected: Behaviour,
}

impl BehaviourControl {
    pub fn new() -> Self {
        BehaviourControl {
            selected: Behaviour::ResponseEquality,
        }
    }
}

impl Default for BehaviourControl {
    fn default() -> Self {
        Self::new()
    }
}
