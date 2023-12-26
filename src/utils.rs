use anyhow::{Context, Error, Result};
use cursive::utils::markup::StyledString;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};
use tempfile::Builder;
use urlencoding::encode;

#[cfg(not(target_family = "windows"))]
use {crate::ActionDescription, skim::prelude::*};

#[cfg(not(target_family = "windows"))]
impl SkimItem for ActionDescription {
    fn text(&self) -> Cow<str> {
        return Cow::Borrowed(self.text);
    }
}

// TODO: render from the bottom
#[cfg(not(target_family = "windows"))]
pub fn fuzzy_actions(actions: Vec<ActionDescription>) -> Option<String> {
    let options = SkimOptionsBuilder::default()
        .height(Some("30%"))
        .build()
        .unwrap();

    let (tx, rx): (SkimItemSender, SkimItemReceiver) = unbounded();
    actions
        .iter()
        .map(|i| tx.send(Arc::new(i.clone())).unwrap())
        // TODO: can this be written better?
        // NOTE: len() optimizes map() out?
        .last();
    drop(tx);

    let out = Skim::run_with(&options, Some(rx))?;
    // FIXME:
    // - skim breaks resizing
    // - skim + flameshow hung

    if out.is_abort {
        return None;
    }

    let selected_items = out.selected_items;
    if selected_items.is_empty() {
        return None;
    }

    // TODO: cast SkimItem to ActionDescription
    return Some(selected_items[0].text().into());
}

pub fn highlight_sql(text: &String) -> Result<StyledString> {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let mut highlighter = syntect::easy::HighlightLines::new(
        syntax_set
            .find_syntax_by_token("sql")
            .context("Cannot load SQL syntax")?,
        &ts.themes["base16-ocean.dark"],
    );
    // NOTE: parse() does not interpret syntect::highlighting::Color::a (alpha/tranparency)
    return cursive_syntect::parse(text, &mut highlighter, &syntax_set)
        .context("Cannot highlight query");
}

pub fn edit_query(query: &String, settings: &HashMap<String, String>) -> Result<String> {
    let mut tmp_file = Builder::new()
        .prefix("chdig-query-")
        .suffix(".sql")
        .rand_bytes(5)
        .tempfile()?;

    tmp_file.write_all(query.as_bytes())?;

    let settings_str = settings
        .iter()
        .map(|kv| format!("\t{}='{}'\n", kv.0, kv.1.replace('\'', "\\\'")))
        .collect::<Vec<String>>()
        .join(",");
    if query.contains("SETTINGS") {
        tmp_file.write_all("\nSETTINGS\n".as_bytes())?;
    } else {
        tmp_file.write_all(",\n".as_bytes())?;
    }
    tmp_file.write_all(settings_str.as_bytes())?;

    let editor = env::var_os("EDITOR").unwrap_or_else(|| "vim".into());
    let tmp_file_path = tmp_file.path().to_str().unwrap();
    let result = Command::new(&editor)
        .arg(tmp_file_path)
        .spawn()
        .map_err(|e| Error::msg(format!("Cannot execute editor {:?} ({})", editor, e)))?
        .wait()?;
    if !result.success() {
        return Err(Error::msg(format!(
            "Editor exited unsuccessfully {:?} ({})",
            editor, result
        )));
    }

    let query = fs::read_to_string(tmp_file_path)?;
    return Ok(query);
}

pub fn open_graph_in_browser(graph: String) -> Result<()> {
    let graph = encode(&graph);
    Command::new("xdg-open")
        .arg(format!(
            "https://dreampuf.github.io/GraphvizOnline/#{}",
            graph
        ))
        // NOTE: avoid breaking of the chdig rendering (though this hides errors...)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()?;
    return Ok(());
}
