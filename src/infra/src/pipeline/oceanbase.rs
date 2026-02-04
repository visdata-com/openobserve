use async_trait::async_trait;

use crate::{errors::Result, pipeline::mysql};

pub struct OceanbasePipelineTable {
    mysql_table: mysql::MySqlPipelineTable,
}

impl OceanbasePipelineTable {
    pub fn new() -> Self {
        Self {
            mysql_table: mysql::MySqlPipelineTable::new(),
        }
    }

    pub fn new_legacy() -> Self {
        // legacy flag not needed for pipeline table; keep API parity
        Self::new()
    }
}

impl Default for OceanbasePipelineTable {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl super::PipelineTable for OceanbasePipelineTable {
    async fn create_table(&self) -> Result<()> {
        self.mysql_table.create_table().await
    }

    async fn create_table_index(&self) -> Result<()> {
        self.mysql_table.create_table_index().await
    }

    async fn drop_table(&self) -> Result<()> {
        self.mysql_table.drop_table().await
    }

    async fn put(&self, pipeline: &config::meta::pipeline::Pipeline) -> Result<()> {
        self.mysql_table.put(pipeline).await
    }

    async fn update(&self, pipeline: &config::meta::pipeline::Pipeline) -> Result<()> {
        self.mysql_table.update(pipeline).await
    }

    async fn get_by_stream(&self, stream_params: &config::meta::stream::StreamParams) -> Result<config::meta::pipeline::Pipeline> {
        self.mysql_table.get_by_stream(stream_params).await
    }

    async fn get_by_id(&self, pipeline_id: &str) -> Result<config::meta::pipeline::Pipeline> {
        self.mysql_table.get_by_id(pipeline_id).await
    }

    async fn get_with_same_source_stream(&self, pipeline: &config::meta::pipeline::Pipeline) -> Result<config::meta::pipeline::Pipeline> {
        self.mysql_table.get_with_same_source_stream(pipeline).await
    }

    async fn list(&self) -> Result<Vec<config::meta::pipeline::Pipeline>> {
        self.mysql_table.list().await
    }

    async fn list_by_org(&self, org: &str) -> Result<Vec<config::meta::pipeline::Pipeline>> {
        self.mysql_table.list_by_org(org).await
    }

    async fn list_streams_with_pipeline(&self, org: &str) -> Result<Vec<config::meta::pipeline::Pipeline>> {
        self.mysql_table.list_streams_with_pipeline(org).await
    }

    async fn delete(&self, pipeline_id: &str) -> Result<config::meta::pipeline::Pipeline> {
        self.mysql_table.delete(pipeline_id).await
    }
}
