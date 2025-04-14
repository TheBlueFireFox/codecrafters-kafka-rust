#![allow(dead_code)]

use std::{
    collections::HashMap,
    ops::Deref,
    path::{Path, PathBuf},
};

use crate::messages::{
    disk::{self, CompactRecords, RecordValueType},
    primitives::Uuid,
};

pub const FILE_PATH: &str = "__cluster_metadata-0/00000000000000000000.log";
pub const PATH: &str = "/tmp/kraft-combined-logs/";

#[derive(Debug, Clone)]
pub struct Meta {
    path: PathBuf,
    pub rec: CompactRecords,
    maps: Maps,
}

impl Meta {
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn from_cluster_metadata(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let buf = std::fs::read(path.as_ref().join(FILE_PATH))?;
        let rec = CompactRecords::from_buf(&buf)?;
        let maps = Maps::generate_maps(&rec);

        Ok(Self {
            maps,
            rec,
            path: path.as_ref().into(),
        })
    }
}

impl Deref for Meta {
    type Target = Maps;

    fn deref(&self) -> &Self::Target {
        &self.maps
    }
}

#[derive(Debug, Clone)]
pub struct Maps {
    map_uuid_name: HashMap<Uuid, String>,
    map_name_uuid: HashMap<String, Uuid>,
    map_uuid_partition: HashMap<Uuid, Vec<disk::PartitionRecord>>,
}

impl Maps {
    pub fn name_map(&self) -> &HashMap<String, Uuid> {
        &self.map_name_uuid
    }

    pub fn uuid_map(&self) -> &HashMap<Uuid, String> {
        &self.map_uuid_name
    }

    pub fn uuid_partitions(&self) -> &HashMap<Uuid, Vec<disk::PartitionRecord>> {
        &self.map_uuid_partition
    }

    fn generate_maps(rec: &CompactRecords) -> Self {
        let mut uuid_name = HashMap::new();
        let mut name_uuid = HashMap::new();
        let mut uuid_partitions = HashMap::new();

        for batch in &rec.vec {
            for record in &batch.records.vec {
                let record = match &&record.value {
                    Some(r) => r,
                    _ => continue,
                };
                match &record.record_type {
                    RecordValueType::Other(_items) => {}
                    RecordValueType::TopicRecord(topic_record) => {
                        uuid_name.insert(topic_record.uuid, topic_record.name.str.clone());
                        name_uuid.insert(topic_record.name.str.clone(), topic_record.uuid);
                    }
                    RecordValueType::PartitionRecord(partition_record) => {
                        let tuuid = partition_record.topic_uuid;
                        uuid_partitions
                            .entry(tuuid)
                            .or_insert_with(Vec::new)
                            .push(partition_record.clone());
                    }
                }
            }
        }

        Self {
            map_uuid_name: uuid_name,
            map_name_uuid: name_uuid,
            map_uuid_partition: uuid_partitions,
        }
    }
}
