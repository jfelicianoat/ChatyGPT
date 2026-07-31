ALTER TABLE conversations
ADD COLUMN execution_preferences_json TEXT NOT NULL
DEFAULT '{"dataClassification":"internal","strategy":"single","preset":"fast","maxCostUsd":0.1,"longContext":"fail"}'
CHECK (json_valid(execution_preferences_json));

ALTER TABLE semantic_chat_workflows
ADD COLUMN execution_preferences_json TEXT NOT NULL
DEFAULT '{"dataClassification":"internal","strategy":"single","preset":"fast","maxCostUsd":0.1,"longContext":"fail"}'
CHECK (json_valid(execution_preferences_json));

ALTER TABLE broker_tasks
ADD COLUMN progress_json TEXT NOT NULL DEFAULT '{}'
CHECK (json_valid(progress_json));
