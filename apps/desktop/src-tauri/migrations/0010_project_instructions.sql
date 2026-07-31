ALTER TABLE projects ADD COLUMN instructions TEXT;

ALTER TABLE semantic_chat_workflows
ADD COLUMN project_instruction_json TEXT
CHECK (project_instruction_json IS NULL OR json_valid(project_instruction_json));
