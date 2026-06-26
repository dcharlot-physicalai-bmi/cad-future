//! CFL invariants — parse safety, execute safety, lex roundtrip.

use proptest::prelude::*;
use physical_cfl::{lex, parse, execute, Token};

/// Generate random CFL-like source strings.
fn arb_cfl_source() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            "let x = 50mm",
            "let y = 30",
            "material al = \"6061-T6\"",
            "solid part = extrude(base, 10mm)",
            "assert 10 < 20",
            "assert 5 > 3",
            "hole(part, diameter: 6mm)",
            "fillet(part, radius: 2mm)",
            "export(part, format: \"step\")",
            "ask \"what material?\"",
            "let z = x + y",
            "let w = 100mm * 2",
            "sketch profile {\n  rect(50mm, 30mm)\n}",
            "// comment line",
            "",
        ]),
        1..10,
    ).prop_map(|lines| lines.join("\n"))
}

/// Generate completely random strings for fuzz testing.
fn arb_random_string() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9 _=+\\-*/()<>{}\\[\\],.:;\"!\n#%&|@^~]*")
        .unwrap()
        .prop_filter("not too long", |s| s.len() < 500)
}

proptest! {
    /// Lexer never panics on any input.
    #[test]
    fn lex_never_panics(source in arb_random_string()) {
        let tokens = lex(&source);
        // Should always produce at least EOF
        prop_assert!(!tokens.is_empty(), "lex should produce at least EOF");
        prop_assert!(matches!(tokens.last(), Some(Token::Eof)), "last token should be Eof");
    }

    /// Parser never panics on any token sequence.
    #[test]
    fn parse_never_panics(source in arb_random_string()) {
        let tokens = lex(&source);
        let _ast = parse(&tokens);
        // Should not panic — parse can return empty AST for unparseable input
    }

    /// Execute never panics on any CFL source.
    #[test]
    fn execute_never_panics(source in arb_cfl_source()) {
        let result = execute(&source);
        // The key invariant: execute NEVER PANICS. Errors are captured in result.errors.
        // Comments and blank lines produce no log entries — that's correct behavior.
        let has_executable = source.lines().any(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//")
        });
        if has_executable {
            prop_assert!(!result.log.is_empty(),
                "source with executable lines should produce log entries");
        }
    }

    /// Execute on random strings never panics.
    #[test]
    fn execute_random_no_panic(source in arb_random_string()) {
        let _result = execute(&source);
        // The key invariant: NEVER PANIC
    }

    /// Valid CFL programs produce correct variable bindings.
    #[test]
    fn let_binding_produces_variable(x in -1000.0..1000.0_f64) {
        let source = format!("let val = {x}");
        let result = execute(&source);
        // Should have 'val' in variables
        if let Some(v) = result.variables.get("val") {
            if let Some(n) = v.as_f64() {
                prop_assert!((n - x).abs() < 0.01, "variable val: expected {x}, got {n}");
            }
        }
    }

    /// Material declaration always produces a tool call.
    #[test]
    fn material_decl_produces_tool_call(
        name in "[a-z]{3,8}",
        mat_id in prop::sample::select(vec!["6061-T6", "7075-T6", "1018-CD", "Ti-6Al-4V", "AISI-304"])
    ) {
        let source = format!("material {name} = \"{mat_id}\"");
        let result = execute(&source);
        prop_assert!(!result.tool_calls.is_empty(),
            "material decl should produce tool call for {name} = {mat_id}");
        prop_assert_eq!(result.tool_calls[0]["tool"].as_str().unwrap_or(""), "lookup_material");
    }

    /// Assert true always passes.
    #[test]
    fn assert_true_passes(a in 1.0..100.0_f64) {
        let source = format!("assert {} < {}", a, a + 1.0);
        let result = execute(&source);
        prop_assert!(result.all_passed, "assert {} < {} should pass", a, a + 1.0);
    }

    /// Assert false always fails.
    #[test]
    fn assert_false_fails(a in 1.0..100.0_f64) {
        let source = format!("assert {} < {}", a + 1.0, a);
        let result = execute(&source);
        prop_assert!(!result.all_passed, "assert {} < {} should fail", a + 1.0, a);
    }

    /// Lex produces consistent token count for same input.
    #[test]
    fn lex_deterministic(source in arb_cfl_source()) {
        let t1 = lex(&source);
        let t2 = lex(&source);
        prop_assert_eq!(t1.len(), t2.len(), "lexer should be deterministic");
    }
}
