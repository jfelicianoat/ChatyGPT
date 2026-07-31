UPDATE feature_flags
SET enabled = 1,
    rationale = 'Fase 3: catálogo local y versiones de GPTs personales'
WHERE key = 'custom_gpts';
