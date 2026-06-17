//! Unit tests for the parser, interpreter, and session facade.

use crate::script::parse_script;
use crate::session::{HostEffect, ObjectProps, Session, ViewNode};

/// A view node's property value by key (empty if absent), for the ADR-0008 view-tree tests.
fn prop(n: &ViewNode, key: &str) -> String {
    n.props
        .iter()
        .find(|p| p.key == key)
        .map(|p| p.value.clone())
        .unwrap_or_default()
}

/// A node's prop by object id, re-rendering the current view tree (for switch/state assertions).
fn prop_node(s: &Session, id: u32, key: &str) -> String {
    let t = s.render_view_tree();
    t.nodes
        .iter()
        .find(|n| n.id == id)
        .map(|n| prop(n, key))
        .unwrap_or_default()
}

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

// --- ADR-0008 native render target: semantic view tree + id-addressed dispatch ---

#[test]
fn view_tree_shape() {
    let s = Session::load_from_json(&sample_json()).unwrap();
    let t = s.render_view_tree();
    assert_eq!(t.card_count, 2);
    assert_eq!(t.card_name, "First");
    // Render order: empty background, then card fields (10, 11), then card buttons (20, 21).
    assert_eq!(t.root_ids, vec![10, 11, 20, 21]);

    let counter = t.nodes.iter().find(|n| n.id == 10).unwrap();
    assert_eq!(counter.kind, "field");
    assert_eq!(prop(counter, "text"), "0");
    assert_eq!(prop(counter, "locked"), "true");

    let inc = t.nodes.iter().find(|n| n.id == 20).unwrap();
    assert_eq!(inc.kind, "button");
    assert_eq!(prop(inc, "title"), "Inc"); // title empty → falls back to name
    assert_eq!(prop(inc, "style"), "rounded"); // default ButtonStyle
}

#[test]
fn dispatch_by_id_matches_touch() {
    // The load-bearing parity: id-dispatch must run the same handler a coordinate tap does.
    let mut by_touch = Session::load_from_json(&sample_json()).unwrap();
    let rt = by_touch.dispatch_touch(20.0, 120.0, "up"); // hits button 20

    let mut by_id = Session::load_from_json(&sample_json()).unwrap();
    let ri = by_id.dispatch_by_id(20, "mouseUp", &[]);

    assert!(rt.error.is_none() && ri.error.is_none());
    assert_eq!(rt.needs_redraw, ri.needs_redraw);
    let field_of = |s: &Session| {
        s.render_current_card()
            .items
            .into_iter()
            .find(|d| d.id == 10)
            .unwrap()
            .text
    };
    assert_eq!(field_of(&by_touch), "1");
    assert_eq!(field_of(&by_id), "1");
}

#[test]
fn dispatch_by_id_unknown_is_noop() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    let r = s.dispatch_by_id(999, "mouseUp", &[]);
    assert!(r.error.is_none());
    assert!(!r.needs_redraw);
    assert!(!r.card_changed);
}

#[test]
fn view_tree_omits_geometry() {
    // Guardrail: geometry/pixels must never cross the boundary outward (ADR-0008).
    let s = Session::load_from_json(&sample_json()).unwrap();
    let t = s.render_view_tree();
    for n in &t.nodes {
        for p in &n.props {
            assert!(
                !matches!(p.key.as_str(), "x" | "y" | "w" | "h" | "rect"),
                "view node leaked geometry prop {:?}",
                p.key
            );
        }
    }
}

// --- ADR-0014 layout overlay: group containers, weight ---

/// A one-card stack with a layout overlay: a root column of [ row[field 10, button 20], field 11 ].
fn layout_yaml() -> String {
    r#"
name: LTest
width: 100
height: 100
cards:
  - id: 1
    name: One
    fields:
      - { id: 10, name: a, rect: { x: 0, y: 0, w: 10, h: 10 }, text: "A", weight: 2 }
      - { id: 11, name: b, rect: { x: 0, y: 0, w: 10, h: 10 }, text: "B" }
    buttons:
      - { id: 20, name: go, rect: { x: 0, y: 0, w: 10, h: 10 }, title: "Go", weight: 1 }
    layout:
      mode: column
      padding: 8
      children:
        - { mode: row, padding: 4, children: [10, 20] }
        - 11
"#
    .to_string()
}

#[test]
fn view_tree_groups_nest() {
    let s = Session::load_from_yaml(&layout_yaml()).unwrap();
    let t = s.render_view_tree();
    // Root is a column with padding; its children are the row-group then field 11.
    assert_eq!(t.layout, "column");
    assert_eq!(t.padding, 8.0);
    // Group ids are synthetic, above the max object id (20) → 21.
    assert_eq!(t.root_ids, vec![21, 11]);

    let group = t.nodes.iter().find(|n| n.id == 21).unwrap();
    assert_eq!(group.kind, "group");
    assert_eq!(prop(group, "mode"), "row");
    assert_eq!(prop(group, "padding"), "4");
    assert_eq!(group.child_ids, vec![10, 20]);

    // Objects carry a weight prop projected from the model.
    let f10 = t.nodes.iter().find(|n| n.id == 10).unwrap();
    assert_eq!(f10.kind, "field");
    assert_eq!(prop(f10, "weight"), "2");
    let b20 = t.nodes.iter().find(|n| n.id == 20).unwrap();
    assert_eq!(prop(b20, "weight"), "1");
}

#[test]
fn view_tree_no_layout_is_flat() {
    // A card without a layout overlay falls back to a flat column (slice-1 behavior, unchanged).
    let s = Session::load_from_json(&sample_json()).unwrap();
    let t = s.render_view_tree();
    assert_eq!(t.layout, "column");
    assert_eq!(t.padding, 0.0);
    assert_eq!(t.root_ids, vec![10, 11, 20, 21]);
    assert!(t.nodes.iter().all(|n| n.kind != "group"));
}

#[test]
fn view_tree_skips_dangling_layout_ref() {
    // A group referencing a non-existent object id silently omits it (ADR-0014 caveat).
    let yaml = r#"
name: D
cards:
  - id: 1
    name: One
    fields:
      - { id: 10, name: a, rect: { x: 0, y: 0, w: 10, h: 10 } }
    layout:
      mode: row
      children: [10, 999]
"#;
    let s = Session::load_from_yaml(yaml).unwrap();
    let t = s.render_view_tree();
    assert_eq!(t.root_ids, vec![10]); // 999 dropped, not present
    assert!(t.nodes.iter().all(|n| n.id != 999));
}

#[test]
fn weight_get_set_via_script() {
    // `the weight of` is scriptable like other object properties (dialect: set the weight of field).
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    let script = "on mouseUp\n  set the weight of field \"input\" to 2\n  \
                  put the weight of field \"input\" into field \"counter\"\nend mouseUp";
    s.set_object_script(11, script);
    let r = s.dispatch_by_id(11, "mouseUp", &[]); // field "input" is id 11
    assert!(r.error.is_none(), "error: {:?}", r.error);
    let counter = s
        .render_current_card()
        .items
        .into_iter()
        .find(|d| d.id == 10)
        .unwrap();
    assert_eq!(counter.text, "2");
}

// --- ADR-0015 switch object kind (a button with `checked`) ---

/// A one-card stack with a switch (button id 20 carrying `checked`) and a readout field 10.
fn switch_yaml() -> String {
    r#"
name: Sw
cards:
  - id: 1
    name: One
    fields:
      - { id: 10, name: out, rect: { x: 0, y: 0, w: 10, h: 10 }, text: "" }
    buttons:
      - { id: 20, name: wifi, rect: { x: 0, y: 0, w: 10, h: 10 }, title: "Wi-Fi", checked: false,
          script: "on mouseUp\n  if the checked of me then put \"on\" into field \"out\" else put \"off\" into field \"out\"\nend mouseUp" }
"#
    .to_string()
}

#[test]
fn switch_projects_as_switch_kind() {
    let s = Session::load_from_yaml(&switch_yaml()).unwrap();
    let t = s.render_view_tree();
    let sw = t.nodes.iter().find(|n| n.id == 20).unwrap();
    assert_eq!(sw.kind, "switch");
    assert_eq!(prop(sw, "checked"), "false");
    // A plain button (no `checked`) stays kind "button".
    let plain = Session::load_from_json(&sample_json())
        .unwrap()
        .render_view_tree();
    assert_eq!(
        plain.nodes.iter().find(|n| n.id == 20).unwrap().kind,
        "button"
    );
}

#[test]
fn switch_auto_toggles_before_handler() {
    let mut s = Session::load_from_yaml(&switch_yaml()).unwrap();
    // First tap: checked flips false→true *before* mouseUp, so the script reads "on".
    let r = s.dispatch_by_id(20, "mouseUp", &[]);
    assert!(r.error.is_none(), "error: {:?}", r.error);
    assert_eq!(prop_node(&s, 20, "checked"), "true");
    assert_eq!(field_text(&s, 10), "on");
    // Second tap: true→false, script reads "off".
    s.dispatch_by_id(20, "mouseUp", &[]);
    assert_eq!(prop_node(&s, 20, "checked"), "false");
    assert_eq!(field_text(&s, 10), "off");
}

#[test]
fn the_checked_of_is_scriptable() {
    let mut s = Session::load_from_yaml(&switch_yaml()).unwrap();
    s.set_object_script(
        10,
        "on mouseUp\n  set the checked of button \"wifi\" to true\nend mouseUp",
    );
    s.dispatch_by_id(10, "mouseUp", &[]);
    assert_eq!(prop_node(&s, 20, "checked"), "true");
}

#[test]
fn set_card_layout_via_script() {
    // `set the layout of this card to "row"` builds a single-level root over all card objects.
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    s.set_object_script(
        20,
        "on mouseUp\n  set the layout of this card to \"row\"\n  set the padding of this card to 6\nend mouseUp",
    );
    s.dispatch_by_id(20, "mouseUp", &[]);
    let t = s.render_view_tree();
    assert_eq!(t.layout, "row");
    assert_eq!(t.padding, 6.0);
    assert_eq!(t.root_ids, vec![10, 11, 20, 21]); // all card objects, render order
}

#[test]
fn the_layout_of_card_getter() {
    let mut s = Session::load_from_yaml(&layout_yaml()).unwrap();
    s.set_object_script(
        20,
        "on mouseUp\n  put the layout of this card into field \"b\"\nend mouseUp",
    );
    s.dispatch_by_id(20, "mouseUp", &[]);
    assert_eq!(field_text(&s, 11), "column"); // field "b" is id 11
}

#[test]
fn free_layout_emits_geometry() {
    // `free` is the absolute escape hatch (ADR-0017): object nodes carry x/y/w/h, ViewTree size.
    let yaml = r#"
name: F
width: 300
height: 400
cards:
  - id: 1
    name: One
    buttons:
      - { id: 20, name: a, rect: { x: 10, y: 20, w: 80, h: 40 }, title: "A" }
    layout:
      mode: free
      children: [20]
"#;
    let s = Session::load_from_yaml(yaml).unwrap();
    let t = s.render_view_tree();
    assert_eq!(t.layout, "free");
    assert_eq!(t.width, 300.0);
    assert_eq!(t.height, 400.0);
    let a = t.nodes.iter().find(|n| n.id == 20).unwrap();
    assert_eq!(prop(a, "x"), "10");
    assert_eq!(prop(a, "y"), "20");
    assert_eq!(prop(a, "w"), "80");
    assert_eq!(prop(a, "h"), "40");
}

#[test]
fn non_free_layout_still_omits_geometry() {
    // Geometry is intentional ONLY in free mode; a grouped (non-free) card stays geometry-free.
    let s = Session::load_from_yaml(&layout_yaml()).unwrap();
    let t = s.render_view_tree();
    for n in &t.nodes {
        for p in &n.props {
            assert!(!matches!(p.key.as_str(), "x" | "y" | "w" | "h" | "rect"));
        }
    }
}

#[test]
fn grid_mode_projects_columns() {
    let yaml = r#"
name: G
cards:
  - id: 1
    name: One
    buttons:
      - { id: 20, name: a, rect: { x: 0, y: 0, w: 10, h: 10 }, title: "A" }
      - { id: 21, name: b, rect: { x: 0, y: 0, w: 10, h: 10 }, title: "B" }
    layout:
      mode: grid
      columns: 2
      children: [20, 21]
"#;
    let s = Session::load_from_yaml(yaml).unwrap();
    let t = s.render_view_tree();
    assert_eq!(t.layout, "grid");
    assert_eq!(t.columns, 2);
}

#[test]
fn layout_group_yaml_round_trips() {
    // The externally-tagged LayoutChild enum round-trips through yaml_serde unchanged.
    let stack: crate::model::Stack = yaml_serde::from_str(&layout_yaml()).unwrap();
    let reser = yaml_serde::to_string(&stack).unwrap();
    let again: crate::model::Stack = yaml_serde::from_str(&reser).unwrap();
    assert_eq!(stack, again);
    // And the overlay actually parsed (not silently dropped).
    assert!(stack.cards[0].layout.is_some());
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
fn goto_card_clamps_and_restores_index() {
    // Backs the host's ADR-0013 card-index restore (HyperStack::open_card_at).
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    let r = s.goto_card(1);
    assert!(r.card_changed);
    assert_eq!(s.card_index(), 1);
    assert_eq!(s.render_current_card().card_name, "Second");
    // Out-of-range index clamps to the last card rather than panicking.
    s.goto_card(999);
    assert_eq!(s.card_index(), s.card_count() - 1);
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
    assert!(s.get_object_script(20).unwrap().contains("add 1 to field"));
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
fn add_delete_object_round_trips() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    let before = s.render_current_card().items.len();

    let id = s.add_object("button").unwrap();
    assert!(id > 21, "new id should exceed existing ids");
    assert_eq!(s.render_current_card().items.len(), before + 1);
    // The new button is tappable/selectable at its default position.
    assert_eq!(s.object_at(30.0, 95.0), Some(id));

    assert!(s.delete_object(id));
    assert_eq!(s.render_current_card().items.len(), before);
    assert!(!s.delete_object(id)); // already gone

    assert!(s.add_object("widget").is_none()); // unknown kind
}

#[test]
fn set_object_rect_moves_and_clamps() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    // Move the "Inc" button (id 20) and shrink below the minimum.
    assert!(s.set_object_rect(20, 200.0, 300.0, 1.0, 1.0));
    let props: serde_json::Value = serde_json::from_str(&s.get_object_props(20).unwrap()).unwrap();
    assert_eq!(props["x"], 200.0);
    assert_eq!(props["y"], 300.0);
    assert!(
        props["w"].as_f64().unwrap() >= 12.0,
        "width clamped to minimum"
    );
    assert!(!s.set_object_rect(999, 0.0, 0.0, 50.0, 50.0));
}

#[test]
fn get_and_set_object_props() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();

    // Button: name + title + style.
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(20).unwrap()).unwrap();
    assert_eq!(p["kind"], "button");
    assert!(s.set_object_props(20, r#"{"name":"Plus","title":"+1","style":"rectangle"}"#));
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(20).unwrap()).unwrap();
    assert_eq!(p["name"], "Plus");
    assert_eq!(p["title"], "+1");
    assert_eq!(p["style"], "rectangle");

    // Field: text + locked.
    assert!(s.set_object_props(11, r#"{"text":"hi","locked":true}"#));
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(11).unwrap()).unwrap();
    assert_eq!(p["text"], "hi");
    assert_eq!(p["locked"], true);

    assert!(s.get_object_props(999).is_none());
    assert!(!s.set_object_props(999, "{}"));
    assert!(!s.set_object_props(20, "not json"));
}

#[test]
fn authored_objects_persist_through_json() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    let id = s.add_object("field").unwrap();
    s.set_object_rect(id, 10.0, 200.0, 80.0, 30.0);
    s.set_object_props(id, r#"{"name":"note","text":"kept"}"#);

    let s2 = Session::load_from_json(&s.to_json()).unwrap();
    let p: serde_json::Value = serde_json::from_str(&s2.get_object_props(id).unwrap()).unwrap();
    assert_eq!(p["name"], "note");
    assert_eq!(p["text"], "kept");
    assert_eq!(p["y"], 200.0);
}

/// Helper: rewrite the "Inc" button (id 20) to run `body`, then tap it.
fn run_on_inc(s: &mut Session, body: &str) {
    let src = format!("on mouseUp\n  {body}\nend mouseUp");
    assert!(s.set_object_script(20, &src));
    let r = s.dispatch_touch(20.0, 120.0, "up");
    assert!(r.error.is_none(), "script error: {:?}", r.error);
}

fn field_text(s: &Session, id: u32) -> String {
    s.render_current_card()
        .items
        .into_iter()
        .find(|d| d.id == id)
        .unwrap()
        .text
}

/// Like `run_on_inc`, but for scripts expected to error; returns the dispatch error.
fn run_on_inc_err(s: &mut Session, body: &str) -> Option<String> {
    let src = format!("on mouseUp\n  {body}\nend mouseUp");
    assert!(s.set_object_script(20, &src));
    s.dispatch_touch(20.0, 120.0, "up").error
}

#[test]
fn reads_geometry_properties() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    // "input" field id 11 has rect {x:10,y:50,w:100,h:30}; results go into "counter" (id 10).
    run_on_inc(
        &mut s,
        r#"put the width of field "input" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "100");
    run_on_inc(
        &mut s,
        r#"put the height of field "input" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "30");
    run_on_inc(
        &mut s,
        r#"put the loc of field "input" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "60,65"); // center: (10+50, 50+15)
    run_on_inc(
        &mut s,
        r#"put the rect of field "input" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "10,50,110,80");
    run_on_inc(
        &mut s,
        r#"put the right of field "input" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "110");
    run_on_inc(
        &mut s,
        r#"put the id of field "input" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "11");
}

#[test]
fn writes_geometry_properties() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();

    // loc re-centers, keeping size (100x30): center (200,200) -> x=150, y=185.
    run_on_inc(&mut s, r#"set the loc of field "input" to "200,200""#);
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(11).unwrap()).unwrap();
    assert_eq!(p["x"], 150.0);
    assert_eq!(p["y"], 185.0);
    assert_eq!(p["w"], 100.0); // unchanged

    // rect sets all four edges.
    run_on_inc(&mut s, r#"set the rect of field "input" to "0,0,40,60""#);
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(11).unwrap()).unwrap();
    assert_eq!(p["x"], 0.0);
    assert_eq!(p["w"], 40.0);
    assert_eq!(p["h"], 60.0);

    // width keeps the top-left corner; a sub-minimum value is clamped.
    run_on_inc(&mut s, r#"set the width of field "input" to "0""#);
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(11).unwrap()).unwrap();
    assert!(p["w"].as_f64().unwrap() >= 1.0, "width clamped to minimum");
    assert_eq!(p["x"], 0.0);
}

#[test]
fn text_style_properties_default_and_set() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();

    // Defaults: size 16, style reads back as "plain".
    run_on_inc(
        &mut s,
        r#"put the textSize of field "input" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "16");
    run_on_inc(
        &mut s,
        r#"put the textStyle of field "input" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "plain");

    // Set style, size, font, align via script.
    run_on_inc(
        &mut s,
        r#"set the textStyle of field "input" to "bold,italic""#,
    );
    run_on_inc(&mut s, r#"set the textSize of field "input" to 24"#);
    run_on_inc(&mut s, r#"set the textFont of field "input" to "serif""#);
    run_on_inc(&mut s, r#"set the textAlign of field "input" to "center""#);

    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(11).unwrap()).unwrap();
    assert_eq!(p["text_style"], "bold,italic");
    assert_eq!(p["text_size"], 24.0);
    assert_eq!(p["text_font"], "serif");
    assert_eq!(p["text_align"], "center");
}

#[test]
fn text_attrs_default_in_render_and_round_trip() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    // Render carries text attributes; defaults are size 16, empty style/font/align.
    let item = s
        .render_current_card()
        .items
        .into_iter()
        .find(|d| d.id == 11)
        .unwrap();
    assert_eq!(item.text_size, 16.0);
    assert_eq!(item.text_style, "");

    // Authoring props round-trip through JSON save/load.
    assert!(s.set_object_props(11, r#"{"text_style":"underline","text_size":20}"#));
    let s2 = Session::load_from_json(&s.to_json()).unwrap();
    let p: serde_json::Value = serde_json::from_str(&s2.get_object_props(11).unwrap()).unwrap();
    assert_eq!(p["text_style"], "underline");
    assert_eq!(p["text_size"], 20.0);
}

#[test]
fn unknown_property_still_errors() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    let src = "on mouseUp\n  set the bogus of field \"input\" to \"1\"\nend mouseUp";
    assert!(s.set_object_script(20, src));
    let r = s.dispatch_touch(20.0, 120.0, "up");
    assert!(r.error.is_some(), "unknown property should error");
}

#[test]
fn out_of_range_selector_errors_without_panic() {
    // Regression: an out-of-range object number must yield Err, not an index panic that
    // would unwind across the FFI boundary (UB). See find_index upper-bound check.
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    let src = "on mouseUp\n  put field 99 into field \"counter\"\nend mouseUp";
    assert!(s.set_object_script(20, src));
    let r = s.dispatch_touch(20.0, 120.0, "up");
    assert!(r.error.is_some(), "out-of-range field number should error");

    let src = "on mouseUp\n  put the name of button 99 into field \"counter\"\nend mouseUp";
    assert!(s.set_object_script(20, src));
    let r = s.dispatch_touch(20.0, 120.0, "up");
    assert!(r.error.is_some(), "out-of-range button number should error");
}

#[test]
fn runaway_repeat_is_bounded() {
    // A huge loop must error (bounded budget) rather than hang the synchronous dispatch.
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    let src =
        "on mouseUp\n  repeat with i = 1 to 999999999\n    put i into x\n  end repeat\nend mouseUp";
    assert!(s.set_object_script(20, src));
    let r = s.dispatch_touch(20.0, 120.0, "up");
    assert!(
        r.error.is_some(),
        "runaway repeat should be bounded with an error"
    );

    // A normal-sized loop still completes fine.
    let src = "on mouseUp\n  put 0 into x\n  repeat with i = 1 to 5\n    add i to x\n  end repeat\n  put x into field \"counter\"\nend mouseUp";
    assert!(s.set_object_script(20, src));
    let r = s.dispatch_touch(20.0, 120.0, "up");
    assert!(r.error.is_none(), "small loop errored: {:?}", r.error);
    assert_eq!(field_text(&s, 10), "15"); // 1+2+3+4+5
}

#[test]
fn me_resolves_for_background_object() {
    // Regression: `me` must resolve for a background-layer object, not only card objects.
    let json = r#"{
      "name": "A",
      "backgrounds": [
        { "id": 1, "name": "bg", "buttons": [
          { "id": 99, "name": "tag", "rect": {"x":0,"y":0,"w":50,"h":50},
            "script": "on mouseUp\n  set the title of me to \"hit\"\nend mouseUp" }
        ] }
      ],
      "cards": [ { "id": 1, "name": "One", "background_id": 1 } ]
    }"#;
    let mut s = Session::load_from_json(json).unwrap();
    let r = s.dispatch_touch(10.0, 10.0, "up");
    assert!(
        r.error.is_none(),
        "me on a background button errored: {:?}",
        r.error
    );
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(99).unwrap()).unwrap();
    assert_eq!(p["title"], "hit");
}

#[test]
fn open_card_surfaces_host_effects() {
    // Regression: an `on openCard` handler's host effects (beep/answer/…) must come back in the
    // DispatchResult — the host now surfaces them after navigation (CardView.applyDispatchResult).
    let json = r#"{
      "name": "A", "cards": [
        { "id": 1, "name": "One", "script": "on openCard\n  beep\nend openCard" },
        { "id": 2, "name": "Two" }
      ]
    }"#;
    let mut s = Session::load_from_json(json).unwrap();
    let r = s.open_current_card();
    assert_eq!(r.host_cmds, vec![HostEffect::Beep]);
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
fn go_to_stack_emits_host_effect() {
    // `go to stack "X"` can't be done in-core (no asset access); it returns a host effect.
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    assert!(s.set_object_script(20, "on mouseUp\n  go to stack \"Other\"\nend mouseUp"));
    let r = s.dispatch_touch(20.0, 120.0, "up");
    assert!(r.error.is_none(), "error: {:?}", r.error);
    assert_eq!(r.host_cmds, vec![HostEffect::GoStack("Other".to_string())]);
    // The switch is the host's job; the current session's card index is untouched.
    assert!(!r.card_changed);
    assert_eq!(s.card_index(), 0);
}

#[test]
fn show_stacks_emits_host_effect() {
    // `show stacks` asks the host to open its picker; it has no in-core effect.
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    assert!(s.set_object_script(20, "on mouseUp\n  show stacks\nend mouseUp"));
    let r = s.dispatch_touch(20.0, 120.0, "up");
    assert!(r.error.is_none(), "error: {:?}", r.error);
    assert_eq!(r.host_cmds, vec![HostEffect::ShowStacks]);
}

#[test]
fn go_stack_without_to_parses() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    assert!(s.set_object_script(20, "on mouseUp\n  go stack \"X\"\nend mouseUp"));
    let r = s.dispatch_touch(20.0, 120.0, "up");
    assert_eq!(r.host_cmds, vec![HostEffect::GoStack("X".to_string())]);
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

#[test]
fn arithmetic_and_comparison_operators() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    // Each case overwrites the locked "counter" field (id 10) with an expression result.
    let cases = [
        (r#"put 2 * 3 into field "counter""#, "6"),
        (r#"put 7 / 2 into field "counter""#, "3.5"),
        (r#"put 10 / 2 into field "counter""#, "5"), // integral result drops ".0"
        (r#"put 7 mod 3 into field "counter""#, "1"),
        (r#"put 1 + 2 * 3 into field "counter""#, "7"), // precedence: mul before add
        (r#"put -5 into field "counter""#, "-5"),       // unary minus
        (r#"put (2 < 3) into field "counter""#, "true"),
        (r#"put (3 <= 3) into field "counter""#, "true"),
        (r#"put (4 > 9) into field "counter""#, "false"),
        (r#"put (5 >= 6) into field "counter""#, "false"),
        (r#"put (1 is 1) into field "counter""#, "true"),
        (r#"put (1 is not 2) into field "counter""#, "true"),
        (r#"put (1 <> 2) into field "counter""#, "true"),
        (r#"put ("ABC" = "abc") into field "counter""#, "true"), // case-insensitive text eq
        (r#"put ("b" > "a") into field "counter""#, "true"),     // lexical text compare
        (r#"put (10 > 9) into field "counter""#, "true"),        // numeric, not lexical
        (r#"put (true and false) into field "counter""#, "false"),
        (r#"put (true or false) into field "counter""#, "true"),
        (r#"put (not false) into field "counter""#, "true"),
        (r#"put ("a" & "b") into field "counter""#, "ab"), // concat
        (r#"put ("a" && "b") into field "counter""#, "a b"), // concat-with-space
    ];
    for (body, expect) in cases {
        run_on_inc(&mut s, body);
        assert_eq!(field_text(&s, 10), expect, "for script body `{body}`");
    }
}

#[test]
fn builtin_functions() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    run_on_inc(&mut s, r#"put length("hello") into field "counter""#);
    assert_eq!(field_text(&s, 10), "5");
    run_on_inc(&mut s, r#"put trunc(3.9) into field "counter""#);
    assert_eq!(field_text(&s, 10), "3");
    run_on_inc(&mut s, r#"put the number of cards into field "counter""#);
    assert_eq!(field_text(&s, 10), "2");
    run_on_inc(&mut s, r#"put the number of fields into field "counter""#);
    assert_eq!(field_text(&s, 10), "2");
    run_on_inc(&mut s, r#"put the number of buttons into field "counter""#);
    assert_eq!(field_text(&s, 10), "2");
    run_on_inc(
        &mut s,
        r#"put the number of backgrounds into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "1");

    // The PRNG is deterministic and seeded; random(1) is always 1, random(n) stays in 1..=n.
    run_on_inc(&mut s, r#"put random(1) into field "counter""#);
    assert_eq!(field_text(&s, 10), "1");
    run_on_inc(&mut s, r#"put random(6) into field "counter""#);
    let n: i64 = field_text(&s, 10).parse().unwrap();
    assert!((1..=6).contains(&n), "random(6) out of range: {n}");

    // Unknown function is an error.
    assert!(run_on_inc_err(&mut s, r#"put bogus(1) into field "counter""#).is_some());
}

#[test]
fn string_constants() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    let cases = [
        (r#"put space into field "counter""#, " "),
        (r#"put quote into field "counter""#, "\""),
        (r#"put comma into field "counter""#, ","),
        (r#"put colon into field "counter""#, ":"),
        ("put return into field \"counter\"", "\n"),
        ("put tab into field \"counter\"", "\t"),
        (r#"put empty into field "counter""#, ""),
    ];
    for (body, expect) in cases {
        run_on_inc(&mut s, body);
        assert_eq!(field_text(&s, 10), expect, "for script body `{body}`");
    }
}

#[test]
fn get_sets_it_then_put_reads_it() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    run_on_inc(&mut s, "get 5 + 3\n  put it into field \"counter\"");
    assert_eq!(field_text(&s, 10), "8");
}

#[test]
fn multiply_and_divide_statements() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    s.set_field_text(10, "5");
    run_on_inc(&mut s, r#"multiply field "counter" by 3"#);
    assert_eq!(field_text(&s, 10), "15");
    run_on_inc(&mut s, r#"divide field "counter" by 5"#);
    assert_eq!(field_text(&s, 10), "3");
}

#[test]
fn arithmetic_on_non_number_container_errors() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    s.set_field_text(10, "abc");
    assert!(run_on_inc_err(&mut s, r#"add 1 to field "counter""#).is_some());
}

#[test]
fn repeat_for_times_and_exit_repeat() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    // `repeat for N times` form.
    run_on_inc(
        &mut s,
        "put 0 into field \"counter\"\n  repeat for 3 times\n    add 1 to field \"counter\"\n  end repeat",
    );
    assert_eq!(field_text(&s, 10), "3");

    // `exit repeat` breaks out of a counting loop early.
    run_on_inc(
        &mut s,
        "put 0 into field \"counter\"\n  repeat with i = 1 to 10\n    add 1 to field \"counter\"\n    if i is 3 then exit repeat\n  end repeat",
    );
    assert_eq!(field_text(&s, 10), "3");
}

#[test]
fn exit_handler_stops_remaining_statements() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    run_on_inc(
        &mut s,
        "put 1 into field \"counter\"\n  exit mouseUp\n  put 99 into field \"counter\"",
    );
    assert_eq!(field_text(&s, 10), "1");
}

#[test]
fn single_line_if_else() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    run_on_inc(
        &mut s,
        r#"if 1 > 2 then put "a" into field "counter" else put "b" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "b");
}

#[test]
fn field_selector_by_number() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    s.set_field_text(11, "hi"); // "input" is the 2nd field on the card
    run_on_inc(&mut s, r#"put field 2 into field "counter""#);
    assert_eq!(field_text(&s, 10), "hi");
}

/// Run `body` on the "Inc" button of a fresh stack and return the resulting card index.
fn nav_index(body: &str) -> usize {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    run_on_inc(&mut s, body);
    s.card_index()
}

#[test]
fn go_navigation_variants() {
    assert_eq!(nav_index("go last card"), 1);
    assert_eq!(nav_index("go first card"), 0);
    assert_eq!(nav_index("go prev card"), 1); // wraps from card 1 back to the last
    assert_eq!(nav_index("go card 2"), 1);
    assert_eq!(nav_index("go card 3"), 0); // (3-1) mod 2 == 0, wraps around
    assert_eq!(nav_index(r#"go card "Second""#), 1);

    // An unknown card name errors rather than navigating.
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    assert!(run_on_inc_err(&mut s, r#"go card "Nope""#).is_some());
    assert_eq!(s.card_index(), 0);
}

#[test]
fn card_and_stack_properties() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    run_on_inc(&mut s, r#"put the name of this card into field "counter""#);
    assert_eq!(field_text(&s, 10), "First");
    run_on_inc(&mut s, r#"put the name of stack into field "counter""#);
    assert_eq!(field_text(&s, 10), "Test");

    // Rename the card and the stack, then read them back.
    run_on_inc(&mut s, r#"set the name of this card to "Renamed""#);
    run_on_inc(&mut s, r#"put the name of this card into field "counter""#);
    assert_eq!(field_text(&s, 10), "Renamed");
    run_on_inc(&mut s, r#"set the name of stack to "NewStack""#);
    run_on_inc(&mut s, r#"put the name of stack into field "counter""#);
    assert_eq!(field_text(&s, 10), "NewStack");
}

#[test]
fn unknown_card_and_stack_property_errors() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    assert!(run_on_inc_err(&mut s, r#"set the bogus of this card to "1""#).is_some());
    assert!(run_on_inc_err(&mut s, r#"set the bogus of stack to "1""#).is_some());
}

#[test]
fn me_resolves_for_card_field() {
    // The locked "counter" field runs its mouseUp on tap; `me` must resolve to that field.
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    assert!(s.set_object_script(10, "on mouseUp\n  set the text of me to \"F\"\nend mouseUp"));
    let r = s.dispatch_touch(20.0, 25.0, "up"); // inside counter's rect
    assert!(r.error.is_none(), "error: {:?}", r.error);
    assert_eq!(field_text(&s, 10), "F");
}

#[test]
fn background_field_read_and_write() {
    let json = r#"{
      "name": "A",
      "backgrounds": [
        { "id": 1, "name": "bg", "fields": [
          { "id": 50, "name": "label", "rect": {"x":0,"y":0,"w":50,"h":20}, "text": "hi" }
        ] }
      ],
      "cards": [
        { "id": 1, "name": "One", "background_id": 1,
          "fields": [ { "id": 51, "name": "out", "rect": {"x":0,"y":60,"w":50,"h":20}, "locked": true } ],
          "buttons": [
            { "id": 60, "name": "B", "rect": {"x":0,"y":100,"w":50,"h":20},
              "script": "on mouseUp\n  put bg field \"label\" into field \"out\"\n  set the text of bg field \"label\" to \"bye\"\nend mouseUp" }
          ] }
      ]
    }"#;
    let mut s = Session::load_from_json(json).unwrap();
    let r = s.dispatch_touch(10.0, 110.0, "up");
    assert!(r.error.is_none(), "error: {:?}", r.error);
    // The card field copied the background field's text...
    assert_eq!(field_text(&s, 51), "hi");
    // ...and the background field itself was updated by `set`.
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(50).unwrap()).unwrap();
    assert_eq!(p["text"], "bye");
}

#[test]
fn reads_button_properties() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    // "Next" (id 21) has no title, so its label falls back to the name.
    run_on_inc(
        &mut s,
        r#"put the title of button "Next" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "Next");
    run_on_inc(
        &mut s,
        r#"put the name of button "Next" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "Next");
    run_on_inc(
        &mut s,
        r#"put the id of button "Next" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "21");
    run_on_inc(
        &mut s,
        r#"put the textSize of button "Next" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "16"); // default
    run_on_inc(
        &mut s,
        r#"put the textStyle of button "Next" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "plain"); // no styles set reads back as "plain"
    run_on_inc(
        &mut s,
        r#"put the visible of button "Next" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "true");
    // Geometry properties resolve on buttons too (rect {x:10,y:150,w:80,h:40}).
    run_on_inc(
        &mut s,
        r#"put the width of button "Next" into field "counter""#,
    );
    assert_eq!(field_text(&s, 10), "80");
}

#[test]
fn writes_button_properties() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    run_on_inc(&mut s, r#"set the title of button "Next" to "Go""#);
    run_on_inc(&mut s, r#"set the textSize of button "Next" to 22"#);
    run_on_inc(
        &mut s,
        r#"set the textStyle of button "Next" to "bold,italic""#,
    );
    run_on_inc(&mut s, r#"set the textFont of button "Next" to "serif""#);
    run_on_inc(&mut s, r#"set the textAlign of button "Next" to "center""#);
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(21).unwrap()).unwrap();
    assert_eq!(p["title"], "Go");
    assert_eq!(p["text_size"], 22.0);
    assert_eq!(p["text_style"], "bold,italic");
    assert_eq!(p["text_font"], "serif");
    assert_eq!(p["text_align"], "center");

    // Renaming by name works; verify by id since the old name no longer resolves.
    run_on_inc(&mut s, r#"set the name of button "Next" to "Forward""#);
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(21).unwrap()).unwrap();
    assert_eq!(p["name"], "Forward");

    // An unknown button property errors.
    assert!(run_on_inc_err(&mut s, r#"set the bogus of button "Inc" to "1""#).is_some());
}

#[test]
fn sets_single_edge_geometry() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    // "input" field id 11 starts at rect {x:10,y:50,w:100,h:30}.
    // top/left move the origin and keep the size.
    run_on_inc(&mut s, r#"set the top of field "input" to "5""#);
    run_on_inc(&mut s, r#"set the left of field "input" to "7""#);
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(11).unwrap()).unwrap();
    assert_eq!(p["y"], 5.0);
    assert_eq!(p["x"], 7.0);
    assert_eq!(p["w"], 100.0); // unchanged
    assert_eq!(p["h"], 30.0);

    // bottom/right keep the size by shifting the origin: y = bottom - h, x = right - w.
    run_on_inc(&mut s, r#"set the bottom of field "input" to "100""#);
    run_on_inc(&mut s, r#"set the right of field "input" to "90""#);
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(11).unwrap()).unwrap();
    assert_eq!(p["y"], 70.0); // 100 - 30
    assert_eq!(p["x"], -10.0); // 90 - 100
    assert_eq!(p["h"], 30.0);
    assert_eq!(p["w"], 100.0);
}

#[test]
fn unicode_comparison_operators() {
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    run_on_inc(&mut s, "put (1 ≠ 2) into field \"counter\"");
    assert_eq!(field_text(&s, 10), "true");
    run_on_inc(&mut s, "put (2 ≤ 2) into field \"counter\"");
    assert_eq!(field_text(&s, 10), "true");
    run_on_inc(&mut s, "put (3 ≥ 4) into field \"counter\"");
    assert_eq!(field_text(&s, 10), "false");
}

#[test]
fn lexer_handles_comments_and_unterminated_strings() {
    // Trailing and whole-line `--` comments are ignored.
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    run_on_inc(
        &mut s,
        "put 7 into field \"counter\" -- trailing comment\n  -- whole-line comment",
    );
    assert_eq!(field_text(&s, 10), "7");

    // An unterminated string literal is a lexer error, surfaced via check_script.
    assert!(Session::check_script("on mouseUp\n  put \"oops into field x\nend mouseUp").is_some());
}

#[test]
fn parses_handler_with_parameter_list() {
    // Exercises the comma token and the parameter-list parse loop.
    let script =
        parse_script("on myMessage a, b\n  put a into field \"x\"\nend myMessage").unwrap();
    assert_eq!(script.handlers[0].params, vec!["a", "b"]);
}

/// A stack for touchscreen-gesture dispatch: a locked field whose long-press writes it, and a
/// stack-level swipe handler for card navigation. Two cards so swipe-to-navigate is observable.
fn gesture_json() -> String {
    r#"{
      "name": "G",
      "script": "on swipeLeft\n  go next card\nend swipeLeft",
      "cards": [
        { "id": 1, "name": "One",
          "fields": [
            { "id": 5, "name": "out", "rect": {"x":0,"y":0,"w":100,"h":40}, "text": "idle", "locked": true }
          ],
          "buttons": [
            { "id": 6, "name": "hold", "rect": {"x":0,"y":60,"w":100,"h":40},
              "script": "on longPress\n  put \"held\" into field \"out\"\nend longPress" }
          ] },
        { "id": 2, "name": "Two" }
      ]
    }"#
    .to_string()
}

#[test]
fn long_press_runs_object_gesture_handler() {
    let mut s = Session::load_from_json(&gesture_json()).unwrap();
    // Long-press the "hold" button (rect y 60..100): its `on longPress` fires.
    let r = s.dispatch_gesture(10.0, 70.0, "longPress");
    assert!(r.error.is_none(), "error: {:?}", r.error);
    assert_eq!(field_text(&s, 5), "held");
}

#[test]
fn swipe_bubbles_to_stack_handler_and_navigates() {
    let mut s = Session::load_from_json(&gesture_json()).unwrap();
    assert_eq!(s.card_index(), 0);
    // A swipe over empty space has no object to handle it, so it bubbles to the stack's
    // `on swipeLeft`, which navigates. Matching is case-insensitive (host sends "swipeleft").
    let r = s.dispatch_gesture(10.0, 200.0, "swipeleft");
    assert!(r.error.is_none(), "error: {:?}", r.error);
    assert!(r.card_changed);
    assert_eq!(s.card_index(), 1);
}

#[test]
fn swipe_over_object_without_handler_still_bubbles() {
    // The gesture starts over the "hold" button, which only handles longPress; the swipe must
    // bubble past it to the stack handler rather than being swallowed.
    let mut s = Session::load_from_json(&gesture_json()).unwrap();
    let r = s.dispatch_gesture(10.0, 70.0, "swipeLeft");
    assert!(r.card_changed);
    assert_eq!(s.card_index(), 1);
}

#[test]
fn unhandled_gesture_is_a_noop() {
    let mut s = Session::load_from_json(&gesture_json()).unwrap();
    // No `on swipeRight` anywhere: no error, no navigation, field unchanged.
    let r = s.dispatch_gesture(10.0, 70.0, "swipeRight");
    assert!(r.error.is_none(), "error: {:?}", r.error);
    assert!(!r.card_changed);
    assert_eq!(field_text(&s, 5), "idle");
}

#[test]
fn gesture_targets_locked_field_without_focusing() {
    // A gesture on an unlocked field must NOT request focus (that is the tap path's job);
    // it runs the field's gesture handler instead.
    let json = r#"{
      "name": "G", "cards": [
        { "id": 1, "name": "One", "fields": [
          { "id": 7, "name": "edit", "rect": {"x":0,"y":0,"w":100,"h":40},
            "script": "on longPress\n  set the locked of me to true\nend longPress" }
        ] }
      ]
    }"#;
    let mut s = Session::load_from_json(json).unwrap();
    let r = s.dispatch_gesture(10.0, 10.0, "longPress");
    assert!(r.error.is_none(), "error: {:?}", r.error);
    assert_eq!(r.focus_field, None); // never focuses on a gesture
    let p: serde_json::Value = serde_json::from_str(&s.get_object_props(7).unwrap()).unwrap();
    assert_eq!(p["locked"], true);
}

#[test]
fn loads_from_yaml_with_block_scalar_script() {
    // YAML authoring (ADR-0011): the script reads as real indented text via a `|` block scalar,
    // not a "\n"-escaped JSON string — and it still runs.
    let yaml = r#"
name: Y
cards:
  - id: 1
    name: One
    fields:
      - id: 5
        name: out
        rect: { x: 0, y: 0, w: 50, h: 50 }
        text: "0"
        locked: true
    buttons:
      - id: 6
        name: Inc
        rect: { x: 0, y: 60, w: 50, h: 50 }
        script: |
          on mouseUp
            add 1 to field "out"
          end mouseUp
"#;
    let mut s = Session::load_from_yaml(yaml).unwrap();
    assert_eq!(s.card_count(), 1);
    let r = s.dispatch_touch(10.0, 70.0, "up"); // tap "Inc"
    assert!(r.error.is_none(), "error: {:?}", r.error);
    assert_eq!(field_text(&s, 5), "1");
}

#[test]
fn yaml_and_json_round_trip_to_the_same_model() {
    // Parity: YAML and JSON deserialize to the same `Stack` (ADR-0011 keeps one model).
    use crate::model::Stack;
    let stack: Stack = serde_json::from_str(&sample_json()).unwrap();
    let yaml = yaml_serde::to_string(&stack).unwrap();
    let from_yaml: Stack = yaml_serde::from_str(&yaml).unwrap();
    assert_eq!(stack, from_yaml);
    // ...and the YAML-loaded model re-serializes to JSON identically.
    let json = serde_json::to_string(&from_yaml).unwrap();
    let from_json: Stack = serde_json::from_str(&json).unwrap();
    assert_eq!(stack, from_json);
}

#[test]
fn invalid_yaml_errors_cleanly() {
    assert!(Session::load_from_yaml("name: [unterminated").is_err());
}

#[test]
fn typed_object_props_read_and_apply() {
    // The typed props path (ADR-0012) used by the UniFFI bridge — no JSON.
    let mut s = Session::load_from_json(&sample_json()).unwrap();
    let mut p = s.object_props(20).unwrap(); // "Inc" button
    assert_eq!(p.kind, "button");
    assert_eq!(p.name, "Inc");
    p.title = "Plus".to_string();
    p.style = "rectangle".to_string();
    p.text_size = 22.0;
    assert!(s.apply_object_props(&p));
    let p2 = s.object_props(20).unwrap();
    assert_eq!(p2.title, "Plus");
    assert_eq!(p2.style, "rectangle");
    assert_eq!(p2.text_size, 22.0);

    // A field reports text/locked; applying flips locked and sets text.
    let mut f = s.object_props(11).unwrap();
    assert_eq!(f.kind, "field");
    f.locked = true;
    f.text = "hi".to_string();
    assert!(s.apply_object_props(&f));
    let f2 = s.object_props(11).unwrap();
    assert!(f2.locked);
    assert_eq!(f2.text, "hi");

    assert!(s.object_props(999).is_none());
    assert!(!s.apply_object_props(&ObjectProps { id: 999, ..p2 }));
}

#[test]
fn to_yaml_round_trips_through_load() {
    // Runtime saves are YAML now (ADR-0011 Phase A): to_yaml -> load_from_yaml is the identity.
    let s = Session::load_from_json(&sample_json()).unwrap();
    let s2 = Session::load_from_yaml(&s.to_yaml()).unwrap();
    assert_eq!(s.to_json(), s2.to_json());
}

/// Direct unit tests for the string-centric `Value` coercions.
mod value_unit {
    use crate::script::value::{Value, fmt_number};

    #[test]
    fn fmt_number_matches_hypertalk() {
        assert_eq!(fmt_number(0.0), "0");
        assert_eq!(fmt_number(42.0), "42");
        assert_eq!(fmt_number(-7.0), "-7");
        assert_eq!(fmt_number(3.5), "3.5");
        assert_eq!(fmt_number(-0.25), "-0.25");
        // Past the integral-formatting threshold, fall back to the float rendering.
        assert_eq!(fmt_number(1e16), "10000000000000000");
        assert_eq!(fmt_number(f64::INFINITY), "inf");
        assert_eq!(fmt_number(f64::NAN), "NaN");
    }

    #[test]
    fn as_number_coerces_string_centric() {
        assert_eq!(Value::Number(3.0).as_number(), Some(3.0));
        assert_eq!(Value::Empty.as_number(), Some(0.0));
        assert_eq!(Value::from_text("  42  ").as_number(), Some(42.0)); // trimmed
        assert_eq!(Value::from_text("").as_number(), Some(0.0));
        assert_eq!(Value::from_text("abc").as_number(), None);
        assert_eq!(Value::Bool(true).as_number(), None);
    }

    #[test]
    fn as_bool_coerces_string_centric() {
        assert!(Value::Bool(true).as_bool());
        assert!(!Value::Empty.as_bool());
        assert!(Value::Number(2.0).as_bool());
        assert!(!Value::Number(0.0).as_bool());
        assert!(Value::from_text("TRUE").as_bool()); // case-insensitive
        assert!(!Value::from_text("nope").as_bool());
    }

    #[test]
    fn display_matches_as_text() {
        assert_eq!(format!("{}", Value::Number(5.0)), "5");
        assert_eq!(format!("{}", Value::from_text("hi")), "hi");
        assert_eq!(format!("{}", Value::Bool(true)), "true");
        assert_eq!(format!("{}", Value::Empty), "");
    }

    #[test]
    fn as_text_and_is_empty() {
        assert_eq!(Value::Number(5.0).as_text(), "5");
        assert_eq!(Value::Bool(false).as_text(), "false");
        assert_eq!(Value::Empty.as_text(), "");
        assert!(Value::Empty.is_empty());
        assert!(Value::from_text("").is_empty());
        assert!(!Value::from_text("x").is_empty());
        assert!(!Value::Number(0.0).is_empty()); // a number is never "empty"
    }
}
