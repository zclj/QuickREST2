#[derive(PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Navigations {
    Summary,
    Examples,
    Sequences,
    Invocations,
    Progress,
    Sequencer,
    APIs,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct CentralNavigation {
    pub selected: Navigations,
}

impl CentralNavigation {
    pub fn new() -> Self {
        Self {
            selected: Navigations::Summary,
        }
    }
}

impl Default for CentralNavigation {
    fn default() -> Self {
        Self::new()
    }
}
