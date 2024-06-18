use serde;

use qr_explore::behaviours::BehaviourControl;
use qr_explore::exploration_settings::ExplorationSettings;
use qr_http_resource::http;

use crate::central_navigation::CentralNavigation;
use crate::fuzzing;
use crate::main_navigation::MainNavigation;
use crate::sequencer;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct AppState {
    pub current_amos_path: Option<String>,

    pub selected_navigation: MainNavigation,

    pub central_navigation: CentralNavigation,

    // Exploration options
    pub exploration_settings: ExplorationSettings,

    // Sequencer
    pub sequencer: sequencer::Sequencer,

    // Behaviours
    pub behaviour: BehaviourControl,

    pub fuzzing: fuzzing::Fuzzing,

    // Exploration Target
    pub target: TargetSettings,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct TargetSettings {
    // TODO: Revisit when there are different types of targets
    pub base_url: String,
    pub protocol: http::Protocol,
    pub port: String,
}

impl TargetSettings {
    pub fn new() -> Self {
        TargetSettings {
            base_url: "".to_string(),
            protocol: http::Protocol::HTTP,
            port: 8080.to_string(),
        }
    }
}

impl Default for TargetSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            current_amos_path: Some("./data/current_amos.amos".to_string()),
            exploration_settings: Default::default(),
            sequencer: Default::default(),
            selected_navigation: MainNavigation::Exploration,
            central_navigation: CentralNavigation::new(),
            behaviour: BehaviourControl::new(),
            fuzzing: fuzzing::Fuzzing::new(),
            target: TargetSettings::new(),
        }
    }

    pub fn load(path: &std::path::Path) -> Self {
        if !path.exists() {
            tracing::info!("No project file found, starting with a new project");
            return AppState::new();
        }

        let content_result = std::fs::read(path);

        match content_result {
            Ok(content) => {
                let loaded_result = serde_json::from_slice(&content);

                match loaded_result {
                    Ok(app_state) => app_state,
                    Err(_) => {
                        tracing::info!("Could not load project file, starting with new project");
                        AppState::new()
                    }
                }
            }
            Err(_) => {
                tracing::info!("Could not load project file, starting with new project");
                AppState::new()
            }
        }
    }

    pub fn save(&self, path: &std::path::Path) {
        // TODO: Improve this. For example, create the dir if it do
        //  not exist
        let payload = serde_json::to_string_pretty(&self);

        match payload {
            Ok(p) => match std::fs::write(path, p) {
                Ok(()) => tracing::info!("Application state saved"),
                Err(e) => tracing::error!("Failed to write file: {}", e),
            },
            Err(e) => panic!("Failed to serialize: {}", e),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
