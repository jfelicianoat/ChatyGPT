ALTER TABLE conversation_summaries
    ADD COLUMN source_message_id TEXT;

ALTER TABLE conversation_summaries
    ADD COLUMN source_character_offset INTEGER NOT NULL DEFAULT 0
        CHECK(source_character_offset >= 0);
