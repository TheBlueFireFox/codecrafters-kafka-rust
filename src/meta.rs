use std::collections::HashMap;

use crate::messages::{
    disk::{CompactRecords, RecordValueType},
    primitives::Uuid,
};

#[derive(Debug, Clone)]
pub struct Meta {
    rec: CompactRecords,
}

impl Meta {
    pub fn topic_map(&self) -> HashMap<Uuid, String> {
        let mut map = HashMap::new();
        for batch in &self.rec.vec.vec {
            for record in &batch.records {
                let record = match &&record.value {
                    Some(r) => r,
                    _ => continue,
                };
                if let RecordValueType::TopicRecord(topic_record) = &record.record_type {
                    map.insert(topic_record.uuid, topic_record.name.str.clone());
                }
            }
        }

        map
    }

    pub fn from_cluster_metadata() -> anyhow::Result<Self> {
        let path = "/tmp/kraft-combined-logs/__cluster_metadata-0/00000000000000000000.log";
        let buf = std::fs::read(path)?;
        let res = CompactRecords::from_cluster_meta(&buf)?;

        Ok(Self { rec: res })
    }
}
