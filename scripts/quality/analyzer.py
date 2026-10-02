"""AST-assisted, pinned Lizard cyclomatic complexity (source functions only).

The syntax tree supplies exact function spans and removes non-executable text
and nested functions. Lizard counts the projected body; documented adapters
normalize operators its readers do not understand. No regex guesses at spans.
"""

from collections import Counter
from dataclasses import dataclass
from hashlib import sha256
from importlib.metadata import version
from pathlib import PurePosixPath

from .environment import PINS

import lizard
from tree_sitter import Language, Parser
import tree_sitter_rust
import tree_sitter_typescript


FUNCTIONS = {
    "function_item", "closure_expression", "function_declaration",
    "function_expression", "generator_function_declaration", "generator_function",
    "arrow_function", "method_definition",
}
LITERALS = {
    "string", "string_literal", "raw_string_literal", "char_literal", "regex",
    "number", "integer_literal", "float_literal", "boolean_literal", "null",
}
TYPES = {
    "type_annotation", "type_arguments", "type_parameters", "type_alias_declaration",
    "interface_declaration", "function_signature", "abstract_method_signature",
}
IDENTIFIERS = {
    "identifier", "property_identifier", "private_property_identifier",
    "field_identifier", "type_identifier", "shorthand_property_identifier",
    "shorthand_property_identifier_pattern", "lifetime", "label",
}


class AnalysisError(ValueError):
    """Analysis could not establish a trustworthy result."""


@dataclass(frozen=True)
class Function:
    name: str
    start: int
    score: int


def check_versions():
    for package, expected in PINS.items():
        actual = version(package)
        if actual != expected:
            raise AnalysisError(f"{package}: expected {expected}, found {actual}; rerun quality setup")


def parser_for(path):
    suffix = PurePosixPath(path).suffix
    languages = {
        ".rs": tree_sitter_rust.language,
        ".ts": tree_sitter_typescript.language_typescript,
        ".tsx": tree_sitter_typescript.language_tsx,
    }
    if suffix not in languages:
        raise AnalysisError(f"unsupported source extension: {path}")
    return Parser(Language(languages[suffix]()))


def descendants(node):
    yield node
    for child in node.named_children:
        yield from descendants(child)


def parse_source(path, source):
    parser = parser_for(path)
    data = source.encode("utf-8")
    tree = parser.parse(data)
    if tree.root_node.has_error:
        raise AnalysisError(f"{path}: syntax error; changed-function mapping cannot be determined")
    if path.endswith(".rs"):
        for _ in range(64):
            expanded = expand_macro_arguments(path, data, tree)
            if expanded == data:
                break
            data = expanded
            tree = parser.parse(data)
            if tree.root_node.has_error:
                raise AnalysisError(f"{path}: unsupported Rust macro argument syntax; add parser support")
        else:
            raise AnalysisError(f"{path}: macro nesting exceeds the supported parser depth")
    return data, tree.root_node


def expand_macro_arguments(path, data, tree):
    """Parse expression macro arguments as calls, retaining all byte offsets.

    Macro-generated code is outside source complexity. Token trees that cannot
    be parsed as expression arguments fail rather than hiding source closures.
    """
    projected = bytearray(data)
    for node in descendants(tree.root_node):
        if node.type == "macro_definition":
            raise AnalysisError(f"{path}:{node.start_point.row + 1}: unsupported macro definition")
        if node.type != "macro_invocation":
            continue
        tokens = next((child for child in node.named_children if child.type == "token_tree"), None)
        if tokens is None:
            raise AnalysisError(f"{path}: macro argument mapping cannot be determined")
        bang = data.rfind(b"!", node.start_byte, tokens.start_byte)
        if bang < 0:
            raise AnalysisError(f"{path}: macro delimiter mapping cannot be determined")
        projected[bang] = ord(" ")
        projected[tokens.start_byte] = ord("(")
        projected[tokens.end_byte - 1] = ord(")")
    return bytes(projected)


def text(node, data):
    return data[node.start_byte:node.end_byte].decode("utf-8")


def function_label(node, data, ordinal):
    named = node.child_by_field_name("name")
    if named is not None:
        return text(named, data)
    parent = node.parent
    if parent is not None and parent.type in {"variable_declarator", "let_declaration"}:
        binding = parent.child_by_field_name("name") or parent.child_by_field_name("pattern")
        if binding is not None and binding.type == "identifier":
            return text(binding, data)
    digest = sha256(data[node.start_byte:node.end_byte]).hexdigest()[:12]
    return f"<closure#{ordinal}:{digest}>"


def namespace(node, data):
    if node.type in {"class_declaration", "class", "mod_item", "internal_module"}:
        name = node.child_by_field_name("name")
        return text(name, data) if name else "<anonymous-class>"
    if node.type == "impl_item":
        owner = node.child_by_field_name("type")
        trait = node.child_by_field_name("trait")
        label = text(owner, data) if owner else "<unknown-impl>"
        return f"{label} as {text(trait, data)}" if trait else label
    return None


def function_start(node):
    start = node.start_point.row + 1
    if node.parent and node.parent.type == "export_statement":
        start = node.parent.start_point.row + 1
    previous = node.prev_named_sibling
    while previous and previous.type == "attribute_item":
        start = previous.start_point.row + 1
        previous = previous.prev_named_sibling
    return start


def analyze(path, source):
    data, root = parse_source(path, source)
    functions = []
    ordinals = Counter()

    def visit(node, owners):
        scope = namespace(node, data)
        if scope:
            owners = (*owners, scope)
        if node.type in FUNCTIONS:
            body = node.child_by_field_name("body")
            if body is None:
                raise AnalysisError(f"{path}:{node.start_point.row + 1}: unsupported function body")
            ordinals[owners] += 1
            label = function_label(node, data, ordinals[owners])
            owners = (*owners, label)
            name = "::".join(owners)
            score = score_body(path, body, data, parameter_values(node))
            functions.append(Function(name, function_start(node), score))
        for child in node.named_children:
            visit(child, owners)

    visit(root, ())
    names = [function.name for function in functions]
    if len(names) != len(set(names)):
        raise AnalysisError(f"{path}: duplicate function identity; changed-function mapping is ambiguous")
    return functions


def project(node, data, rust):
    """Project executable syntax to tokens understood by Lizard's reader."""
    kind = node.type
    if kind in {"function_declaration", "generator_function_declaration", "function_item"}:
        return ";"
    if kind in FUNCTIONS or "comment" in kind:
        return "0"
    if kind in TYPES:
        return ""
    if kind in LITERALS or kind in IDENTIFIERS:
        return "0" if kind in LITERALS else "v_" + sha256(node.text).hexdigest()[:10]
    if kind == "jsx_expression":
        return "( " + " ".join(project(child, data, rust) for child in node.named_children) + " )"
    if kind.startswith("jsx_"):
        expressions = []
        for child in node.named_children:
            if child.type.startswith("jsx_"):
                expressions.append(project(child, data, rust))
        return "( 0 " + "".join(", " + expression for expression in expressions) + " )"
    if kind == "template_string":
        substitutions = [project(child, data, rust) for child in node.named_children
                         if child.type == "template_substitution"]
        return "( 0 " + "".join(", " + substitution for substitution in substitutions) + " )"
    if kind == "template_substitution":
        return "( " + " ".join(project(child, data, rust) for child in node.named_children) + " )"
    if kind == "optional_chain":
        return "||"
    if node.child_count == 0:
        token = text(node, data)
        replacements = {"??": "||", "??=": "||", "&&=": "&&", "||=": "||", "loop": "while"}
        return replacements.get(token, token)
    tokens = " ".join(project(child, data, rust) for child in node.children)
    if rust and kind == "let_declaration" and node.child_by_field_name("alternative"):
        tokens += " if value { }"
    return tokens


def match_adjustment(node):
    if node.type in FUNCTIONS:
        return 0
    adjustment = 0
    if node.type == "match_expression":
        body = node.child_by_field_name("body")
        arms = sum(child.type == "match_arm" for child in body.named_children)
        # Lizard counts match once. Our policy counts N outcomes as N - 1.
        adjustment = max(0, arms - 1) - 1
    return adjustment + sum(match_adjustment(child) for child in node.named_children)


def parameter_values(function):
    parameters = function.child_by_field_name("parameters") or function.child_by_field_name("parameter")
    if parameters is None:
        return []
    values = []

    def visit(node):
        if node.type in FUNCTIONS or node.type in TYPES:
            return
        if node.type in {"required_parameter", "optional_parameter", "assignment_pattern", "object_assignment_pattern"}:
            value = node.child_by_field_name("value") or node.child_by_field_name("right")
            if value:
                values.append(value)
                pattern = node.child_by_field_name("pattern") or node.child_by_field_name("left")
                if pattern:
                    visit(pattern)
                return
        for child in node.named_children:
            visit(child)

    visit(parameters)
    return values


def score_body(path, body, data, defaults=()):
    rust = path.endswith(".rs")
    projected = " ; ".join(project(value, data, rust) for value in defaults)
    projected += " ; " + project(body, data, rust)
    # TypeScript's heuristic state machine can end a function at JSX-derived
    # expressions. The projected tokens use the C++ decision reader, which has
    # the same branch operators and no TypeScript declaration heuristics.
    wrapper = f"fn quality() {{ {projected} }}" if rust else f"void quality() {{ {projected} }}"
    result = lizard.analyze_file.analyze_source_code("body.rs" if rust else "body.cpp", wrapper)
    if len(result.function_list) != 1 or result.function_list[0].name != "quality":
        raise AnalysisError(f"{path}:{body.start_point.row + 1}: analyzer function mapping failed")
    score = result.function_list[0].cyclomatic_complexity
    if rust:
        score += match_adjustment(body)
    expected = 1 + decision_count(body, rust) + sum(decision_count(value, rust) for value in defaults)
    if score != expected:
        raise AnalysisError(f"{path}:{body.start_point.row + 1}: unsupported scoring syntax: Lizard observed {score}, syntax policy expected {expected}")
    return score


def decision_count(node, rust):
    """Independent reconciliation prevents projection/parser false passes."""
    if node.type in FUNCTIONS or node.type in TYPES or "comment" in node.type:
        return 0
    branches = {
        "if_statement", "for_statement", "for_in_statement", "while_statement",
        "do_statement", "catch_clause", "ternary_expression", "switch_case",
        "optional_chain", "if_expression", "for_expression", "while_expression",
        "loop_expression", "try_expression",
    }
    count = int(node.type in branches)
    if node.type in {"binary_expression", "augmented_assignment_expression"}:
        operator = node.child_by_field_name("operator")
        if operator and operator.text in {b"&&", b"||", b"??", b"&&=", b"||=", b"??="}:
            count += 1
    if rust and node.type == "match_expression":
        arms = node.child_by_field_name("body").named_children
        count += max(0, sum(arm.type == "match_arm" for arm in arms) - 1)
    if rust and node.type == "match_pattern" and node.child_by_field_name("condition"):
        count += 1
    if rust and node.type == "let_declaration" and node.child_by_field_name("alternative"):
        count += 1
    return count + sum(decision_count(child, rust) for child in node.named_children)
