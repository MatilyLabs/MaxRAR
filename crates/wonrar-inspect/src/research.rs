//! Static, bounded paper-source hints. No TeX engine or supplied code is executed.
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub entry_id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_start: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchProfile {
    pub kind: String,
    pub archive_identity: String,
    pub title: Option<String>,
    pub main_candidates: Vec<SourceRef>,
    pub bibliographies: Vec<SourceRef>,
    pub figures: Vec<SourceRef>,
    pub inspected_tex_files: usize,
    pub tex_files: usize,
    pub limited: bool,
    pub warnings: Vec<String>,
}

fn extension(path: &str) -> String {
    path.rsplit('.').next().unwrap_or("").to_ascii_lowercase()
}

fn reference(entry: &wonrar_core::Entry) -> SourceRef {
    SourceRef {
        entry_id: entry.id.clone(),
        path: entry.name.clone(),
        line_start: None,
    }
}

// Keep escaped percent signs; strip actual comments for static command discovery.
fn uncomment(text: &str) -> String {
    text.lines()
        .map(|line| {
            let bytes = line.as_bytes();
            for (index, byte) in bytes.iter().enumerate() {
                if *byte == b'%' {
                    let escapes = bytes[..index]
                        .iter()
                        .rev()
                        .take_while(|b| **b == b'\\')
                        .count();
                    if escapes % 2 == 0 {
                        return &line[..index];
                    }
                }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn command_argument(text: &str, command: &str) -> Option<(usize, String)> {
    let marker = format!("\\{command}");
    for (offset, _) in text.match_indices(&marker) {
        let tail = &text[offset + marker.len()..];
        if tail.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            continue;
        }
        let tail = tail.trim_start();
        // Support the common optional short title. This is a hint parser, not TeX expansion.
        let tail = if tail.starts_with('[') {
            tail.split_once(']')?.1.trim_start()
        } else {
            tail
        };
        let Some(tail) = tail.strip_prefix('{') else {
            continue;
        };
        let mut depth = 1usize;
        let mut escaped = false;
        for (index, ch) in tail.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '{' {
                depth += 1;
            }
            if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    let value = tail[..index]
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    if value.is_empty() {
                        break;
                    }
                    return Some((
                        text[..offset].bytes().filter(|b| *b == b'\n').count() + 1,
                        value.chars().take(240).collect(),
                    ));
                }
            }
        }
    }
    None
}

pub fn profile(path: &Path, password: Option<&str>) -> Result<ResearchProfile, String> {
    let info = wonrar_core::inspect_with_password(path, password).map_err(|e| e.to_string())?;
    let mut tex: Vec<_> = info
        .entries
        .iter()
        .filter(|e| !e.is_dir && extension(&e.name) == "tex")
        .collect();
    let named_tex_files = tex.len();
    let unknown_single_source = tex.is_empty()
        && ["gzip", "bzip2", "xz", "zstd"].contains(&info.format.as_str())
        && info.entries.len() == 1
        && !info.entries[0].is_dir;
    if unknown_single_source {
        tex.push(&info.entries[0]);
    }
    let mut result = ResearchProfile {
        kind: if named_tex_files == 0 { "other" } else { "paper" }.into(), archive_identity: info.identity.clone(),
        title: None, main_candidates: Vec::new(), bibliographies: Vec::new(), figures: Vec::new(),
        inspected_tex_files: 0, tex_files: named_tex_files, limited: tex.len() > 16,
        warnings: vec!["Static source hints only: TeX macros are not expanded, and sources are never compiled. A TeX file can also belong to a non-paper project.".into()],
    };
    let mut titles = Vec::new();
    for entry in tex.into_iter().take(16) {
        match wonrar_core::preview_with_password(path, &entry.id, 16 * 1024, password) {
            Ok(preview) => {
                if !unknown_single_source {
                    result.inspected_tex_files += 1;
                }
                result.limited |= preview.truncated;
                let text = uncomment(&preview.text);
                if let Some((line, _)) = command_argument(&text, "documentclass") {
                    if unknown_single_source {
                        result.kind = "paper".into();
                        result.tex_files = 1;
                        result.inspected_tex_files = 1;
                    }
                    let mut source = reference(entry);
                    source.line_start = Some(line);
                    result.main_candidates.push(source);
                    if let Some((_, title)) = command_argument(&text, "title") {
                        titles.push(title);
                    }
                }
            }
            Err(_) => result.limited = true,
        }
    }
    if result.main_candidates.len() == 1 && titles.len() == 1 {
        result.title = titles.pop();
    }
    for entry in info.entries.iter().filter(|e| !e.is_dir) {
        let ext = extension(&entry.name);
        if ["bib", "bbl"].contains(&ext.as_str()) {
            if result.bibliographies.len() < 64 {
                result.bibliographies.push(reference(entry));
            } else {
                result.limited = true;
            }
        }
        if ["png", "jpg", "jpeg", "pdf", "eps", "svg"].contains(&ext.as_str()) {
            if result.figures.len() < 64 {
                result.figures.push(reference(entry));
            } else {
                result.limited = true;
            }
        }
    }
    if wonrar_core::inspect_with_password(path, password)
        .map_err(|e| e.to_string())?
        .identity
        != info.identity
    {
        return Err(
            "Archive changed while identifying paper sources. Reopen it before continuing.".into(),
        );
    }
    if result.limited {
        result.warnings.push("Source inspection is partial (up to 16 TeX files, 16 KiB each, and 64 listed bibliography/figure candidates).".into());
    }
    if result.main_candidates.len() > 1 {
        result
            .warnings
            .push("Multiple potential main documents were found. Select the one you need.".into());
    }
    if !result.figures.is_empty() {
        result.warnings.push("Figure candidates are identified by file type, not verified TeX references; a PDF may be a document rather than a figure.".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comments_commands_and_nested_title_are_static_hints() {
        let text = uncomment("%\\documentclass{wrong}\n\\documentclassname{wrong}\n\\documentclass[11pt]{article}\n\\title[Short]{A {Nested} Title} % trailing\n\\% literal");
        assert_eq!(
            command_argument(&text, "documentclass"),
            Some((3, "article".into()))
        );
        assert_eq!(
            command_argument(&text, "title").unwrap().1,
            "A {Nested} Title"
        );
        assert!(text.contains("\\% literal"));
    }
    #[test]
    fn real_archive_exposes_cited_paper_sources_and_bibliography_search() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("paper");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("main.tex"), "\\documentclass{article}\n\\title{Archive Inspection}\n\\begin{document}\n\\bibliography{references}\n").unwrap();
        std::fs::write(
            source.join("references.bib"),
            "@article{example, title={Evidence Retrieval}, author={Example Author}}\n",
        )
        .unwrap();
        std::fs::write(source.join("plot.png"), b"fixture, no image decoding").unwrap();
        let archive = dir.path().join("paper.zip");
        wonrar_core::create(
            &archive,
            &[source],
            &std::sync::atomic::AtomicBool::new(false),
            &|_| {},
        )
        .unwrap();
        let report = profile(&archive, None).unwrap();
        assert_eq!(report.kind, "paper");
        assert_eq!(report.title.as_deref(), Some("Archive Inspection"));
        assert_eq!(report.main_candidates[0].path, "paper/main.tex");
        assert_eq!(report.main_candidates[0].line_start, Some(1));
        assert_eq!(report.bibliographies.len(), 1);
        assert_eq!(report.figures.len(), 1);
        let answer = crate::ask(&archive, "bibliography references", None).unwrap();
        assert!(answer
            .evidence
            .iter()
            .any(|e| e.path.ends_with("references.bib")));
        let gzip = dir.path().join("source.gz");
        wonrar_core::create(
            &gzip,
            &[dir.path().join("paper/main.tex")],
            &std::sync::atomic::AtomicBool::new(false),
            &|_| {},
        )
        .unwrap();
        let extensionless = dir.path().join("arxiv-source");
        std::fs::copy(&gzip, &extensionless).unwrap();
        let report = profile(&extensionless, None).unwrap();
        assert_eq!(report.kind, "paper");
        assert_eq!(report.title.as_deref(), Some("Archive Inspection"));
        assert_eq!(report.tex_files, 1);
        let answer = crate::ask(&extensionless, "research paper title", None).unwrap();
        assert!(answer
            .evidence
            .iter()
            .any(|e| e.text.contains("Archive Inspection")));
    }
}
