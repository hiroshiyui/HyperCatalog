//! A tiny REPL that drives a `hypercore::Session` so the model + HyperTalk interpreter
//! can be exercised without an Android emulator.
//!
//! Usage: `hyper-desktop <stack.json>` then type commands:
//!   dump            show the current card and its objects
//!   tap <name|id>   tap a button/field by name or id (fires its mouseUp)
//!   tap <x> <y>     tap at card coordinates
//!   type <id> <txt> set a field's text (simulates host editing)
//!   go next|prev|first|last
//!   save <path>     write the stack JSON
//!   quit

use std::io::{self, Write};

use hypercore::Session;

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: hyper-desktop <stack.json>");
            std::process::exit(2);
        }
    };
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("cannot read {path}: {e}");
        std::process::exit(1);
    });
    // YAML for the readable authoring format (.yaml/.yml), JSON otherwise. See ADR-0011.
    let is_yaml = path.ends_with(".yaml") || path.ends_with(".yml");
    let loaded = if is_yaml {
        Session::load_from_yaml(&src)
    } else {
        Session::load_from_json(&src)
    };
    let mut session = loaded.unwrap_or_else(|e| {
        eprintln!("cannot load stack: {e}");
        std::process::exit(1);
    });

    let open = session.open_current_card();
    report(&open);
    dump(&session);
    println!("Type 'help' for commands.");

    let stdin = io::stdin();
    loop {
        print!("> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let cmd = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("").trim();
        match cmd {
            "quit" | "q" | "exit" => break,
            "help" => help(),
            "dump" => dump(&session),
            "save" => {
                let out = if rest.is_empty() {
                    path.clone()
                } else {
                    rest.to_string()
                };
                match std::fs::write(&out, session.to_json()) {
                    Ok(_) => println!("saved {out}"),
                    Err(e) => println!("save failed: {e}"),
                }
            }
            "go" => match go_target(&session, rest) {
                Some(idx) => report(&session.goto_card(idx)),
                None => println!("usage: go next|prev|first|last"),
            },
            "type" => {
                let mut it = rest.splitn(2, char::is_whitespace);
                match (it.next().and_then(|s| s.parse::<u32>().ok()), it.next()) {
                    (Some(id), text) => {
                        let text = text.unwrap_or("");
                        if session.set_field_text(id, text) {
                            println!("field {id} = {text:?}");
                        } else {
                            println!("no field with id {id}");
                        }
                    }
                    _ => println!("usage: type <field-id> <text>"),
                }
            }
            "tap" => handle_tap(&mut session, rest),
            other => println!("unknown command: {other}"),
        }
    }
}

/// Map a `go` keyword to a target card index relative to the current card.
fn go_target(session: &Session, rest: &str) -> Option<usize> {
    let n = session.card_count();
    if n == 0 {
        return None;
    }
    match rest {
        "next" => Some((session.card_index() + 1) % n),
        "prev" | "previous" => Some((session.card_index() + n - 1) % n),
        "first" => Some(0),
        "last" => Some(n - 1),
        _ => None,
    }
}

fn handle_tap(session: &mut Session, rest: &str) {
    let coords: Vec<f32> = rest
        .split_whitespace()
        .filter_map(|s| s.parse::<f32>().ok())
        .collect();
    if coords.len() == 2 {
        let r = session.dispatch_touch(coords[0], coords[1], "up");
        report(&r);
        dump(session);
        return;
    }
    // Tap by object name or id: find its rect center and dispatch there.
    if let Some((cx, cy)) = session.object_center(rest) {
        let r = session.dispatch_touch(cx, cy, "up");
        report(&r);
        dump(session);
    } else {
        println!("no object matching {rest:?}; try 'tap <x> <y>'");
    }
}

fn report(r: &hypercore::DispatchResult) {
    if let Some(e) = &r.error {
        println!("script error: {e}");
    }
    for c in &r.host_cmds {
        println!("[host] {c:?}");
    }
    if r.card_changed {
        println!("[navigated]");
    }
}

fn dump(session: &Session) {
    let rl = session.render_current_card();
    println!(
        "── {} · card {}/{} \"{}\" ──",
        rl.stack_name,
        rl.card_index + 1,
        rl.card_count,
        rl.card_name
    );
    for d in &rl.items {
        let vis = if d.visible { "" } else { " (hidden)" };
        if d.kind == "field" {
            let lock = if d.locked { " [locked]" } else { "" };
            println!("  field #{} = {:?}{}{}", d.id, d.text, lock, vis);
        } else {
            println!("  button #{} {:?}{}", d.id, d.text, vis);
        }
    }
}

fn help() {
    println!(
        "commands:\n  dump\n  tap <name|id> | tap <x> <y>\n  type <field-id> <text>\n  go next|prev|first|last\n  save [path]\n  quit"
    );
}
