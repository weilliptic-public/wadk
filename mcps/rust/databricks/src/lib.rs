
use serde::{Deserialize, Serialize};
use weil_macros::{constructor, query, smart_contract, WeilType};
use weil_rs::config::Secrets;

mod auth;
mod sql;
mod dbfs;
mod cluster;
mod model_registry;
mod model_serving;
mod job;
mod catalog;
mod functions;
mod pipeline;

use auth::AuthClient;
use sql::SqlClient;
use dbfs::DbfsClient;
use cluster::ClusterClient;
use model_registry::ModelRegistryClient;
use model_serving::ModelServingClient;
use job::JobClient;
use catalog::CatalogClient;
use functions::FunctionsClient;
use pipeline::PipelineClient;


#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct DatabricksConfig {
    pat_token: String,
    workspace_url: String,
}

trait Databricks {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    async fn list_users(&self) -> Result<String, String>;
    async fn get_user(&self, user_id: String) -> Result<String, String>;
    async fn create_user(&self, username: String, email: String, display_name: Option<String>) -> Result<String, String>;
    async fn execute_sql(&self, query_str: String, warehouse_id: String) -> Result<String, String>;
    async fn list_sql_warehouses(&self, warehouse_id: String) -> Result<String, String>;
    async fn start_sql_warehouse(&self, warehouse_id: String) -> Result<String, String>;
    async fn stop_sql_warehouse(&self, warehouse_id: String) -> Result<String, String>;
    async fn create_sql_warehouse(&self, name: String, cluster_size: String, min_num_clusters: i32, max_num_clusters: i32, auto_stop_mins: i32) -> Result<String, String>;
    async fn list_dbfs_files(&self, path: String) -> Result<String, String>;
    async fn get_dbfs_file_info(&self, path: String) -> Result<String, String>;
    async fn delete_dbfs_file(&self, path: String) -> Result<String, String>;
    async fn move_dbfs_file(&self, source_path: String, destination_path: String) -> Result<String, String>;
    async fn copy_dbfs_file(&self, source_path: String, destination_path: String) -> Result<String, String>;
    async fn write_dbfs_file(&self, path: String, content: String, overwrite: bool) -> Result<String, String>;
    async fn read_dbfs_file(&self, path: String, offset: Option<i64>, length: Option<i64>) -> Result<String, String>;
    async fn list_clusters(&self) -> Result<String, String>;
    async fn get_cluster(&self, cluster_id: String) -> Result<String, String>;
    async fn create_cluster(&self, name: String, spark_version: String, node_type: String, num_workers: i32) -> Result<String, String>;
    async fn list_sql_queries(&self, user_id: String, include_metrics: Option<bool>) -> Result<String, String>;
    async fn create_directory(&self, path: String) -> Result<String, String>;
    async fn list_workspace_directory(&self, path: String) -> Result<String, String>;
    async fn list_registered_models(&self) -> Result<String, String>;
    async fn get_registered_model(&self, name: String) -> Result<String, String>;
    async fn create_registered_model(&self, name: String, description: Option<String>) -> Result<String, String>;
    async fn list_model_versions(&self, name: String) -> Result<String, String>;
    async fn get_model_version(&self, name: String, version: String) -> Result<String, String>;
    async fn set_model_version_stage(&self, name: String, version: String, stage: String) -> Result<String, String>;
    async fn delete_registered_model(&self, name: String) -> Result<String, String>;
    async fn list_serving_endpoints(&self) -> Result<String, String>;
    async fn get_serving_endpoint(&self, name: String) -> Result<String, String>;
    async fn create_serving_endpoint(&self, name: String, configuration: String) -> Result<String, String>;
    async fn update_serving_endpoint(&self, name: String, configuration: String) -> Result<String, String>;
    async fn delete_serving_endpoint(&self, name: String) -> Result<String, String>;
    async fn get_serving_endpoint_logs(&self, name: String, lines: Option<i32>) -> Result<String, String>;
    async fn query_serving_endpoint(&self, name: String, data: String) -> Result<String, String>;
    async fn list_jobs(&self) -> Result<String, String>;
    async fn get_job(&self, job_id: String) -> Result<String, String>;
    async fn run_job_now(&self, job_id: String) -> Result<String, String>;
    async fn get_job_run(&self, run_id: String) -> Result<String, String>;
    async fn cancel_job_run(&self, run_id: String) -> Result<String, String>;
    async fn create_sql_alert(&self, name: String, query_id: String, column: String, op: String, threshold: String, rearm: i32) -> Result<String, String>;
    async fn list_catalogs(&self) -> Result<String, String>;
    async fn get_catalog(&self, catalog_name: String) -> Result<String, String>;
    async fn list_schemas(&self, catalog_name: String) -> Result<String, String>;
    async fn get_schema(&self, catalog_name: String, schema_name: String) -> Result<String, String>;
    async fn list_tables(&self, catalog_name: String, schema_name: String) -> Result<String, String>;
    async fn get_table(&self, catalog_name: String, schema_name: String, table_name: String) -> Result<String, String>;
    async fn list_metastores(&self) -> Result<String, String>;
    async fn list_functions(&self, catalog_name: String, schema_name: String) -> Result<String, String>;
    async fn get_function(&self, function_name: String) -> Result<String, String>;
    async fn create_function(&self, name: String, catalog_name: String, schema_name: String, input_params: String, data_type: String, language: String, routine_definition: String) -> Result<String, String>;
    async fn delete_function(&self, function_name: String) -> Result<String, String>;
    async fn list_pipelines(&self) -> Result<String, String>;
    async fn create_pipeline(&self, name: String, catalog: String, target: String, notebook_path: String, continuous: bool) -> Result<String, String>;
    async fn get_pipeline(&self, pipeline_id: String) -> Result<String, String>;
    async fn update_pipeline(&self, pipeline_id: String, name: Option<String>, catalog: Option<String>, target: Option<String>, notebook_path: Option<String>, continuous: Option<bool>) -> Result<String, String>;
    async fn delete_pipeline(&self, pipeline_id: String) -> Result<String, String>;
    async fn execute_pipeline(&self, pipeline_id: String) -> Result<String, String>;
    async fn get_pipeline_events(&self, pipeline_id: String) -> Result<String, String>;
    
    fn tools(&self) -> String;
    fn prompts(&self) -> String;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct DatabricksContractState {
    // define your contract state here!
    secrets: Secrets<DatabricksConfig>,
}

#[smart_contract]
impl Databricks for DatabricksContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self{
            secrets: Secrets::new(),
        })
    }


    #[query]
    async fn list_users(&self) -> Result<String, String> {
        let config = self.secrets.config();
        let auth_client = AuthClient::new(&config.workspace_url, &config.pat_token);
        auth_client.list_users().await
    }

    #[query]
    async fn get_user(&self, user_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let auth_client = AuthClient::new(&config.workspace_url, &config.pat_token);
        auth_client.get_user(user_id).await
    }

    #[query]
    async fn create_user(&self, username: String, email: String, display_name: Option<String>) -> Result<String, String> {
        let config = self.secrets.config();
        let auth_client = AuthClient::new(&config.workspace_url, &config.pat_token);
        auth_client.create_user(username, email, display_name).await
    }

    #[query]
    async fn execute_sql(&self, query_str: String, warehouse_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let sql_client = SqlClient::new(&config.workspace_url, &config.pat_token);
        sql_client.execute_sql(query_str, warehouse_id).await
    }

    #[query]
    async fn list_sql_warehouses(&self, warehouse_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let sql_client = SqlClient::new(&config.workspace_url, &config.pat_token);
        sql_client.list_sql_warehouses(warehouse_id).await
    }

    #[query]
    async fn start_sql_warehouse(&self, warehouse_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let sql_client = SqlClient::new(&config.workspace_url, &config.pat_token);
        sql_client.start_sql_warehouse(warehouse_id).await
    }

    #[query]
    async fn stop_sql_warehouse(&self, warehouse_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let sql_client = SqlClient::new(&config.workspace_url, &config.pat_token);
        sql_client.stop_sql_warehouse(warehouse_id).await
    }

    #[query]
    async fn create_sql_warehouse(&self, name: String, cluster_size: String, min_num_clusters: i32, max_num_clusters: i32, auto_stop_mins: i32) -> Result<String, String> {
        let config = self.secrets.config();
        let sql_client = SqlClient::new(&config.workspace_url, &config.pat_token);
        sql_client.create_sql_warehouse(name, cluster_size, min_num_clusters, max_num_clusters, auto_stop_mins).await
    }

    #[query]
    async fn list_dbfs_files(&self, path: String) -> Result<String, String> {
        let config = self.secrets.config();
        let dbfs_client = DbfsClient::new(&config.workspace_url, &config.pat_token);
        dbfs_client.list_dbfs_files(path).await
    }

    #[query]
    async fn get_dbfs_file_info(&self, path: String) -> Result<String, String> {
        let config = self.secrets.config();
        let dbfs_client = DbfsClient::new(&config.workspace_url, &config.pat_token);
        dbfs_client.get_dbfs_file_info(path).await
    }

    #[query]
    async fn delete_dbfs_file(&self, path: String) -> Result<String, String> {
        let config = self.secrets.config();
        let dbfs_client = DbfsClient::new(&config.workspace_url, &config.pat_token);
        dbfs_client.delete_dbfs_file(path).await
    }

    #[query]
    async fn move_dbfs_file(&self, source_path: String, destination_path: String) -> Result<String, String> {
        let config = self.secrets.config();
        let dbfs_client = DbfsClient::new(&config.workspace_url, &config.pat_token);
        dbfs_client.move_dbfs_file(source_path, destination_path).await
    }

    #[query]
    async fn copy_dbfs_file(&self, source_path: String, destination_path: String) -> Result<String, String> {
        let config = self.secrets.config();
        let dbfs_client = DbfsClient::new(&config.workspace_url, &config.pat_token);
        dbfs_client.copy_dbfs_file(source_path, destination_path).await
    }

    #[query]
    async fn write_dbfs_file(&self, path: String, content: String, overwrite: bool) -> Result<String, String> {
        let config = self.secrets.config();
        let dbfs_client = DbfsClient::new(&config.workspace_url, &config.pat_token);
        dbfs_client.write_dbfs_file(path, content, overwrite).await
    }

    #[query]
    async fn read_dbfs_file(&self, path: String, offset: Option<i64>, length: Option<i64>) -> Result<String, String> {
        let config = self.secrets.config();
        let dbfs_client = DbfsClient::new(&config.workspace_url, &config.pat_token);
        dbfs_client.read_dbfs_file(path, offset, length).await
    }

    #[query]
    async fn list_clusters(&self) -> Result<String, String> {
        let config = self.secrets.config();
        let cluster_client = ClusterClient::new(&config.workspace_url, &config.pat_token);
        cluster_client.list_clusters().await
    }

    #[query]
    async fn get_cluster(&self, cluster_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let cluster_client = ClusterClient::new(&config.workspace_url, &config.pat_token);
        cluster_client.get_cluster(cluster_id).await
    }

    #[query]
    async fn create_cluster(&self, name: String, spark_version: String, node_type: String, num_workers: i32) -> Result<String, String> {
        let config = self.secrets.config();
        let cluster_client = ClusterClient::new(&config.workspace_url, &config.pat_token);
        cluster_client.create_cluster(name, spark_version, node_type, num_workers).await
    }

    #[query]
    async fn list_sql_queries(&self, user_id: String, include_metrics: Option<bool>) -> Result<String, String> {
        let config = self.secrets.config();
        let sql_client = SqlClient::new(&config.workspace_url, &config.pat_token);
        sql_client.list_sql_queries(user_id, include_metrics).await
    }

    #[query]
    async fn create_directory(&self, path: String) -> Result<String, String> {
        let config = self.secrets.config();
        let dbfs_client = DbfsClient::new(&config.workspace_url, &config.pat_token);
        dbfs_client.create_directory(path).await
    }

    #[query]
    async fn list_workspace_directory(&self, path: String) -> Result<String, String> {
        let config = self.secrets.config();
        let dbfs_client = DbfsClient::new(&config.workspace_url, &config.pat_token);
        dbfs_client.list_workspace_directory(path).await
    }

    #[query]
    async fn list_registered_models(&self) -> Result<String, String> {
        let config = self.secrets.config();
        let model_registry_client = ModelRegistryClient::new(&config.workspace_url, &config.pat_token);
        model_registry_client.list_registered_models().await
    }

    #[query]
    async fn get_registered_model(&self, name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let model_registry_client = ModelRegistryClient::new(&config.workspace_url, &config.pat_token);
        model_registry_client.get_registered_model(name).await
    }

    #[query]
    async fn create_registered_model(&self, name: String, description: Option<String>) -> Result<String, String> {
        let config = self.secrets.config();
        let model_registry_client = ModelRegistryClient::new(&config.workspace_url, &config.pat_token);
        model_registry_client.create_registered_model(name, description).await
    }

    #[query]
    async fn list_model_versions(&self, name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let model_registry_client = ModelRegistryClient::new(&config.workspace_url, &config.pat_token);
        model_registry_client.list_model_versions(name).await
    }

    #[query]
    async fn get_model_version(&self, name: String, version: String) -> Result<String, String> {
        let config = self.secrets.config();
        let model_registry_client = ModelRegistryClient::new(&config.workspace_url, &config.pat_token);
        model_registry_client.get_model_version(name, version).await
    }

    #[query]
    async fn set_model_version_stage(&self, name: String, version: String, stage: String) -> Result<String, String> {
        let config = self.secrets.config();
        let model_registry_client = ModelRegistryClient::new(&config.workspace_url, &config.pat_token);
        model_registry_client.set_model_version_stage(name, version, stage).await
    }

    #[query]
    async fn delete_registered_model(&self, name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let model_registry_client = ModelRegistryClient::new(&config.workspace_url, &config.pat_token);
        model_registry_client.delete_registered_model(name).await
    }

    #[query]
    async fn list_serving_endpoints(&self) -> Result<String, String> {
        let config = self.secrets.config();
        let model_serving_client = ModelServingClient::new(&config.workspace_url, &config.pat_token);
        model_serving_client.list_serving_endpoints().await
    }

    #[query]
    async fn get_serving_endpoint(&self, name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let model_serving_client = ModelServingClient::new(&config.workspace_url, &config.pat_token);
        model_serving_client.get_serving_endpoint(name).await
    }

    #[query]
    async fn create_serving_endpoint(&self, name: String, configuration: String) -> Result<String, String> {
        let secrets_config = self.secrets.config();
        let model_serving_client = ModelServingClient::new(&secrets_config.workspace_url, &secrets_config.pat_token);
        let config_json: serde_json::Value = serde_json::from_str(&configuration)
            .map_err(|e| format!("Invalid JSON config: {}", e))?;
        model_serving_client.create_serving_endpoint(name, config_json).await
    }

    #[query]
    async fn update_serving_endpoint(&self, name: String, configuration: String) -> Result<String, String> {
        let secrets_config = self.secrets.config();
        let model_serving_client = ModelServingClient::new(&secrets_config.workspace_url, &secrets_config.pat_token);
        let config_json: serde_json::Value = serde_json::from_str(&configuration)
            .map_err(|e| format!("Invalid JSON config: {}", e))?;
        model_serving_client.update_serving_endpoint(name, config_json).await
    }

    #[query]
    async fn delete_serving_endpoint(&self, name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let model_serving_client = ModelServingClient::new(&config.workspace_url, &config.pat_token);
        model_serving_client.delete_serving_endpoint(name).await
    }

    #[query]
    async fn get_serving_endpoint_logs(&self, name: String, lines: Option<i32>) -> Result<String, String> {
        let config = self.secrets.config();
        let model_serving_client = ModelServingClient::new(&config.workspace_url, &config.pat_token);
        model_serving_client.get_serving_endpoint_logs(name, lines).await
    }

    #[query]
    async fn query_serving_endpoint(&self, name: String, data: String) -> Result<String, String> {
        let config = self.secrets.config();
        let model_serving_client = ModelServingClient::new(&config.workspace_url, &config.pat_token);
        let data_json: serde_json::Value = serde_json::from_str(&data)
            .map_err(|e| format!("Invalid JSON data: {}", e))?;
        model_serving_client.query_serving_endpoint(name, data_json).await
    }

    #[query]
    async fn list_jobs(&self) -> Result<String, String> {
        let config = self.secrets.config();
        let job_client = JobClient::new(&config.workspace_url, &config.pat_token);
        job_client.list_jobs().await
    }

    #[query]
    async fn get_job(&self, job_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let job_client = JobClient::new(&config.workspace_url, &config.pat_token);
        job_client.get_job(job_id).await
    }

    #[query]
    async fn run_job_now(&self, job_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let job_client = JobClient::new(&config.workspace_url, &config.pat_token);
        job_client.run_job_now(job_id).await
    }

    #[query]
    async fn get_job_run(&self, run_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let job_client = JobClient::new(&config.workspace_url, &config.pat_token);
        job_client.get_job_run(run_id).await
    }

    #[query]
    async fn cancel_job_run(&self, run_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let job_client = JobClient::new(&config.workspace_url, &config.pat_token);
        job_client.cancel_job_run(run_id).await
    }

    #[query]
    async fn create_sql_alert(&self, name: String, query_id: String, column: String, op: String, threshold: String, rearm: i32) -> Result<String, String> {
        let config = self.secrets.config();
        let sql_client = SqlClient::new(&config.workspace_url, &config.pat_token);
        sql_client.create_sql_alert(name, query_id, column, op, threshold, rearm).await
    }

    #[query]
    async fn list_catalogs(&self) -> Result<String, String> {
        let config = self.secrets.config();
        let catalog_client = CatalogClient::new(&config.workspace_url, &config.pat_token);
        catalog_client.list_catalogs().await
    }

    #[query]
    async fn get_catalog(&self, catalog_name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let catalog_client = CatalogClient::new(&config.workspace_url, &config.pat_token);
        catalog_client.get_catalog(catalog_name).await
    }

    #[query]
    async fn list_schemas(&self, catalog_name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let catalog_client = CatalogClient::new(&config.workspace_url, &config.pat_token);
        catalog_client.list_schemas(catalog_name).await
    }

    #[query]
    async fn get_schema(&self, catalog_name: String, schema_name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let catalog_client = CatalogClient::new(&config.workspace_url, &config.pat_token);
        catalog_client.get_schema(catalog_name, schema_name).await
    }

    #[query]
    async fn list_tables(&self, catalog_name: String, schema_name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let catalog_client = CatalogClient::new(&config.workspace_url, &config.pat_token);
        catalog_client.list_tables(catalog_name, schema_name).await
    }

    #[query]
    async fn get_table(&self, catalog_name: String, schema_name: String, table_name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let catalog_client = CatalogClient::new(&config.workspace_url, &config.pat_token);
        catalog_client.get_table(catalog_name, schema_name, table_name).await
    }

    #[query]
    async fn list_metastores(&self) -> Result<String, String> {
        let config = self.secrets.config();
        let catalog_client = CatalogClient::new(&config.workspace_url, &config.pat_token);
        catalog_client.list_metastores().await
    }

    #[query]
    async fn list_functions(&self, catalog_name: String, schema_name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let functions_client = FunctionsClient::new(&config.workspace_url, &config.pat_token);
        functions_client.list_functions(&catalog_name, &schema_name).await
    }

    #[query]
    async fn get_function(&self, function_name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let functions_client = FunctionsClient::new(&config.workspace_url, &config.pat_token);
        functions_client.get_function(&function_name).await
    }

    #[query]
    async fn create_function(&self, name: String, catalog_name: String, schema_name: String, input_params: String, data_type: String, language: String, routine_definition: String) -> Result<String, String> {
        let config = self.secrets.config();
        let functions_client = FunctionsClient::new(&config.workspace_url, &config.pat_token);
        
        // Parse input_params JSON string into Vec<FunctionParameter>
        // If input_params is empty array "[]", set to None to avoid API issues
        let params: Vec<functions::FunctionParameter> = serde_json::from_str(&input_params)
            .map_err(|e| format!("Invalid input_params JSON: {}", e))?;
        
        let input_params_option = if params.is_empty() {
            None
        } else {
            Some(params)
        };
        
        let function_info = functions::FunctionInfo {
            name: name.clone(),
            catalog_name,
            schema_name,
            input_params: input_params_option,
            data_type: data_type.clone(),
            full_data_type: data_type,
            parameter_style: "S".to_string(),
            routine_body: "SQL".to_string(),
            routine_definition,
            language,
            is_deterministic: true,
            sql_data_access: "CONTAINS_SQL".to_string(),
            is_null_call: false,
            security_type: "DEFINER".to_string(),
            specific_name: name,
            comment: None,
            properties: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        
        functions_client.create_function(function_info).await
    }

    #[query]
    async fn delete_function(&self, function_name: String) -> Result<String, String> {
        let config = self.secrets.config();
        let functions_client = FunctionsClient::new(&config.workspace_url, &config.pat_token);
        functions_client.delete_function(&function_name).await
    }

    #[query]
    async fn list_pipelines(&self) -> Result<String, String> {
        let config = self.secrets.config();
        let pipeline_client = PipelineClient::new(&config.workspace_url, &config.pat_token);
        pipeline_client.list_pipelines().await
    }

    #[query]
    async fn create_pipeline(&self, name: String, catalog: String, target: String, notebook_path: String, continuous: bool) -> Result<String, String> {
        let config = self.secrets.config();
        let pipeline_client = PipelineClient::new(&config.workspace_url, &config.pat_token);
        
        let request = pipeline::PipelineCreateRequest {
            name,
            libraries: vec![pipeline::Library {
                notebook: pipeline::Notebook {
                    path: notebook_path,
                },
            }],
            catalog,
            target,
            serverless: true,
            continuous,
        };
        
        pipeline_client.create_pipeline(request).await
    }

    #[query]
    async fn get_pipeline(&self, pipeline_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let pipeline_client = PipelineClient::new(&config.workspace_url, &config.pat_token);
        pipeline_client.get_pipeline(pipeline_id).await
    }

    #[query]
    async fn update_pipeline(&self, pipeline_id: String, name: Option<String>, catalog: Option<String>, target: Option<String>, notebook_path: Option<String>, continuous: Option<bool>) -> Result<String, String> {
        let config = self.secrets.config();
        let pipeline_client = PipelineClient::new(&config.workspace_url, &config.pat_token);
        
        let mut request = pipeline::PipelineUpdateRequest {
            name,
            catalog,
            target,
            serverless: Some(true),
            continuous,
            libraries: None,
        };
        
        if let Some(notebook_path) = notebook_path {
            request.libraries = Some(vec![pipeline::Library {
                notebook: pipeline::Notebook {
                    path: notebook_path,
                },
            }]);
        }
        
        pipeline_client.update_pipeline(pipeline_id, request).await
    }

    #[query]
    async fn delete_pipeline(&self, pipeline_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let pipeline_client = PipelineClient::new(&config.workspace_url, &config.pat_token);
        pipeline_client.delete_pipeline(pipeline_id).await
    }

    #[query]
    async fn execute_pipeline(&self, pipeline_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let pipeline_client = PipelineClient::new(&config.workspace_url, &config.pat_token);
        pipeline_client.execute_pipeline(pipeline_id).await
    }

    #[query]
    async fn get_pipeline_events(&self, pipeline_id: String) -> Result<String, String> {
        let config = self.secrets.config();
        let pipeline_client = PipelineClient::new(&config.workspace_url, &config.pat_token);
        pipeline_client.get_pipeline_events(pipeline_id).await
    }


    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "list_users",
      "description": "get all users in databricks\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_user",
      "description": "get a specific user in databricks from id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "user_id": {
            "type": "string",
            "description": "user id\n"
          }
        },
        "required": [
          "user_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_user",
      "description": "Create a new user\n",
      "parameters": {
        "type": "object",
        "properties": {
          "username": {
            "type": "string",
            "description": "name of the user\n"
          },
          "email": {
            "type": "string",
            "description": "email of the user\n"
          },
          "display_name": {
            "type": "string",
            "description": "display name of the user\n"
          }
        },
        "required": [
          "username",
          "email"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "execute_sql",
      "description": "run an sql query on databricks\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "the raw sql to run\n"
          },
          "warehouse_id": {
            "type": "string",
            "description": "the id of the warehouse to run this query in\n"
          }
        },
        "required": [
          "query_str",
          "warehouse_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_sql_warehouses",
      "description": "get all warehouses\n",
      "parameters": {
        "type": "object",
        "properties": {
          "warehouse_id": {
            "type": "string",
            "description": "the id of the warehouse to run this query in\n"
          }
        },
        "required": [
          "warehouse_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "start_sql_warehouse",
      "description": "start a warehouse\n",
      "parameters": {
        "type": "object",
        "properties": {
          "warehouse_id": {
            "type": "string",
            "description": "the id of the warehouse to start\n"
          }
        },
        "required": [
          "warehouse_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "stop_sql_warehouse",
      "description": "start a warehouse\n",
      "parameters": {
        "type": "object",
        "properties": {
          "warehouse_id": {
            "type": "string",
            "description": "the id of the warehouse to stop\n"
          }
        },
        "required": [
          "warehouse_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_sql_warehouse",
      "description": "create a warehouse\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the warehourse\n"
          },
          "cluster_size": {
            "type": "string",
            "description": "cluster size of the warehourse\n"
          },
          "min_num_clusters": {
            "type": "integer",
            "description": "minimum number of clusters\n"
          },
          "max_num_clusters": {
            "type": "integer",
            "description": "maximum number of clusters\n"
          },
          "auto_stop_mins": {
            "type": "integer",
            "description": "time in minutes of inactivity for stopping\n"
          }
        },
        "required": [
          "name",
          "cluster_size",
          "min_num_clusters",
          "max_num_clusters",
          "auto_stop_mins"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_dbfs_files",
      "description": "list files in databricks file system\n",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "the path to look for files in\n"
          }
        },
        "required": [
          "path"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_dbfs_file_info",
      "description": "get file info in databricks file system\n",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "the path to look for the file in\n"
          }
        },
        "required": [
          "path"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_dbfs_file",
      "description": "delete file in databricks file system\n",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "the path to delete\n"
          }
        },
        "required": [
          "path"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "move_dbfs_file",
      "description": "move file in databricks file system\n",
      "parameters": {
        "type": "object",
        "properties": {
          "source_path": {
            "type": "string",
            "description": "the path of the source\n"
          },
          "destination_path": {
            "type": "string",
            "description": "the path of the destination\n"
          }
        },
        "required": [
          "source_path",
          "destination_path"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "copy_dbfs_file",
      "description": "copy file in databricks file system\n",
      "parameters": {
        "type": "object",
        "properties": {
          "source_path": {
            "type": "string",
            "description": "the path of the source\n"
          },
          "destination_path": {
            "type": "string",
            "description": "the path of the destination\n"
          }
        },
        "required": [
          "source_path",
          "destination_path"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "write_dbfs_file",
      "description": "write to a file in databricks file system\n",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "the path of the file\n"
          },
          "content": {
            "type": "string",
            "description": "content to write\n"
          },
          "overwrite": {
            "type": "boolean",
            "description": "whether to overwrite or not\n"
          }
        },
        "required": [
          "path",
          "content",
          "overwrite"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "read_dbfs_file",
      "description": "read from a file in databricks file system\n",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "the path of the file\n"
          },
          "offset": {
            "type": "integer",
            "description": "the offset to read from\n"
          },
          "length": {
            "type": "integer",
            "description": "length to read\n"
          }
        },
        "required": [
          "path"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_clusters",
      "description": "list all clusters\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_cluster",
      "description": "get a cluster\n",
      "parameters": {
        "type": "object",
        "properties": {
          "cluster_id": {
            "type": "string",
            "description": "the id of the cluster\n"
          }
        },
        "required": [
          "cluster_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_cluster",
      "description": "create a cluster\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "the name of the cluster\n"
          },
          "spark_version": {
            "type": "string",
            "description": "the versions of spark\n"
          },
          "node_type": {
            "type": "string",
            "description": "the node type\n"
          },
          "num_workers": {
            "type": "integer",
            "description": "the number of workers\n"
          }
        },
        "required": [
          "name",
          "spark_version",
          "node_type",
          "num_workers"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_sql_queries",
      "description": "list all sql queries\n",
      "parameters": {
        "type": "object",
        "properties": {
          "user_id": {
            "type": "string",
            "description": "id of the user\n"
          },
          "include_metrics": {
            "type": "boolean",
            "description": "whether to include metrics\n"
          }
        },
        "required": [
          "user_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_directory",
      "description": "create a directory in workspace\n",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "the path to create the directory\n"
          }
        },
        "required": [
          "path"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_workspace_directory",
      "description": "list contents of a workspace directory (use this for workspace paths like /Users/...)\n",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "the workspace path to list\n"
          }
        },
        "required": [
          "path"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_registered_models",
      "description": "list all registered models in the model registry\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_registered_model",
      "description": "get details of a specific registered model\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the registered model\n"
          }
        },
        "required": [
          "name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_registered_model",
      "description": "create a new registered model\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the model\n"
          },
          "description": {
            "type": "string",
            "description": "description of the model\n"
          }
        },
        "required": [
          "name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_model_versions",
      "description": "list all versions of a registered model\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the registered model\n"
          }
        },
        "required": [
          "name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_model_version",
      "description": "get details of a specific model version\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the registered model\n"
          },
          "version": {
            "type": "string",
            "description": "version of the model\n"
          }
        },
        "required": [
          "name",
          "version"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "set_model_version_stage",
      "description": "set the stage of a model version (e.g., Production, Staging)\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the registered model\n"
          },
          "version": {
            "type": "string",
            "description": "version of the model\n"
          },
          "stage": {
            "type": "string",
            "description": "stage to set (e.g., Production, Staging, Archived)\n"
          }
        },
        "required": [
          "name",
          "version",
          "stage"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_registered_model",
      "description": "delete a registered model\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the registered model to delete\n"
          }
        },
        "required": [
          "name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_serving_endpoints",
      "description": "list all model serving endpoints\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_serving_endpoint",
      "description": "get details of a specific serving endpoint\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the serving endpoint\n"
          }
        },
        "required": [
          "name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_serving_endpoint",
      "description": "create a new model serving endpoint\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the serving endpoint\n"
          },
          "configuration": {
            "type": "string",
            "description": "JSON configuration for the serving endpoint\n"
          }
        },
        "required": [
          "name",
          "configuration"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "update_serving_endpoint",
      "description": "update configuration of a serving endpoint\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the serving endpoint\n"
          },
          "configuration": {
            "type": "string",
            "description": "JSON configuration for the serving endpoint\n"
          }
        },
        "required": [
          "name",
          "configuration"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_serving_endpoint",
      "description": "delete a serving endpoint\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the serving endpoint to delete\n"
          }
        },
        "required": [
          "name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_serving_endpoint_logs",
      "description": "get logs from a serving endpoint\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the serving endpoint\n"
          },
          "lines": {
            "type": "integer",
            "description": "number of log lines to retrieve\n"
          }
        },
        "required": [
          "name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "query_serving_endpoint",
      "description": "make predictions using a serving endpoint\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the serving endpoint\n"
          },
          "data": {
            "type": "string",
            "description": "JSON data for prediction\n"
          }
        },
        "required": [
          "name",
          "data"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_jobs",
      "description": "list all jobs in databricks\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_job",
      "description": "get details of a specific job\n",
      "parameters": {
        "type": "object",
        "properties": {
          "job_id": {
            "type": "string",
            "description": "the id of the job\n"
          }
        },
        "required": [
          "job_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "run_job_now",
      "description": "run a job now\n",
      "parameters": {
        "type": "object",
        "properties": {
          "job_id": {
            "type": "string",
            "description": "the id of the job to run\n"
          }
        },
        "required": [
          "job_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_job_run",
      "description": "get details of a specific job run\n",
      "parameters": {
        "type": "object",
        "properties": {
          "run_id": {
            "type": "string",
            "description": "the id of the job run\n"
          }
        },
        "required": [
          "run_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "cancel_job_run",
      "description": "cancel a job run\n",
      "parameters": {
        "type": "object",
        "properties": {
          "run_id": {
            "type": "string",
            "description": "the id of the job run to cancel\n"
          }
        },
        "required": [
          "run_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_sql_alert",
      "description": "create a SQL alert\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the alert\n"
          },
          "query_id": {
            "type": "string",
            "description": "id of the query to monitor\n"
          },
          "column": {
            "type": "string",
            "description": "column to monitor\n"
          },
          "op": {
            "type": "string",
            "description": "comparison operator\n"
          },
          "threshold": {
            "type": "string",
            "description": "threshold value\n"
          },
          "rearm": {
            "type": "integer",
            "description": "rearm count (0 for no rearm)\n"
          }
        },
        "required": [
          "name",
          "query_id",
          "column",
          "op",
          "threshold",
          "rearm"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_catalogs",
      "description": "list all catalogs in Unity Catalog\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_catalog",
      "description": "get details of a specific catalog\n",
      "parameters": {
        "type": "object",
        "properties": {
          "catalog_name": {
            "type": "string",
            "description": "name of the catalog\n"
          }
        },
        "required": [
          "catalog_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_schemas",
      "description": "list all schemas in a catalog\n",
      "parameters": {
        "type": "object",
        "properties": {
          "catalog_name": {
            "type": "string",
            "description": "name of the catalog\n"
          }
        },
        "required": [
          "catalog_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_schema",
      "description": "get details of a specific schema\n",
      "parameters": {
        "type": "object",
        "properties": {
          "catalog_name": {
            "type": "string",
            "description": "name of the catalog\n"
          },
          "schema_name": {
            "type": "string",
            "description": "name of the schema\n"
          }
        },
        "required": [
          "catalog_name",
          "schema_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_tables",
      "description": "list all tables in a schema\n",
      "parameters": {
        "type": "object",
        "properties": {
          "catalog_name": {
            "type": "string",
            "description": "name of the catalog\n"
          },
          "schema_name": {
            "type": "string",
            "description": "name of the schema\n"
          }
        },
        "required": [
          "catalog_name",
          "schema_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_table",
      "description": "get details of a specific table\n",
      "parameters": {
        "type": "object",
        "properties": {
          "catalog_name": {
            "type": "string",
            "description": "name of the catalog\n"
          },
          "schema_name": {
            "type": "string",
            "description": "name of the schema\n"
          },
          "table_name": {
            "type": "string",
            "description": "name of the table\n"
          }
        },
        "required": [
          "catalog_name",
          "schema_name",
          "table_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_metastores",
      "description": "list all metastores in Unity Catalog\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_functions",
      "description": "list all functions in a Unity Catalog schema\n",
      "parameters": {
        "type": "object",
        "properties": {
          "catalog_name": {
            "type": "string",
            "description": "name of the catalog\n"
          },
          "schema_name": {
            "type": "string",
            "description": "name of the schema\n"
          }
        },
        "required": [
          "catalog_name",
          "schema_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_function",
      "description": "get details of a specific function\n",
      "parameters": {
        "type": "object",
        "properties": {
          "function_name": {
            "type": "string",
            "description": "full name of the function (catalog.schema.function)\n"
          }
        },
        "required": [
          "function_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_function",
      "description": "create a new function in Unity Catalog\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the function\n"
          },
          "catalog_name": {
            "type": "string",
            "description": "name of the catalog\n"
          },
          "schema_name": {
            "type": "string",
            "description": "name of the schema\n"
          },
          "input_params": {
            "type": "string",
            "description": "JSON string of input parameters array like \"[{\\\"name\\\": \\\"param1\\\", \\\"type\\\": \\\"STRING\\\"}, {\\\"name\\\": \\\"param2\\\", \\\"type\\\": \\\"INT\\\"}]\"\n"
          },
          "data_type": {
            "type": "string",
            "description": "return data type of the function\n"
          },
          "language": {
            "type": "string",
            "description": "programming language (PYTHON, SQL, etc.)\n"
          },
          "routine_definition": {
            "type": "string",
            "description": "function routine definition/code\n"
          }
        },
        "required": [
          "name",
          "catalog_name",
          "schema_name",
          "input_params",
          "data_type",
          "language",
          "routine_definition"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_function",
      "description": "delete a function from Unity Catalog\n",
      "parameters": {
        "type": "object",
        "properties": {
          "function_name": {
            "type": "string",
            "description": "full name of the function to delete (catalog.schema.function)\n"
          }
        },
        "required": [
          "function_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_pipelines",
      "description": "list all pipelines\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_pipeline",
      "description": "create a new pipeline\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "name of the pipeline\n"
          },
          "catalog": {
            "type": "string",
            "description": "catalog name for the pipeline\n"
          },
          "target": {
            "type": "string",
            "description": "target schema name\n"
          },
          "notebook_path": {
            "type": "string",
            "description": "notebook path for the pipeline\n"
          },
          "continuous": {
            "type": "boolean",
            "description": "whether to run continuously\n"
          }
        },
        "required": [
          "name",
          "catalog",
          "target",
          "notebook_path",
          "continuous"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_pipeline",
      "description": "get details of a specific pipeline\n",
      "parameters": {
        "type": "object",
        "properties": {
          "pipeline_id": {
            "type": "string",
            "description": "id of the pipeline\n"
          }
        },
        "required": [
          "pipeline_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "update_pipeline",
      "description": "update a pipeline\n",
      "parameters": {
        "type": "object",
        "properties": {
          "pipeline_id": {
            "type": "string",
            "description": "id of the pipeline\n"
          },
          "name": {
            "type": "string",
            "description": "new name of the pipeline (optional)\n"
          },
          "catalog": {
            "type": "string",
            "description": "new catalog name (optional)\n"
          },
          "target": {
            "type": "string",
            "description": "new target schema name (optional)\n"
          },
          "notebook_path": {
            "type": "string",
            "description": "new notebook path (optional)\n"
          },
          "continuous": {
            "type": "boolean",
            "description": "whether to run continuously (optional)\n"
          }
        },
        "required": [
          "pipeline_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_pipeline",
      "description": "delete a pipeline\n",
      "parameters": {
        "type": "object",
        "properties": {
          "pipeline_id": {
            "type": "string",
            "description": "id of the pipeline to delete\n"
          }
        },
        "required": [
          "pipeline_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "execute_pipeline",
      "description": "execute/run a pipeline\n",
      "parameters": {
        "type": "object",
        "properties": {
          "pipeline_id": {
            "type": "string",
            "description": "id of the pipeline to execute\n"
          }
        },
        "required": [
          "pipeline_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_pipeline_events",
      "description": "get events/logs for a pipeline\n",
      "parameters": {
        "type": "object",
        "properties": {
          "pipeline_id": {
            "type": "string",
            "description": "id of the pipeline\n"
          }
        },
        "required": [
          "pipeline_id"
        ]
      }
    }
  }
]"#.to_string()
    }

    #[query]
    fn prompts(&self) -> String {
        r#"{
  "prompts": []
}"#.to_string()
    }
}

