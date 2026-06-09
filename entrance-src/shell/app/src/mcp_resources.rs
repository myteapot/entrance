pub(crate) fn resource_templates() -> serde_json::Value {
    serde_json::json!({
        "resourceTemplates": [
            {
                "uriTemplate": "entrance://issues/{issue_id}",
                "name": "Entrance issue by id",
                "description": "Read one local issue card.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://issues/{issue_id}/control",
                "name": "Entrance issue control by id",
                "description": "Read one local issue control packet.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://issues/{issue_id}/timeline",
                "name": "Entrance issue timeline by id",
                "description": "Read one local issue timeline.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://issues/{issue_id}/transition-policy",
                "name": "Entrance issue transition policy by id",
                "description": "Read one local issue transition policy.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://loops/{loop_id}/control",
                "name": "Entrance loop control by id",
                "description": "Read one Reviewer-ready loop control packet.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://loops/{loop_id}/dashboard",
                "name": "Entrance loop dashboard by id",
                "description": "Read one local loop dashboard.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://loops/{loop_id}/evidence-drilldown",
                "name": "Entrance loop evidence drilldown by id",
                "description": "Read one local loop evidence drilldown.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://loops/{loop_id}/evidence-manifest",
                "name": "Entrance loop evidence manifest by id",
                "description": "Read one local loop evidence manifest.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://loops/{loop_id}/runtime-preflight",
                "name": "Entrance loop runtime preflight by id",
                "description": "Read one local loop runtime preflight.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://loops/{loop_id}/worker-lifecycle",
                "name": "Entrance loop worker lifecycle by id",
                "description": "Read one local loop worker lifecycle.",
                "mimeType": "application/json"
            }
        ]
    })
}

pub(crate) fn resource_spec(uri: &str, name: &str, description: &str) -> serde_json::Value {
    serde_json::json!({
        "uri": uri,
        "name": name,
        "description": description,
        "mimeType": "application/json"
    })
}
