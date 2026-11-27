use logifos_model::{Formula, ScriptPair, StackRow};
use std::{collections::BTreeMap, path::Path};

/// Prints the operator texts the wire's inventory does not claim, over a
/// case directory of serialized forms — the straggler work list.
#[test]
#[ignore = "work-list probe; run with --ignored --nocapture"]
fn print_unclaimed_operator_texts() {
    let dir = std::env::var("LOGIFOS_PROBE_DIR").map_or_else(
        |_| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus/reference/plurimath-testsuite/derived")
        },
        std::path::PathBuf::from,
    );
    if !dir.is_dir() {
        eprintln!("probe dir not found; skipping");
        return;
    }
    let mut texts: BTreeMap<String, u64> = BTreeMap::new();
    for entry in std::fs::read_dir(&dir).expect("dir reads") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("mml") {
            continue;
        }
        let bytes = std::fs::read(&path).expect("case reads");
        if let Ok(document) = logifos_syntax_mathml::parse(&bytes) {
            if let Ok(formula) = logifos_syntax_mathml::transform(&document) {
                walk(&formula, &mut texts);
            }
        }
    }
    for (text, count) in &texts {
        println!("{count:5}  {text:?}");
    }
}

fn walk(formula: &Formula, texts: &mut BTreeMap<String, u64>) {
    if let Formula::Operator(text) = formula {
        if formula.to_wire().is_err() {
            *texts.entry(text.clone()).or_default() += 1;
        }
    }
    for child in children(formula) {
        walk(child, texts);
    }
}

fn children(formula: &Formula) -> Vec<&Formula> {
    match formula {
        Formula::Sequence(items)
        | Formula::Style(items)
        | Formula::ErrorGroup(items)
        | Formula::Padded(items)
        | Formula::Phantom(items) => items.iter().collect(),
        Formula::Styled { content, .. }
        | Formula::Fraction(content, _)
        | Formula::Superscript(content, _)
        | Formula::Subscript(content, _)
        | Formula::Under(content, _)
        | Formula::Over(content, _)
        | Formula::Sqrt(content) => {
            let mut out = vec![content.as_ref()];
            if let Formula::Styled { .. } = formula {
                return out;
            }
            match formula {
                Formula::Fraction(_, second)
                | Formula::Superscript(_, second)
                | Formula::Subscript(_, second)
                | Formula::Under(_, second)
                | Formula::Over(_, second) => {
                    out.push(second.as_ref());
                }
                _ => {}
            }
            out
        }
        Formula::SubSup(first, second, third) | Formula::UnderOver(first, second, third) => {
            vec![first.as_ref(), second.as_ref(), third.as_ref()]
        }
        Formula::Root(first, second) => vec![first.as_ref(), second.as_ref()],
        Formula::MultiScripts {
            base,
            postscripts,
            prescripts,
        } => {
            let mut out = vec![base.as_ref()];
            let all = postscripts.iter().chain(prescripts.iter().flatten());
            for ScriptPair {
                subscript,
                superscript,
            } in all
            {
                if let Some(script) = subscript.as_ref() {
                    out.push(script.as_ref());
                }
                if let Some(script) = superscript.as_ref() {
                    out.push(script.as_ref());
                }
            }
            out
        }
        Formula::Table(rows) => rows
            .iter()
            .flat_map(|row| row.iter())
            .flat_map(|cell| match cell {
                Formula::Sequence(items) => items.iter().collect(),
                other => vec![other],
            })
            .collect(),
        Formula::Space(..) | Formula::Menclose { .. } => Vec::new(),
        Formula::Stack(_, rows) => rows
            .iter()
            .flat_map(|row| match row {
                StackRow::Cells { cells, .. } => cells.iter().collect::<Vec<&Formula>>(),
                StackRow::Group { rows, .. } => rows
                    .iter()
                    .flat_map(|inner| match inner {
                        StackRow::Cells { cells, .. } => cells.iter().collect::<Vec<&Formula>>(),
                        StackRow::Group { .. } | StackRow::Line { .. } => Vec::new(),
                        StackRow::Carries { .. } => Vec::new(),
                    })
                    .collect::<Vec<&Formula>>(),
                StackRow::Carries { cells, .. } => cells
                    .iter()
                    .filter_map(|cell| cell.content.as_ref())
                    .collect::<Vec<&Formula>>(),
                StackRow::Line { .. } => Vec::new(),
            })
            .collect(),
        Formula::LongDivision(division) => division
            .children
            .iter()
            .flat_map(|child| match child {
                Formula::Stack(_, rows) => rows
                    .iter()
                    .flat_map(|row| match row {
                        StackRow::Cells { cells, .. } => cells.iter().collect::<Vec<&Formula>>(),
                        StackRow::Carries { cells, .. } => cells
                            .iter()
                            .filter_map(|cell| cell.content.as_ref())
                            .collect::<Vec<&Formula>>(),
                        _ => Vec::new(),
                    })
                    .collect::<Vec<&Formula>>(),
                other => vec![other],
            })
            .collect(),
        Formula::Symbol(_) | Formula::StringLiteral(_) | Formula::Operator(_) => Vec::new(),
    }
}
