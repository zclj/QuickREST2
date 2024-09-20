use proptest::{
    strategy::Strategy,
    test_runner::{Config, FileFailurePersistence, TestRunner},
};

use crate::{
    amos::{InvokeResult, Operation},
    amos_generation::{self, gen_banana_cake_value},
    explore::{self, ControlEvent, Event, ExplorationContext, LogLevel},
    synthesize::synthesize_operation,
};

// EXPERIMENT: make a path UI -> gen
pub fn banana_cakes_generation(expr: &str) -> amos_generation::ParameterValue {
    let mut runner = TestRunner::new(Config {
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..Config::default()
    });

    let gen = gen_banana_cake_value(expr.to_string())
        .new_tree(&mut runner)
        .unwrap();

    gen.current()
}

pub fn sequence_invoke(
    ctx: &ExplorationContext,
    operations: Vec<Operation>,
    invoke: explore::InvokeFn,
    operations_to_invoke: Vec<Operation>,
) -> Option<Vec<InvokeResult>> {
    ctx.publish_event(Event::log(LogLevel::Info, "Start sequence invocation"));

    ctx.publish_event(Event::TimeLineStart {
        enter: std::time::Instant::now(),
        message: "Start invocation".to_string(),
    });

    let mut runner = TestRunner::new(Config {
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..Config::default()
    });

    let mut syn_ops = vec![];
    for op in operations_to_invoke {
        let gen = amos_generation::gen_param_array(&op.parameters)
            .new_tree(&mut runner)
            .unwrap();

        let v = gen.current();

        // NOTE: if references should be supported, we need to potential seq of
        //  operations to reference
        syn_ops.push(synthesize_operation(&[], (op.clone(), v)));
    }

    let results = invoke(ctx, &operations, &syn_ops);

    ctx.publish_event(Event::Control {
        event: ControlEvent::Finished,
    });

    ctx.publish_event(Event::TimeLineEnd {
        time: std::time::Instant::now(),
        message: "Completed invocation".to_string(),
    });

    results
}
