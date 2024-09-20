use std::thread;

use tracing::info;

pub mod amos;
pub mod amos_buckets;
pub mod amos_generation;
pub mod amos_relations;
pub mod amos_result;
pub mod behaviours;
pub mod exploration_settings;
pub mod explore;
pub mod http_translation;
pub mod meta_properties;
pub mod sequence;
pub mod synthesize;

pub fn spawn_exploration(
    target: &explore::Target,
    is_dry_run: bool,
    amos: &amos::AMOS,
    channel: std::sync::mpsc::Sender<explore::Event>,
    ops: Vec<amos::Operation>,
    behaviour: &behaviours::Behaviour,
    parameters: &exploration_settings::StateMutationSettings,
) -> std::thread::JoinHandle<()> {
    // Resolve operations relevant to this exploration,
    //  i.e., any operations depending on defintions will be
    //  resolved into actual parameters based on the definition
    //let exploration_ops = ops.clone();
    let exploration_ops: Vec<amos::Operation> = ops
        .iter()
        .map(|op| {
            amos.resolve_operation(&op.info.name)
                .expect("Selected operations must be in AMOS")
        })
        .collect();

    let selected_query_ops: Vec<amos::Operation> = parameters
        .query_operation_ids
        .iter()
        .map(|idx| {
            let op = &ops[*idx];
            amos.resolve_operation(&op.info.name)
                .expect("Operation must be in AMOS")
        })
        .collect();

    let number_of_tests = parameters.number_of_tests;
    let min_length = parameters.min_length;
    let max_length = parameters.max_length;

    info!("Explore Behaviour: {:?}", behaviour);

    let bhvr = behaviour.clone();
    let sut_target = target.clone();

    thread::spawn(move || {
        // TODO: this is spread out, fix
        let http_send_fn = if !is_dry_run {
            explore::invoke_with_reqwest
        } else {
            explore::invoke_dry
        };

        let mut context = explore::ExplorationContext {
            http_client: reqwest::blocking::Client::new(),
            http_send_fn,
            target: sut_target,
            query_operation: None,
            tx: Some(channel),
            number_of_tests,
            // TODO: Adapt to the different properties
            min_length,
            max_length,
        };

        let query_ops = selected_query_ops;

        let invoke = explore::invoke;

        match bhvr {
            behaviours::Behaviour::Property => {
                explore::response_check(&context, exploration_ops.clone(), query_ops, invoke)
            }
            behaviours::Behaviour::StateMutation => explore::explore_state_mutation(
                &mut context,
                exploration_ops.clone(),
                &query_ops,
                invoke,
            ),
            behaviours::Behaviour::StateIdentity => explore::explore_state_identity(
                &mut context,
                exploration_ops.clone(),
                &query_ops,
                invoke,
            ),
            behaviours::Behaviour::ResponseEquality => explore::explore_response_equality(
                &context,
                exploration_ops.clone(),
                query_ops,
                invoke,
            ),
            behaviours::Behaviour::ResponseInequality => explore::explore_response_inequality(
                &context,
                exploration_ops.clone(),
                query_ops,
                invoke,
            ),
        };
    })
}
