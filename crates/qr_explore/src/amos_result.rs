use std::collections::HashSet;

use crate::amos::InvokeResult;

#[allow(dead_code)] // WIP for now
#[derive(Debug)]
pub struct InvocationCoverage {
    covered: Vec<String>,
}

pub fn invocation_coverage(results: &[InvokeResult]) -> InvocationCoverage {
    // Which operations are covered
    let mut covered = HashSet::new();

    for result in results {
        covered.insert(result.operation.name.clone());
    }

    let mut covered = covered.into_iter().collect::<Vec<String>>();
    covered.sort();

    InvocationCoverage { covered }
}

//pub fn example_coverage()
