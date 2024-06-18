#[derive(PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum MainNavigation {
    Exploration,
    Fuzzing,
    Operations,
    Generation,
    Definitions,
    Sequences,
}
