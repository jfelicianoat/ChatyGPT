ALTER TABLE conversations
ADD COLUMN custom_gpt_id TEXT
REFERENCES custom_gpts(id) ON DELETE SET NULL;

ALTER TABLE broker_tasks
ADD COLUMN gpt_version_id TEXT
REFERENCES gpt_versions(id) ON DELETE SET NULL;

ALTER TABLE semantic_chat_workflows
ADD COLUMN custom_gpt_context_json TEXT
CHECK (custom_gpt_context_json IS NULL OR json_valid(custom_gpt_context_json));
