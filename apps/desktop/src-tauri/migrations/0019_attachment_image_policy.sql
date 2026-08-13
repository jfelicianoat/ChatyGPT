ALTER TABLE attachments ADD COLUMN describe_images INTEGER
    CHECK (describe_images IS NULL OR describe_images IN (0, 1));

DROP INDEX IF EXISTS idx_attachments_sha256;
CREATE INDEX idx_attachments_sha256_image_policy
    ON attachments(sha256, describe_images)
    WHERE sha256 IS NOT NULL;
