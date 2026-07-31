---
type: "query"
date: "2026-07-28T16:38:03.808682+00:00"
question: "Estoy subiendo un pdf de 2000 y pico paginas, en el broker he puesto un limite de 5000 paginas, sin embargo, la subida me da error: El PDF supera el límite de páginas. Tiene 2.204 páginas y el Broker admite 2.000 por conversión."
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "attachment_runtime.rs", "AttachmentFailureGuidance", "retrying_failed_attachment_discards_terminal_broker_file_id()"]
---

# Q: Estoy subiendo un pdf de 2000 y pico paginas, en el broker he puesto un limite de 5000 paginas, sin embargo, la subida me da error: El PDF supera el límite de páginas. Tiene 2.204 páginas y el Broker admite 2.000 por conversión.

## Answer

Expanded via graph vocabulary: [attachment, broker, error, failure, guidance, ingestion, pdf, retry]. The persisted CONVERSION_FAILED came from an old Broker file conversion with max_num_pages=2000. ChatyGPT deduplicated the PDF by SHA-256 and, when the same failed file was attached again, kept polling the terminal broker_file_id instead of uploading again. Fixed Database.register_attachment so a reused failed attachment clears broker_file_id, error and ingestion metadata, returns to local, records retry_requested, and triggers a fresh upload. Regression test reattaching_a_failed_file_starts_a_fresh_broker_conversion failed before and passes after. The next fresh Broker result will reveal whether the active ingestion worker truly uses 5000.

## Outcome

- Signal: useful

## Source Nodes

- Database
- attachment_runtime.rs
- AttachmentFailureGuidance
- retrying_failed_attachment_discards_terminal_broker_file_id()