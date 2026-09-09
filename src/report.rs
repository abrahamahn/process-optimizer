//! Human-readable outcomes; the database remains the single detailed audit source.
use crate::model::*;
pub fn render(s: &Session) -> String {
    let mut text = format!("GAME SESSION — {:?}\r\n{}\r\nSession: {}\r\nGaming performance benefit: not measured\r\n\r\n", s.stage, s.plan.game_path, s.id);
    text.push_str("SETTINGS\r\n");
    if s.changes.is_empty() {
        text.push_str("No scheduling settings were changed.\r\n");
    }
    for c in &s.changes {
        text.push_str(&format!(
            "{:?} | PID {} | {:?}: {:?} -> {:?}\r\n{}\r\n{}\r\n\r\n",
            c.state,
            c.target.pid,
            c.before.property(),
            c.before,
            c.applied,
            c.target.path,
            c.detail
        ));
    }
    text.push_str("CLOSE REQUESTS\r\n");
    if s.closed.is_empty() {
        text.push_str("No close requests were recorded.\r\n");
    }
    for c in &s.closed {
        text.push_str(&format!(
            "{:?} | PID {} | {}\r\n{}\r\n\r\n",
            c.state, c.target.pid, c.target.path, c.detail
        ));
    }
    text.push_str("OPTIONAL APP REOPENING\r\n");
    if s.reopened.is_empty() {
        text.push_str("No app reopening has been recorded.\r\n");
    }
    for r in &s.reopened {
        text.push_str(&format!(
            "{:?} | {}\r\n{}\r\n\r\n",
            r.state, r.target.path, r.detail
        ));
    }
    text.push_str("EVENTS\r\n");
    for e in &s.events {
        text.push_str(e);
        text.push_str("\r\n");
    }
    text.push_str("\r\nReopening starts a new process. It does not recover documents, tabs, unsaved work, RAM or VRAM. Unselected helpers may remain.\r\n");
    text
}
