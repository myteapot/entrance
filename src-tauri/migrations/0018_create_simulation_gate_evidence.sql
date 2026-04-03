CREATE TABLE IF NOT EXISTS simulation_gate_evidence (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    allocation_id   INTEGER NOT NULL,
    evidence_kind   TEXT NOT NULL CHECK (
        evidence_kind IN (
            'test_result',
            'review_verdict',
            'integration_probe',
            'quality_metric'
        )
    ),
    verdict         TEXT NOT NULL DEFAULT 'pending' CHECK (
        verdict IN ('pending', 'accepted', 'rejected', 'expired')
    ),
    summary         TEXT NOT NULL,
    payload_json    TEXT NOT NULL DEFAULT '{}',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    FOREIGN KEY (allocation_id) REFERENCES nota_runtime_allocations(id)
);

CREATE INDEX IF NOT EXISTS idx_simulation_gate_evidence_allocation
    ON simulation_gate_evidence(allocation_id);

CREATE TABLE IF NOT EXISTS simulation_gate_attempt_receipts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    evidence_id     INTEGER NOT NULL,
    attempt_number  INTEGER NOT NULL CHECK (attempt_number >= 0 AND attempt_number <= 255),
    passed          INTEGER NOT NULL DEFAULT 0 CHECK (passed IN (0, 1)),
    reason          TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    FOREIGN KEY (evidence_id) REFERENCES simulation_gate_evidence(id),
    UNIQUE (evidence_id, attempt_number)
);

CREATE INDEX IF NOT EXISTS idx_simulation_gate_attempt_receipts_evidence
    ON simulation_gate_attempt_receipts(evidence_id);
