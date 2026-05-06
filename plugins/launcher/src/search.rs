use entrance_core::LauncherEntry;

pub fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn score(query: &str, entry: &LauncherEntry) -> f64 {
    let query = normalize(query);
    if query.is_empty() {
        return entry.launch_count as f64 * 0.01 + if entry.pinned { 0.1 } else { 0.0 };
    }

    let name = normalize(&entry.name);
    let command = normalize(&entry.command);

    let base = if name == query {
        2.0
    } else if name.starts_with(&query) {
        1.4
    } else if name.contains(&query) {
        1.0
    } else if command.contains(&query) {
        0.7
    } else {
        subsequence_score(&query, &name)
    };

    base + (entry.launch_count as f64 * 0.01) + if entry.pinned { 0.1 } else { 0.0 }
}

fn subsequence_score(query: &str, candidate: &str) -> f64 {
    if query.is_empty() || candidate.is_empty() {
        return 0.0;
    }

    let mut matched = 0usize;
    let mut chars = query.chars();
    let mut current = match chars.next() {
        Some(ch) => ch,
        None => return 0.0,
    };

    for ch in candidate.chars() {
        if ch == current {
            matched += 1;
            match chars.next() {
                Some(next) => current = next,
                None => break,
            }
        }
    }

    matched as f64 / query.chars().count().max(1) as f64
}
