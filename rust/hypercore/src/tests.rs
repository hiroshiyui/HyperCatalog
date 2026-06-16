//! Unit tests for the parser, interpreter, and session facade.

use crate::script::parse_script;
use crate::session::{HostEffect, Session};

/// A minimal two-card stack used across tests.
fn sample_json() -> String {
    r#"{
      "name": "Test",
      "width": 320,
      "height": 480,
      "backgrounds": [
        { "id": 1, "name": "bg", "fields": [], "buttons": [], "script": "" }
      ],
      "cards": [
        {
          "id": 1, "name": "First", "background_id": 1,
          "fields": [
            { "id": 10, "name": "counter", "rect": {"x":10,"y":10,"w":100,"h":30}, "text": "0", "locked": true },
            { "id": 11, "name": "input", "rect": {"x":10,"y":50,"w":100,"h":30}, "text": "" }
          ],
          "buttons": [
            { "id": 20, "name": "Inc", "rect": {"x":10,"y":100,"w":80,"h":40},
              "script": "on mouseUp\n  add 1 to field \"counter\"\nend mouseUp" },
            { "id": 21, "name": "Next", "rect": {"x":10,"y":150,"w":80,"h":40},
              "script": "on mouseUp\n  go next card\nend mouseUp" }
          ],
          "script": ""
        },
        {
          "id": 2, "name": "Second", "background_id": 1,
          "fields": [], "buttons": [], "script": ""
        }
      ],
      "script": ""
    }"#
    .to_string()
}

#[test]
fn parses_a_basic_handler() {
    let script = parse_script("on mouseUp\n  put 1 into field \"x\"\nend mouseUp").unwrap();
    assert_eq!(script.handlers.len(), 1);
    assert_eq!(script.handlers[0].message, "mouseup");
}

#[test]
fn parses_if_and_arithmetic() {
    let src = "on mouseUp\n  if 1 < 2 then\n    add 3 to field \"x\"\n  else\n    beep\n  end if\nend mouseUp";
    let script = parse_script(src).unwrap();
    assert_eq!(script.handlers.len(), 1);
}

#[test]
fn round_trips_json() {
    let s = Session::load_from_json(&sample_json()).unwrap();
    let json = s.to_json();
    let s2 = Session::load_from_json(&json).unwrap();
    assert_eq!(s2.card_count(), 2);
}

#[test]
fn button_handler_mutates_field() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    // Tap the "Inc" button (inside its rect): counter 0 -> 1.
    let r = s.dispatch_touch(20.0, 120.0, "up");
    assert!(r.error.is_none(), "error: {:?}", r.error);
    assert!(r.needs_redraw);
    let render = s.render_current_card();
    let counter = render.items.iter().find(|d| d.id == 10).unwrap();
    assert_eq!(counter.text, "1");

    // Tap again -> 2.
    s.dispatch_touch(20.0, 120.0, "up");
    let counter = s
        .render_current_card()
        .items
        .into_iter()
        .find(|d| d.id == 10)
        .unwrap();
    assert_eq!(counter.text, "2");
}

#[test]
fn go_next_card_navigates() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    assert_eq!(s.card_index(), 0);
    let r = s.dispatch_touch(20.0, 170.0, "up"); // "Next" button
    assert!(r.card_changed);
    assert_eq!(s.card_index(), 1);
    assert_eq!(s.render_current_card().card_name, "Second");
}

#[test]
fn editable_field_requests_focus() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    let r = s.dispatch_touch(20.0, 60.0, "up"); // unlocked "input" field
    assert_eq!(r.focus_field, Some(11));
}

#[test]
fn host_set_field_text_persists() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    assert!(s.set_field_text(11, "hello"));
    let render = s.render_current_card();
    let input = render.items.iter().find(|d| d.id == 11).unwrap();
    assert_eq!(input.text, "hello");
}

#[test]
fn object_at_returns_topmost_id() {
    let s = Session::load_from_json(&sample_json()).unwrap();
    assert_eq!(s.object_at(20.0, 120.0), Some(20)); // "Inc" button
    assert_eq!(s.object_at(20.0, 25.0), Some(10)); // "counter" field
    assert_eq!(s.object_at(300.0, 300.0), None); // empty space
}

#[test]
fn get_and_set_object_script() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    assert!(
        s.get_object_script(20)
            .unwrap()
            .contains("add 1 to field")
    );
    assert!(s.get_object_script(999).is_none());

    // Rewrite "Inc" to subtract; lazy per-dispatch parsing means the next tap runs it.
    assert!(s.set_object_script(
        20,
        "on mouseUp\n  subtract 1 from field \"counter\"\nend mouseUp"
    ));
    s.set_field_text(10, "5");
    s.dispatch_touch(20.0, 120.0, "up");
    let counter = s
        .render_current_card()
        .items
        .into_iter()
        .find(|d| d.id == 10)
        .unwrap();
    assert_eq!(counter.text, "4");

    assert!(!s.set_object_script(999, "on mouseUp\nend mouseUp"));
}

#[test]
fn set_object_script_persists_through_json() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    assert!(s.set_object_script(21, "on mouseUp\n  beep\nend mouseUp"));
    let s2 = Session::load_from_json(&s.to_json()).unwrap();
    assert!(s2.get_object_script(21).unwrap().contains("beep"));
}

#[test]
fn check_script_flags_parse_errors() {
    assert!(Session::check_script("on mouseUp\n  beep\nend mouseUp").is_none());
    // `if` with no `then` is a parse error.
    assert!(Session::check_script("on mouseUp\n  if 1 < 2\n    beep\nend mouseUp").is_some());
}

#[test]
fn answer_produces_host_effect() {
    let json = r#"{
      "name": "A", "cards": [
        { "id": 1, "name": "C", "buttons": [
          { "id": 1, "name": "B", "rect": {"x":0,"y":0,"w":50,"h":50},
            "script": "on mouseUp\n  answer \"hi\"\nend mouseUp" }
        ] }
      ]
    }"#;
    let mut s = Session::load_from_json(json).unwrap();
    let r = s.dispatch_touch(10.0, 10.0, "up");
    assert_eq!(r.host_cmds, vec![HostEffect::Answer("hi".to_string())]);
}

#[test]
fn background_button_script_runs() {
    // A nav button living on the shared background must have its own handler run.
    let json = r#"{
      "name": "A",
      "backgrounds": [
        { "id": 1, "name": "bg", "buttons": [
          { "id": 99, "name": "Next", "rect": {"x":0,"y":0,"w":50,"h":50},
            "script": "on mouseUp\n  go next card\nend mouseUp" }
        ] }
      ],
      "cards": [
        { "id": 1, "name": "One", "background_id": 1 },
        { "id": 2, "name": "Two", "background_id": 1 }
      ]
    }"#;
    let mut s = Session::load_from_json(json).unwrap();
    assert_eq!(s.card_index(), 0);
    let r = s.dispatch_touch(10.0, 10.0, "up");
    assert!(r.error.is_none(), "error: {:?}", r.error);
    assert!(r.card_changed);
    assert_eq!(s.render_current_card().card_name, "Two");
}

#[test]
fn put_into_field_and_concat() {
    let json = r#"{
      "name": "A", "cards": [
        { "id": 1, "name": "C",
          "fields": [ { "id": 5, "name": "out", "rect": {"x":0,"y":0,"w":50,"h":50}, "locked": true } ],
          "buttons": [
            { "id": 1, "name": "B", "rect": {"x":0,"y":60,"w":50,"h":50},
              "script": "on mouseUp\n  put \"a\" & \"b\" into field \"out\"\nend mouseUp" }
          ] }
      ]
    }"#;
    let mut s = Session::load_from_json(json).unwrap();
    s.dispatch_touch(10.0, 70.0, "up");
    let render = s.render_current_card();
    assert_eq!(render.items.iter().find(|d| d.id == 5).unwrap().text, "ab");
}
