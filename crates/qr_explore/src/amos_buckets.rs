use crate::amos::Operation;
use crate::amos::OperationMetaData::HTTP;
use qr_http_resource::http::HTTPMethod;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum BucketKind {
    Create,
    Read,
    Update,
    Delete,
}

#[derive(Debug, Clone)]
pub struct BucketItem {
    pub precedence: u8,
    pub kind: BucketKind,
    pub name: String,
    pub url: String,
    pub method: HTTPMethod,
}

#[derive(Debug, Clone)]
pub struct Buckets {
    pub items: Vec<BucketItem>,
    pub index: HashMap<u8, Vec<usize>>,
}

// GET operations without an update Operation must not be pushed to far
// GET operations with update Operation have a higher yield if pushed
// How can we 'know' this?
// Or should we loop the exploration if the behaviour is not found?
impl Buckets {
    pub fn new(ops: &[Operation]) -> Self {
        let mut items = vec![];

        for op in ops {
            if let Some(meta) = &op.meta_data {
                match meta {
                    HTTP { url, method } => {
                        let init_bucket = url
                            .split('/')
                            .filter(|s| s.starts_with('{'))
                            .collect::<Vec<&str>>()
                            .len();
                        //println!("Bucket split: {:#?}", bucket);

                        let (bucket, kind) = match method {
                            HTTPMethod::GET => {
                                if init_bucket > 0 {
                                    if url.ends_with('}') {
                                        (init_bucket, BucketKind::Read)
                                    } else {
                                        (init_bucket + 1, BucketKind::Read)
                                    }
                                } else {
                                    (init_bucket, BucketKind::Read)
                                }
                            }
                            HTTPMethod::DELETE => (init_bucket + 2, BucketKind::Delete),
                            HTTPMethod::POST => {
                                if init_bucket > 0 {
                                    if url.ends_with('}') {
                                        (init_bucket, BucketKind::Create)
                                    } else {
                                        (init_bucket + 1, BucketKind::Update)
                                    }
                                } else {
                                    (init_bucket, BucketKind::Create)
                                }
                            }
                            HTTPMethod::PUT => {
                                if init_bucket > 0 {
                                    if url.ends_with('}') {
                                        (init_bucket, BucketKind::Update)
                                    } else {
                                        (init_bucket + 1, BucketKind::Update)
                                    }
                                } else {
                                    (init_bucket, BucketKind::Update)
                                }
                            }
                            _ => panic!(),
                        };

                        items.push(BucketItem {
                            precedence: bucket as u8,
                            kind,
                            name: op.info.name.clone(),
                            url: url.clone(),
                            method: method.clone(),
                        })
                    }
                }
            }
        }

        let mut index: HashMap<u8, Vec<usize>> = HashMap::new();
        for (idx, item) in items.iter().enumerate() {
            // Do we have an entry for this bucket?
            if let Some(b) = index.get_mut(&item.precedence) {
                b.push(idx)
            } else {
                index.insert(item.precedence, vec![idx]);
            }
        }

        Buckets { items, index }
    }

    pub fn find(&self, op: &Operation) -> Option<&BucketItem> {
        // TODO: look ups should be id based
        self.items.iter().find(|elt| op.info.name == elt.name)
    }

    pub fn find_create_operations_with_precedence(&self, p_min: u8, p_max: u8) -> Vec<&BucketItem> {
        self.items
            .iter()
            .filter(|item| item.precedence >= p_min && item.precedence <= p_max)
            .filter(|item| matches!(&item.kind, BucketKind::Create))
            .collect()
    }

    pub fn find_delete_operations_with_precedence(&self, p_min: u8, p_max: u8) -> Vec<&BucketItem> {
        self.items
            .iter()
            .filter(|item| item.precedence >= p_min && item.precedence <= p_max)
            .filter(|item| matches!(&item.kind, BucketKind::Delete))
            .collect()
    }

    pub fn find_read_operations_with_precedence(&self, p_min: u8, p_max: u8) -> Vec<&BucketItem> {
        self.items
            .iter()
            .filter(|item| item.precedence >= p_min && item.precedence <= p_max)
            .filter(|item| matches!(&item.kind, BucketKind::Read))
            .collect()
    }

    pub fn find_update_operations_with_precedence(&self, p_min: u8, p_max: u8) -> Vec<&BucketItem> {
        self.items
            .iter()
            .filter(|item| item.precedence >= p_min && item.precedence <= p_max)
            .filter(|item| matches!(&item.kind, BucketKind::Update))
            .collect()
    }

    pub fn bucketize_for_state_identity(&self, bucket_len: u8) -> Vec<Vec<&BucketItem>> {
        // state identity means at least 2, at most 5
        match bucket_len {
            2 => {
                // Q(X) -> C(X) -> Q(X) -> D(X) -> Q(X)
                vec![
                    self.find_create_operations_with_precedence(1, 1),
                    self.find_delete_operations_with_precedence(2, 3),
                    vec![],
                    vec![],
                    vec![],
                ]
            }
            3 => {
                // C(X) -> Q(Y) -> C(Y/X) -> Q(Y) -> D(Y/X) -> Q(X)
                vec![
                    self.find_create_operations_with_precedence(1, 1),
                    self.find_create_operations_with_precedence(2, 2),
                    self.find_delete_operations_with_precedence(2, 4),
                    vec![],
                    vec![],
                ]
            }
            4 => {
                // C(X) -> C(Y) -> Q(Y) -> U(Z/Y) -> Q(Y) -> D(Z/Y) -> Q(Y)
                vec![
                    self.find_create_operations_with_precedence(1, 1),
                    self.find_create_operations_with_precedence(2, 2),
                    self.find_update_operations_with_precedence(2, 3),
                    self.find_delete_operations_with_precedence(4, 4),
                    vec![],
                ]
            }
            5 => {
                // C(X) -> C(Y) -> Q(Y) -> U(Z/Y) -> Q(Y) -> D(Z/Y) -> Q(Y)
                vec![
                    self.find_create_operations_with_precedence(1, 1),
                    self.find_create_operations_with_precedence(2, 2),
                    self.find_create_operations_with_precedence(2, 3),
                    self.find_create_operations_with_precedence(3, 3),
                    self.find_delete_operations_with_precedence(4, 5),
                ]
            }
            _ => panic!("Unsupported bucket length"),
        }
    }
}

// C(X) -> C(Y) -> C(Z) -> Q(X) -> U(X/Y/Z) -> Q(X) -> D(X?Y?Z?) -> Q(X)
pub fn bucketize_for_state_identity_strategy(
    bucket: &Buckets,
    bucket_len: u8,
) -> Vec<Vec<&BucketItem>> {
    // state identity means at least 2, at most 5
    match bucket_len {
        2 => {
            // Q(X) -> C(X) -> Q(X) -> D(X) -> Q(X)
            vec![
                bucket.find_create_operations_with_precedence(1, 1),
                bucket.find_delete_operations_with_precedence(2, 3),
                vec![],
                vec![],
                vec![],
            ]
        }
        3 => {
            // C(X) -> Q(Y) -> C(Y/X) -> Q(Y) -> D(Y/X) -> Q(X)
            vec![
                bucket.find_create_operations_with_precedence(1, 1),
                bucket.find_create_operations_with_precedence(2, 2),
                bucket.find_delete_operations_with_precedence(2, 4),
                vec![],
                vec![],
            ]
        }
        4 => {
            // C(X) -> C(Y) -> Q(Y) -> U(Z/Y) -> Q(Y) -> D(Z/Y) -> Q(Y)
            vec![
                bucket.find_create_operations_with_precedence(1, 1),
                bucket.find_create_operations_with_precedence(2, 2),
                bucket.find_update_operations_with_precedence(2, 3),
                bucket.find_delete_operations_with_precedence(4, 4),
                vec![],
            ]
        }
        5 => {
            // C(X) -> C(Y) -> Q(Y) -> U(Z/Y) -> Q(Y) -> D(Z/Y) -> Q(Y)
            vec![
                bucket.find_create_operations_with_precedence(1, 1),
                bucket.find_create_operations_with_precedence(2, 2),
                bucket.find_create_operations_with_precedence(2, 3),
                bucket.find_create_operations_with_precedence(3, 3),
                bucket.find_delete_operations_with_precedence(4, 5),
            ]
        }
        _ => panic!("Unsupported bucket length"),
    }
}

pub fn bucketize_for_state_identity_update_strategy(
    bucket: &Buckets,
    bucket_len: u8,
) -> Vec<Vec<&BucketItem>> {
    // state identity means at least 2, at most 5
    match bucket_len {
        2 => {
            //panic!("To short sequence for update strategy");
            vec![
                bucket.find_create_operations_with_precedence(1, 1),
                bucket.find_delete_operations_with_precedence(2, 3),
                vec![],
                vec![],
                vec![],
            ]
        }
        3 => {
            // C(X) -> Q(Y) -> C(Y/X) -> Q(Y) -> D(Y/X) -> Q(X)
            vec![
                bucket.find_create_operations_with_precedence(1, 1),
                bucket.find_update_operations_with_precedence(2, 2),
                bucket.find_delete_operations_with_precedence(2, 4),
                vec![],
                vec![],
            ]
        }
        4 => {
            // C(X) -> C(Y) -> Q(Y) -> U(Z/Y) -> Q(Y) -> D(Z/Y) -> Q(Y)
            vec![
                bucket.find_create_operations_with_precedence(1, 1),
                bucket.find_create_operations_with_precedence(2, 2),
                bucket.find_update_operations_with_precedence(2, 3),
                bucket.find_delete_operations_with_precedence(2, 4),
                vec![],
            ]
        }
        5 => {
            // C(X) -> C(Y) -> Q(Y) -> U(Z/Y) -> Q(Y) -> D(Z/Y) -> Q(Y)
            vec![
                bucket.find_create_operations_with_precedence(1, 1),
                bucket.find_create_operations_with_precedence(2, 2),
                bucket.find_create_operations_with_precedence(2, 2),
                bucket.find_update_operations_with_precedence(2, 4),
                bucket.find_delete_operations_with_precedence(2, 5),
            ]
        }
        _ => panic!("Unsupported bucket length"),
    }
}
