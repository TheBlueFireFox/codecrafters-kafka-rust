#![allow(dead_code)]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::messages::{
    disk::{CompactRecords, RecordValueType},
    primitives::Uuid,
};

pub const FILE_PATH: &str = "__cluster_metadata-0/00000000000000000000.log";
pub const PATH: &str = "/tmp/kraft-combined-logs/";

#[derive(Debug, Clone)]
pub struct Meta {
    path: PathBuf,
    pub rec: CompactRecords,
    map: HashMap<Uuid, String>,
}

impl Meta {
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
    pub fn topic_map(&self) -> &HashMap<Uuid, String> {
        &self.map
    }

    fn generate_topic_map(rec: &CompactRecords) -> HashMap<Uuid, String> {
        let mut map = HashMap::new();
        for batch in &rec.vec {
            for record in &batch.records.vec {
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

    pub fn from_cluster_metadata(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let buf = std::fs::read(path.as_ref().join(FILE_PATH))?;
        let res = CompactRecords::from_buf(&buf)?;
        let map = Self::generate_topic_map(&res);

        Ok(Self {
            rec: res,
            path: path.as_ref().into(),
            map,
        })
    }
}
