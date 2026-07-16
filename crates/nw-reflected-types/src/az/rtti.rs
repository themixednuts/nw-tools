use crate::az::uuid::Uuid;

pub trait AzRtti {
    const NAME: &'static str;
    const TYPE_ID: Uuid;
    const BASE_TYPE_IDS: &'static [Uuid] = &[];
}
