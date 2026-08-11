UPDATE protection_modules
SET configured = TRUE,
    healthy = TRUE,
    config = config || '{"action":"delete","admin_exempt":true,"links":true,"mentions":true}'::JSONB,
    updated_at = NOW()
WHERE module_key = 'link_filter';
