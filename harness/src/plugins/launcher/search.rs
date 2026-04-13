use strsim::normalized_levenshtein;

use crate::core::data_store::StoredLauncherApp;

pub fn normalize_text(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    let mut previous_was_space = false;

    for character in input.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            normalized.push(character);
            previous_was_space = false;
        } else if !previous_was_space {
            normalized.push(' ');
            previous_was_space = true;
        }
    }

    normalized.trim().to_string()
}

pub fn score_launcher_app(query: &str, app: &StoredLauncherApp) -> f64 {
    let query = normalize_text(query);
    if query.is_empty() {
        return 0.0;
    }

    let file_name = std::path::Path::new(&app.path)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(normalize_text)
        .unwrap_or_default();
    let path_text = normalize_text(&app.path);
    let candidates = [
        app.normalized_name.as_str(),
        file_name.as_str(),
        path_text.as_str(),
    ];

    let best_score = candidates
        .iter()
        .filter(|candidate| !candidate.is_empty())
        .map(|candidate| score_text_pair(&query, candidate))
        .fold(0.0, f64::max);

    let launch_bonus = (app.launch_count as f64).min(20.0) * 0.01;
    let pin_bonus = if app.pinned { 0.1 } else { 0.0 };

    best_score + launch_bonus + pin_bonus
}

fn score_text_pair(query: &str, candidate: &str) -> f64 {
    if candidate.is_empty() {
        return 0.0;
    }

    if candidate == query {
        return 2.0;
    }

    let compact_query = query.replace(' ', "");
    let compact_candidate = candidate.replace(' ', "");

    let exact_prefix: f64 =
        if candidate.starts_with(query) || compact_candidate.starts_with(&compact_query) {
            1.4
        } else {
            0.0
        };
    let word_prefix: f64 = candidate
        .split(' ')
        .any(|part| part.starts_with(query) || part.starts_with(&compact_query))
        .then_some(1.15)
        .unwrap_or(0.0);
    let subsequence = subsequence_score(&compact_query, &compact_candidate);
    let levenshtein = normalized_levenshtein(&compact_query, &compact_candidate);

    let blended = (subsequence * 0.65) + (levenshtein * 0.35);
    exact_prefix.max(word_prefix).max(blended)
}

fn subsequence_score(query: &str, candidate: &str) -> f64 {
    if query.is_empty() || candidate.is_empty() {
        return 0.0;
    }

    let mut query_chars = query.chars();
    let mut current = match query_chars.next() {
        Some(character) => character,
        None => return 0.0,
    };

    let mut matched = 0usize;
    let mut first_match_index = None;

    for (index, candidate_char) in candidate.chars().enumerate() {
        if candidate_char == current {
            if first_match_index.is_none() {
                first_match_index = Some(index);
            }

            matched += 1;

            match query_chars.next() {
                Some(next_character) => current = next_character,
                None => {
                    let span = (index + 1 - first_match_index.unwrap_or(0)) as f64;
                    let density = matched as f64 / span.max(1.0);
                    let coverage = matched as f64 / query.chars().count() as f64;
                    let start_bonus = if first_match_index == Some(0) {
                        0.15
                    } else {
                        0.0
                    };
                    return (coverage * 0.7) + (density * 0.3) + start_bonus;
                }
            }
        }
    }

    0.0
}

#[cfg(test)]
mod tests {
    use super::{normalize_text, score_launcher_app};
    use crate::core::data_store::StoredLauncherApp;

    fn app(name: &str, path: &str, launch_count: i64) -> StoredLauncherApp {
        StoredLauncherApp {
            id: 1,
            name: name.to_string(),
            normalized_name: normalize_text(name),
            path: path.to_string(),
            arguments: None,
            working_dir: None,
            icon_path: None,
            source: "test".to_string(),
            launch_count,
            last_used: None,
            pinned: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn normalizes_text_for_fuzzy_matching() {
        assert_eq!(normalize_text("Visual-Studio_Code!"), "visual studio code");
    }

    #[test]
    fn subsequence_query_matches_visual_studio_code() {
        let vscode = app(
            "Visual Studio Code",
            r"C:\Program Files\Microsoft VS Code\Code.exe",
            3,
        );
        let calc = app("Calculator", r"C:\Windows\System32\calc.exe", 10);

        let vscode_score = score_launcher_app("vsc", &vscode);
        let calc_score = score_launcher_app("vsc", &calc);

        assert!(vscode_score > calc_score);
        assert!(vscode_score > 0.7);
    }

    #[test]
    fn launch_count_breaks_relevance_ties() {
        let low_usage = app("Terminal", r"C:\Windows\System32\wt.exe", 1);
        let high_usage = app("Terminal", r"C:\Windows\System32\WindowsTerminal.exe", 12);

        assert!(
            score_launcher_app("terminal", &high_usage)
                > score_launcher_app("terminal", &low_usage)
        );
    }
}
