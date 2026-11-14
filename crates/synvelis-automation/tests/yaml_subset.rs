//! YAML safe-subset parser golden rejections and accepts.

mod common;

use synvelis_automation::yaml::YamlValue;
use synvelis_automation::yaml::parse;

fn assert_yaml_err(src: &str) {
    assert!(parse(src).is_err(), "expected YAML rejection for: {src:?}");
}

#[test]
fn accepts_block_subset() {
    let v = parse("a: 1\nb:\n  - x\n  - y\n").unwrap();
    let YamlValue::Map(m) = v else { panic!("map") };
    assert_eq!(m["a"], YamlValue::Int(1));
}

#[test]
fn rejects_anchor_alias_tag() {
    assert_yaml_err("a: &x 1\nb: *x\n");
    assert_yaml_err("a: !foo bar\n");
    assert_yaml_err("a: &anchor value\n");
    assert_yaml_err("a: *alias\n");
}

#[test]
fn rejects_flow_collections() {
    assert_yaml_err("a: [1, 2]\n");
    assert_yaml_err("a: {x: 1}\n");
    assert_yaml_err("a:\n  - [1]\n");
}

#[test]
fn rejects_block_scalars_and_merge() {
    assert_yaml_err("a: |\n  text\n");
    assert_yaml_err("a: >\n  text\n");
    assert_yaml_err("<<: {x: 1}\n");
    assert_yaml_err("? complex\n: value\n");
}

#[test]
fn implicit_types_are_restricted() {
    // yes/on/hex/dates stay strings — never silently typed.
    let v = parse("a: yes\nb: 0x10\nc: 2024-01-01\n").unwrap();
    let YamlValue::Map(m) = v else { panic!("map") };
    assert_eq!(m["a"], YamlValue::Str("yes".into()));
    assert_eq!(m["b"], YamlValue::Str("0x10".into()));
    assert_eq!(m["c"], YamlValue::Str("2024-01-01".into()));
    // core-schema spellings only.
    let v = parse("a: True\nb: FALSE\nc: ~\nd: null\n").unwrap();
    let YamlValue::Map(m) = v else { panic!("map") };
    assert_eq!(m["a"], YamlValue::Bool(true));
    assert_eq!(m["b"], YamlValue::Bool(false));
    assert_eq!(m["c"], YamlValue::Null);
    assert_eq!(m["d"], YamlValue::Null);
}

#[test]
fn rejects_noncanonical_numbers() {
    // Leading zeros are not canonical ints/floats → stay strings.
    let v = parse("a: 01\nb: 1.50e+03\n").unwrap();
    let YamlValue::Map(m) = v else { panic!("map") };
    assert_eq!(m["a"], YamlValue::Str("01".into()));
    assert!(matches!(m["b"], YamlValue::Float(_)));
    // i64 overflow is rejected outright.
    assert_yaml_err("a: 99999999999999999999999\n");
}

#[test]
fn rejects_duplicate_keys_unknown_indent() {
    assert_yaml_err("a: 1\na: 2\n");
    assert_yaml_err("a:\n    b: 1\n  c: 2\n");
    assert_yaml_err("a: 1\n b: 2\n");
}

#[test]
fn rejects_oversized_and_deep() {
    let big = "a: ".to_owned() + &"x".repeat(synvelis_automation::yaml::MAX_YAML_BYTES + 1);
    assert!(parse(&big).is_err());
    // Depth beyond MAX_YAML_DEPTH.
    let mut deep = String::new();
    for i in 0..80 {
        deep.push_str(&" ".repeat(i));
        deep.push_str("k:\n");
    }
    deep.push_str(&" ".repeat(80));
    deep.push_str("v: 1\n");
    assert_yaml_err(&deep);
}

#[test]
fn rejects_multidoc_and_directives() {
    assert_yaml_err("---\na: 1\n---\nb: 2\n");
    assert_yaml_err("%YAML 1.2\n---\na: 1\n");
    assert_yaml_err("a: 1\n...\nb: 2\n");
}

#[test]
fn quoted_scalars() {
    let v = parse("a: 'it''s'\nb: \"x\\ny\"\n").unwrap();
    let YamlValue::Map(m) = v else { panic!("map") };
    assert_eq!(m["a"], YamlValue::Str("it's".into()));
    assert_eq!(m["b"], YamlValue::Str("x\ny".into()));
    assert_yaml_err("a: 'unterminated\n");
    assert_yaml_err("a: \"bad \\q escape\"\n");
}

#[test]
fn single_quoted_preserves_utf8() {
    // Regression: single-quoted scalars must copy UTF-8 scalars, not
    // raw bytes — non-ASCII text must not be mojibake'd.
    let v = parse("a: 'café — 日本語'\n").unwrap();
    let YamlValue::Map(m) = v else { panic!("map") };
    assert_eq!(m["a"], YamlValue::Str("café — 日本語".into()));
}

#[test]
fn comments_strip_everywhere() {
    // Inline comments are stripped quote-aware for keys, sequence
    // items, and values alike.
    let v = parse("- foo # trailing\n- \"bar\" # trailing\n- 'a # b' # c\n- # only a comment\n")
        .unwrap();
    let YamlValue::Seq(items) = v else {
        panic!("seq")
    };
    assert_eq!(items[0], YamlValue::Str("foo".into()));
    assert_eq!(items[1], YamlValue::Str("bar".into()));
    assert_eq!(items[2], YamlValue::Str("a # b".into()));
    assert_eq!(items[3], YamlValue::Null);
    // A `#` inside quotes survives; `#` not preceded by space is data.
    let v = parse("a: 'x # y' # c\nb: foo#bar\n").unwrap();
    let YamlValue::Map(m) = v else { panic!("map") };
    assert_eq!(m["a"], YamlValue::Str("x # y".into()));
    assert_eq!(m["b"], YamlValue::Str("foo#bar".into()));
}

#[test]
fn rejects_tab_indentation() {
    // Tabs in leading whitespace are malformed indentation.
    assert_yaml_err("\tkey: value\n");
    assert_yaml_err("key:\n\t  nested: 1\n");
    assert_yaml_err("  \tkey: value\n");
    // A tab inside scalar content is data, not indentation.
    let v = parse("a: foo\tbar\n").unwrap();
    let YamlValue::Map(m) = v else { panic!("map") };
    assert_eq!(m["a"], YamlValue::Str("foo\tbar".into()));
}

#[test]
fn end_of_document_marker() {
    // `...` ends the document; only blanks/comments may follow.
    let v = parse("a: 1\n...\n# done\n").unwrap();
    let YamlValue::Map(m) = v else { panic!("map") };
    assert_eq!(m["a"], YamlValue::Int(1));
    // Content after `...` is a second document → rejected.
    assert_yaml_err("a: 1\n...\nb: 2\n");
}

#[test]
fn inline_seq_map_forms() {
    let v = parse("- a: 1\n  b: 2\n- c: 3\n").unwrap();
    let YamlValue::Seq(items) = v else {
        panic!("seq")
    };
    assert_eq!(items.len(), 2);
    let v = parse("- - 1\n  - 2\n").unwrap();
    assert!(matches!(v, YamlValue::Seq(_)));
}
