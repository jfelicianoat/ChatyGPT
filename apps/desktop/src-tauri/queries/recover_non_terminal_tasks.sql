UPDATE broker_tasks
SET local_state = 'recovery_pending',
    next_poll_at = datetime('now'),
    updated_at = datetime('now')
WHERE remote_status NOT IN ('completed', 'failed', 'cancelled')
  AND local_state != 'recovery_pending';

