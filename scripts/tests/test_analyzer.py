import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.quality.analyzer import AnalysisError, analyze, check_versions


def decisions(count, rust=False):
    statement = "if ready { run(); }" if rust else "if (ready) run();"
    return "\n".join(statement for _ in range(count))


class AnalyzerTests(unittest.TestCase):
    def test_exact_pins(self):
        check_versions()

    def test_wrong_analyzer_version_fails(self):
        with patch("scripts.quality.analyzer.version", return_value="0.0.0"):
            with self.assertRaisesRegex(AnalysisError, "expected"):
                check_versions()

    def test_thresholds_for_named_methods_and_closures(self):
        templates = [
            ("fixture.ts", "function work() { BODY }", "work"),
            ("fixture.ts", "class Worker { work() { BODY } }", "Worker::work"),
            ("fixture.tsx", "function Component() { BODY return <div/>; }", "Component"),
            ("fixture.tsx", "const work = () => { BODY return <div/>; };", "work"),
            ("fixture.rs", "fn work() { BODY }", "work"),
            ("fixture.rs", "impl Worker { fn work(&self) { BODY } }", "Worker::work"),
            ("fixture.rs", "unsafe extern \"system\" fn work() { BODY }", "work"),
            ("fixture.rs", "fn outer() { let work = move || unsafe { BODY }; }", "outer::work"),
        ]
        for path, template, symbol in templates:
            for score in (10, 11):
                with self.subTest(path=path, symbol=symbol, score=score):
                    source = template.replace("BODY", decisions(score - 1, path.endswith(".rs")))
                    functions = {function.name: function for function in analyze(path, source)}
                    self.assertEqual(functions[symbol].score, score)

    def test_expression_closures_and_arrows(self):
        for path, source, name in [
            ("f.ts", "const work = x => x ? yes : no;", "work"),
            ("f.rs", "fn outer() { let work = |x| if x { 1 } else { 0 }; }", "outer::work"),
            ("f.rs", "fn outer() { let work = || if yes { 1 } else { 0 }; }", "outer::work"),
        ]:
            with self.subTest(path=path, source=source):
                self.assertEqual(next(f.score for f in analyze(path, source) if f.name == name), 2)

    def test_parameter_default_decisions_are_in_function_score(self):
        self.assertEqual(analyze("f.ts", "function work(value = yes ? 1 : 0) { return value; }")[0].score, 2)
        self.assertEqual(analyze("f.ts", "const work = ({value = yes ? 1 : 0} = {}) => value;")[0].score, 2)
        for score in (10, 11):
            source = "function work(value = [" + ",".join("yes ? 1 : 0" for _ in range(score - 1)) + "]) {}"
            self.assertEqual(analyze("f.ts", source)[0].score, score)

    def test_nested_functions_score_independently(self):
        old = "function outer() {\n function inner() {\n  return 0;\n }\n return inner();\n}"
        new = old.replace("return 0;", "if (yes) return 1; return 0;")
        after = analyze("f.ts", new)
        self.assertEqual({f.name: f.score for f in after}, {"outer": 1, "outer::inner": 2})

    def test_react_jsx_callbacks_template_text_and_optional_operators(self):
        source = '''function App() {
          const title = `if while ?? ${ready ? yes : no}`;
          return <div label="if ?? while" onClick={() => { if (ready) run(); }}>
            while if ?? {state?.value ?? fallback}{ready && <span/>}
          </div>;
        }'''
        functions = analyze("f.tsx", source)
        self.assertEqual(functions[0].score, 5)
        self.assertEqual(functions[1].score, 2)

    def test_rust_match_guards_try_loop_and_let_else(self):
        source = '''fn work<T>() where T: Ready {
          let Some(value) = get() else { return; };
          loop { break; }
          run()?;
          match value { A | B => 1, C if ready && yes => 2, _ => 3 }
        }'''
        self.assertEqual(analyze("f.rs", source)[0].score, 8)

    def test_types_strings_comments_regex_are_not_branches(self):
        source = r'''function work<T>() {
          type Maybe<U> = U extends T ? U : never;
          const data = /if\?\?while/;
          const name = "if ? && ||";
          // if (yes) while (ready) run();
          return data;
        }'''
        self.assertEqual(analyze("f.ts", source)[0].score, 1)
        rust = 'fn work<T>() where T: Ready { let data = r#"if || ? match"#; /* if */ }'
        self.assertEqual(analyze("f.rs", rust)[0].score, 1)

    def test_macro_expression_arguments_do_not_hide_closures(self):
        source = 'fn outer() { call!(|| { if ready { run(); } }); }'
        functions = analyze("f.rs", source)
        self.assertEqual([f.score for f in functions], [1, 2])
        nested = 'fn outer() { call!(another!(|| { if ready { run(); } })); }'
        self.assertEqual([f.score for f in analyze("f.rs", nested)], [1, 2])

    def test_unknown_syntax_and_opaque_macros_fail_clearly(self):
        for path, source in [("f.ts", "function work( {"),
                             ("f.rs", "macro_rules! hidden { () => { fn work() {} } }"),
                             ("f.rs", "hidden! { fn work() {} }")]:
            with self.subTest(path=path, source=source), self.assertRaises(AnalysisError):
                analyze(path, source)

    def test_repo_function_forms_are_supported(self):
        root = Path(__file__).resolve().parents[2]
        for relative in ("app/src/main.tsx", "app/vite.config.ts", "app/src-tauri/build.rs",
                         "app/src-tauri/src/main.rs", "app/src-tauri/src/lib.rs", "app/src-tauri/src/windows.rs"):
            with self.subTest(path=relative):
                analyze(relative, (root / relative).read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
