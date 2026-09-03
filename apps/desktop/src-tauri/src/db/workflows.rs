//! Workflows: definicion, publicacion y versiones congeladas.

use super::*;

impl Database {
    pub fn create_workflow(
        &self,
        name: &str,
        project_id: Option<&str>,
    ) -> Result<WorkflowView, AppError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 120 {
            return Err(AppError::Validation(
                "el nombre del flujo debe tener entre 1 y 120 caracteres".to_owned(),
            ));
        }
        let connection = self.connect()?;
        if let Some(project_id) = project_id {
            let exists: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL)",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        let id = format!("workflow_{}", Uuid::new_v4().simple());
        let input_id = format!("node_{}", Uuid::new_v4().simple());
        let result_id = format!("node_{}", Uuid::new_v4().simple());
        let definition = WorkflowDefinition {
            nodes: vec![
                WorkflowNode {
                    id: input_id.clone(),
                    kind: "input".to_owned(),
                    label: "Entrada".to_owned(),
                    x: 70.0,
                    y: 170.0,
                    custom_gpt_id: None,
                    custom_gpt_version_id: None,
                    custom_gpt_name: None,
                    custom_gpt_icon_ref: None,
                    custom_gpt_instructions: None,
                    preferred_model: None,
                    execution_profile: None,
                    context_profile: "balanced".to_owned(),
                    custom_gpt_memory_ids: Vec::new(),
                    custom_gpt_attachment_ids: Vec::new(),
                    instruction: None,
                    attachment_ids: Vec::new(),
                },
                WorkflowNode {
                    id: result_id.clone(),
                    kind: "result".to_owned(),
                    label: "Resultado".to_owned(),
                    x: 650.0,
                    y: 170.0,
                    custom_gpt_id: None,
                    custom_gpt_version_id: None,
                    custom_gpt_name: None,
                    custom_gpt_icon_ref: None,
                    custom_gpt_instructions: None,
                    preferred_model: None,
                    execution_profile: None,
                    context_profile: "balanced".to_owned(),
                    custom_gpt_memory_ids: Vec::new(),
                    custom_gpt_attachment_ids: Vec::new(),
                    instruction: None,
                    attachment_ids: Vec::new(),
                },
            ],
            edges: vec![WorkflowEdge {
                id: format!("edge_{}", Uuid::new_v4().simple()),
                source: input_id.clone(),
                target: result_id,
            }],
            project_context: None,
        };
        let definition_json = serde_json::to_string(&definition)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        connection.execute(
            "INSERT INTO workflows(id, name, project_id, draft_definition_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, name, project_id, definition_json],
        )?;
        self.workflow_view(&id)
    }

    pub fn list_workflows(&self) -> Result<Vec<WorkflowSummary>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT workflow.id, workflow.name, workflow.description, workflow.project_id,
                    version.version_no,
                    json_array_length(json_extract(workflow.draft_definition_json, '$.nodes')),
                    workflow.updated_at
             FROM workflows workflow
             LEFT JOIN workflow_versions version ON version.id = workflow.published_version_id
             WHERE workflow.archived_at IS NULL
             ORDER BY workflow.updated_at DESC, workflow.name COLLATE NOCASE",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(WorkflowSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    project_id: row.get(3)?,
                    published_version_no: row.get(4)?,
                    node_count: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn workflow_view(&self, id: &str) -> Result<WorkflowView, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT workflow.id, workflow.name, workflow.description, workflow.project_id,
                        version.version_no, workflow.draft_definition_json, workflow.updated_at
                 FROM workflows workflow
                 LEFT JOIN workflow_versions version ON version.id = workflow.published_version_id
                 WHERE workflow.id = ?1 AND workflow.archived_at IS NULL",
                params![id],
                |row| {
                    let definition_json: String = row.get(5)?;
                    let definition: WorkflowDefinition = serde_json::from_str(&definition_json)
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                5,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(WorkflowView {
                        summary: WorkflowSummary {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            description: row.get(2)?,
                            project_id: row.get(3)?,
                            published_version_no: row.get(4)?,
                            node_count: definition.nodes.len() as i64,
                            updated_at: row.get(6)?,
                        },
                        definition,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("flujo {id}")))
    }

    pub fn update_workflow(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        project_id: Option<&str>,
        definition: &WorkflowDefinition,
    ) -> Result<WorkflowView, AppError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 120 {
            return Err(AppError::Validation(
                "el nombre del flujo debe tener entre 1 y 120 caracteres".to_owned(),
            ));
        }
        let definition_json = serde_json::to_string(definition)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let connection = self.connect()?;
        let changed = connection.execute(
            "UPDATE workflows
             SET name = ?2, description = ?3, project_id = ?4,
                 draft_definition_json = ?5, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![id, name, description, project_id, definition_json],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("flujo {id}")));
        }
        self.workflow_view(id)
    }

    pub fn publish_workflow(&self, id: &str) -> Result<WorkflowView, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let (draft_definition_json, project_id): (String, Option<String>) = transaction
            .query_row(
                "SELECT draft_definition_json, project_id FROM workflows
                 WHERE id = ?1 AND archived_at IS NULL",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("flujo {id}")))?;
        let mut definition: WorkflowDefinition = serde_json::from_str(&draft_definition_json)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        definition.project_context = if let Some(project_id) = project_id {
            let project = self.project_summary(&project_id)?;
            let memory = self.memory_overview()?;
            let mut used_characters = 0_usize;
            let memory_ids = if memory.enabled {
                memory
                    .items
                    .into_iter()
                    .filter(|item| item.enabled && item.project_id.as_deref() == Some(&project_id))
                    .filter(|item| {
                        used_characters += item.content.chars().count();
                        used_characters <= 8_000
                    })
                    .take(20)
                    .map(|item| item.id)
                    .collect()
            } else {
                Vec::new()
            };
            Some(WorkflowProjectContext {
                project_id,
                project_name: project.name,
                instructions: project.instructions,
                memory_ids,
            })
        } else {
            None
        };
        for node in &mut definition.nodes {
            if node.kind == "custom_gpt" {
                let custom_gpt_id = node.custom_gpt_id.as_deref().ok_or_else(|| {
                    AppError::Validation(format!(
                        "el nodo «{}» no tiene un GPT seleccionado",
                        node.label
                    ))
                })?;
                let context = self.custom_gpt_context(custom_gpt_id)?;
                node.custom_gpt_version_id = Some(context.version_id);
                node.custom_gpt_name = Some(context.name);
                node.custom_gpt_icon_ref = Some(context.icon_ref);
                node.custom_gpt_instructions = Some(context.instructions);
                node.preferred_model = context.preferred_model;
                node.execution_profile = context.execution_profile;
                node.context_profile = context.context_profile.clone();
                let (memory_limit, memory_characters) = match context.context_profile.as_str() {
                    "focused" => (5, 2_000),
                    "broad" => (30, 16_000),
                    _ => (20, 8_000),
                };
                let mut used_characters = 0_usize;
                node.custom_gpt_memory_ids = self
                    .custom_gpt_knowledge(custom_gpt_id)?
                    .into_iter()
                    .filter(|item| item.enabled)
                    .filter(|item| {
                        used_characters += item.content.chars().count();
                        used_characters <= memory_characters
                    })
                    .take(memory_limit)
                    .map(|item| item.id)
                    .collect();
                node.custom_gpt_attachment_ids = self
                    .list_custom_gpt_files(custom_gpt_id)?
                    .into_iter()
                    .filter(|file| {
                        file.ingestion_status == "ready" && file.broker_file_id.is_some()
                    })
                    .map(|file| file.id)
                    .collect();
                let total_files = node
                    .attachment_ids
                    .iter()
                    .chain(node.custom_gpt_attachment_ids.iter())
                    .collect::<HashSet<_>>()
                    .len();
                if total_files > 20 {
                    return Err(AppError::Validation(format!(
                        "el nodo «{}» supera el límite de 20 archivos al sumar los del proyecto y los del GPT",
                        node.label
                    )));
                }
            }
        }
        let definition_json = serde_json::to_string(&definition)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let version_no: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(version_no), 0) + 1 FROM workflow_versions WHERE workflow_id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        let version_id = format!("workflow_version_{}", Uuid::new_v4().simple());
        transaction.execute(
            "INSERT INTO workflow_versions(id, workflow_id, version_no, definition_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![version_id, id, version_no, definition_json],
        )?;
        transaction.execute(
            "UPDATE workflows SET published_version_id = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![id, version_id],
        )?;
        transaction.commit()?;
        self.workflow_view(id)
    }
}
