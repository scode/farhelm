"""Recognize delay calls and their rationale in Rust and browser test syntax.

This is a source convention check, not type checking or macro expansion. Keep
literal text separate from executable expressions and preserve test scope when
helpers move into external modules.
"""
import re
import json
import pathlib
import posixpath
import ast
from tree_sitter import Language, Parser
import tree_sitter_rust
import tree_sitter_typescript


class SyntaxBudget:
    """Share discovery and report bounds across files, before eagerly retaining results."""

    def __init__(self, *, check=lambda: None, calls=10000, report_bytes=8 * 1024 * 1024):
        self.check = check
        self.calls_left = calls
        self.report_bytes_left = report_bytes

    def candidate(self):
        """Charge a newly discovered call before adding it to the candidate map."""
        self.check()
        self.calls_left -= 1
        if self.calls_left < 0:
            raise ValueError('source check exceeded its delay-call budget')

    def retain(self, value):
        """Charge serialized field bytes before callers can retain repeated rationale values."""
        self.check()
        self.report_bytes_left -= len(json.dumps(value, ensure_ascii=True).encode('ascii'))
        if self.report_bytes_left < 0:
            raise ValueError('source check exceeded its report-byte budget')


def walk(node, check=lambda: None):
    """Visit syntax without Python recursion or interpreting literal content as source."""
    pending = [node]
    while pending:
        check()
        current = pending.pop()
        yield current
        pending.extend(reversed(current.children))


def text(node, source):
    """Recover exact token spelling, preserving byte-based parser coordinates."""
    return source[node.start_byte:node.end_byte].decode('utf-8') if node is not None else ''


def cfg_without_test(tokens, source):
    """Evaluate possible cfg truth values with test disabled and other flags unknown.

    Only expressions impossible without test establish test-only scope.
    In particular, any(test, target_os = "macos") also includes production
    code and must not be mistaken for a test-only declaration.
    """
    tokens = [node for node in tokens if node.type not in {'line_comment', 'block_comment'}]
    if len(tokens) == 1 and text(tokens[0], source) == 'test':
        return {False}
    if len(tokens) == 2 and tokens[1].type == 'token_tree':
        operator = text(tokens[0], source)
        arguments, current = [], []
        for node in tokens[1].children[1:-1]:
            if node.type == ',':
                if current:
                    arguments.append(current)
                    current = []
            else:
                current.append(node)
        if current:
            arguments.append(current)
        values = [cfg_without_test(argument, source) for argument in arguments]
        if operator == 'not' and len(values) == 1:
            return {not value for value in values[0]}
        if operator == 'all':
            return ({True} if all(True in value for value in values) else set()) | (
                {False} if any(False in value for value in values) else set())
        if operator == 'any':
            return ({True} if any(True in value for value in values) else set()) | (
                {False} if all(False in value for value in values) else set())
    return {False, True}


def test_attribute(item, source):
    """Interpret the same test predicate for outer declarations and inner module attributes."""
    for attribute in item.named_children:
        if attribute.type != 'attribute':
            continue
        spelling = ''.join(text(attribute, source).split())
        name = spelling.split('(', 1)[0].split('::')[-1]
        if name == 'test':
            return True
        if name == 'cfg':
            groups = [child for child in attribute.named_children if child.type == 'token_tree']
            if (len(groups) == 1
                    and any(child.type == 'identifier' and text(child, source) == 'test' for child in walk(groups[0]))
                    and cfg_without_test(groups[0].children[1:-1], source) == {False}):
                return True
    return False


def test_item(node, source):
    """Include test-qualified expressions/initializers and enclosing inner cfg attributes."""
    sibling = node.prev_named_sibling
    while sibling is not None and sibling.type in {'attribute_item', 'line_comment', 'block_comment'}:
        if sibling.type == 'attribute_item' and test_attribute(sibling, source):
            return True
        sibling = sibling.prev_named_sibling
    if node.type in {'source_file', 'declaration_list', 'block'}:
        for child in node.named_children:
            if child.type not in {'inner_attribute_item', 'line_comment', 'block_comment'}:
                break
            if child.type == 'inner_attribute_item' and test_attribute(child, source):
                return True
    return False


def in_test_scope(node, source):
    """Nested closures and macro tokens inherit their enclosing test-only declaration."""
    current = node
    while current is not None:
        if test_item(current, source):
            return True
        current = current.parent
    return False


def module_path_attribute(node, source):
    """Read a literal module path without evaluating attributes or following the filesystem."""
    sibling = node.prev_named_sibling
    paths = []
    while sibling is not None and sibling.type in {'attribute_item', 'line_comment', 'block_comment'}:
        if sibling.type == 'attribute_item':
            for attribute in sibling.named_children:
                spelling = text(attribute, source)
                if (spelling.lstrip().startswith('cfg_attr')
                        and any(part.type == 'identifier' and text(part, source) == 'path'
                                for part in walk(attribute))):
                    raise ValueError('conditional module path attributes are unsupported')
                match = re.fullmatch(r'path\s*=\s*(.+)', spelling, re.DOTALL)
                if match:
                    value = match.group(1)
                    raw = re.fullmatch(r'r(#{0,})"(.*)"\1', value, re.DOTALL)
                    try:
                        path = raw.group(2) if raw else json.loads(value)
                    except (ValueError, TypeError) as error:
                        raise ValueError('unsupported module path literal') from error
                    if not isinstance(path, str) or '\x00' in path or pathlib.PurePosixPath(path).is_absolute():
                        raise ValueError('module path must be a relative literal')
                    paths.append(path)
        sibling = sibling.prev_named_sibling
    if len(paths) > 1:
        raise ValueError('multiple module path attributes')
    return paths[0] if paths else None


def external_modules(source, filename, *, crate_root=False, budget=None):
    """Return source-declared module candidates and whether their declaration is test-only.

    Paths follow the Rust Reference's outlined-module rules. Resolution is
    against the caller's bounded source inventory, never an unchecked path
    opened because source happened to name it.
    """
    budget = budget or SyntaxBudget()
    budget.check()
    tree = Parser(Language(tree_sitter_rust.language())).parse(source)
    budget.check()
    if tree.root_node.has_error:
        raise ValueError('source has syntax errors under the pinned grammar')
    path = pathlib.PurePosixPath(filename)
    base = path.parent if crate_root or path.name == 'mod.rs' else path.with_suffix('')
    result = []
    for node in walk(tree.root_node, budget.check):
        if node.type != 'mod_item' or node.child_by_field_name('body') is not None:
            continue
        name = symbol_name(node.child_by_field_name('name'), source)
        ancestors, current = [], node.parent
        while current is not None:
            if current.type == 'mod_item':
                ancestors.append(current)
            current = current.parent
        directory = base
        for index, ancestor in enumerate(reversed(ancestors)):
            explicit = module_path_attribute(ancestor, source)
            if explicit is not None:
                directory = (path.parent if index == 0 else directory) / explicit
            else:
                directory /= symbol_name(ancestor.child_by_field_name('name'), source)
        explicit = module_path_attribute(node, source)
        if explicit is not None:
            candidates = [(directory if ancestors else path.parent) / explicit]
        else:
            candidates = [directory / (name + '.rs'), directory / name / 'mod.rs']
        result.append(([posixpath.normpath(str(candidate)) for candidate in candidates], in_test_scope(node, source)))
    return result


def annotation_index(nodes, source, budget):
    """Decode each eligible comment once and index it by row for constant-size call lookup."""
    result = {}
    for comment in nodes:
        budget.check()
        if comment.type not in {'line_comment', 'comment'}:
            continue
        spelling = text(comment, source).strip()
        match = re.fullmatch(r'//\s*sleep-ok:\s*(\S.*?)\s*', spelling)
        if match is None:
            continue
        value = match.group(1)
        budget.retain(value)
        line_start = source.rfind(b'\n', 0, comment.start_byte) + 1
        standalone = not source[line_start:comment.start_byte].strip()
        result.setdefault(comment.start_point.row, []).append((comment.start_byte, standalone, value))
    return result


def rationale(start, end, comments, source):
    """Associate an explanatory line comment only with an adjacent call.

    A preceding annotation must occupy its own line immediately before the
    call. A trailing annotation must follow the completed call on that line.
    Strings, block comments and documentation comments cannot authorize a
    delay. One rationale may explain multiple calls on the same source line.
    """
    for _, standalone, value in comments.get(start.start_point.row - 1, ()):
        if standalone:
            return value
    for offset, _, value in comments.get(end.end_point.row, ()):
        if offset >= end.end_byte:
            return value
    return None


def symbol_name(node, source):
    """Normalize identifier spelling and literal property/import names without evaluating expressions."""
    if node is None:
        return ''
    if node.type == 'scoped_identifier':
        return symbol_name(node.child_by_field_name('name'), source)
    spelling = text(node, source)
    if node.type == 'string':
        try:
            value = ast.literal_eval(spelling)
        except (ValueError, SyntaxError):
            return ''
        return value if isinstance(value, str) else ''
    return spelling.removeprefix('r#')


def unwrap_expression(node):
    """Remove syntax-only wrappers without interpreting a type or changing the underlying expression."""
    while node is not None and node.type in {
        'parenthesized_expression', 'non_null_expression', 'as_expression', 'satisfies_expression', 'type_assertion',
    }:
        children = [child for child in node.named_children if child.type not in {'comment', 'line_comment', 'block_comment'}]
        if node.type in {'as_expression', 'satisfies_expression'}:
            node = children[0] if children else None
        elif node.type == 'type_assertion':
            node = children[-1] if children else None
        else:
            node = children[0] if len(children) == 1 else None
    return node


def callee_name(node, source):
    """Remove transparent call wrappers while keeping unrelated string literals out of candidate discovery."""
    node = unwrap_expression(node)
    if node is None:
        return ''
    if node.type in {'member_expression', 'subscript_expression'}:
        receiver = unwrap_expression(node.child_by_field_name('object'))
        property_node = node.child_by_field_name('property' if node.type == 'member_expression' else 'index')
        if symbol_name(receiver, source) in {'test', 'testInfo'} and symbol_name(property_node, source) == 'setTimeout':
            # Playwright's per-test deadline configuration does not perform a
            # delay. It must not be migrated or annotated as a sleep.
            return ''
    if node.type in {'member_expression', 'field_expression'}:
        node = node.child_by_field_name('property' if node.type == 'member_expression' else 'field')
    elif node.type == 'subscript_expression':
        node = node.child_by_field_name('index')
        if node is None or node.type != 'string':
            return ''
    return symbol_name(node, source)


def import_aliases(nodes, names, source, budget):
    """Recognize explicit aliases, including source-written use statements inside Rust macro bodies."""
    edges = {}

    def rename(original, alias):
        """Retain syntax edges before following them, so declaration order cannot hide a chain."""
        if original and alias:
            edges.setdefault(original, set()).add(alias)

    for node in nodes:
        budget.check()
        if node.type == 'import_specifier':
            original = symbol_name(node.child_by_field_name('name'), source)
            alias = symbol_name(node.child_by_field_name('alias'), source)
            rename(original, alias)
        elif node.type == 'use_as_clause':
            identifiers = [child for child in node.named_children if child.type in {'identifier', 'scoped_identifier'}]
            if len(identifiers) == 2:
                rename(symbol_name(identifiers[0], source), symbol_name(identifiers[1], source))
        elif node.type == 'use' and node.parent is not None and node.parent.type == 'token_tree':
            # Macro bodies retain tokens rather than use_declaration nodes.
            # Inspect only the source-written use statement up to its semicolon,
            # so casts elsewhere in a branch do not become imported aliases.
            leaves = []
            sibling = node.next_sibling
            while sibling is not None and sibling.type != ';':
                for part in walk(sibling, budget.check):
                    if part.type in {'identifier', 'as'}:
                        leaves.append(part)
                sibling = sibling.next_sibling
            for index in range(1, len(leaves) - 1):
                if leaves[index].type == 'as':
                    rename(symbol_name(leaves[index - 1], source), symbol_name(leaves[index + 1], source))
    pending = list(names)
    while pending:
        budget.check()
        for alias in edges.get(pending.pop(), ()):
            budget.check()
            if alias not in names:
                names.add(alias)
                pending.append(alias)


def macro_callee(node, source):
    """Recognize a source-written identifier path inside transparent macro token parentheses.

    Macro tokens have no expression tree. Accept only identifier paths and
    nested parentheses here; operators, literals and arbitrary macro output
    cannot be inferred to denote a callable delay.
    """
    while node.type == 'token_tree' and text(node, source).startswith('('):
        children = [child for child in node.children[1:-1]
                    if child.type not in {'line_comment', 'block_comment'}]
        if len(children) == 1:
            node = children[0]
            continue
        if children and all(child.type == ('identifier' if index % 2 == 0 else '::')
                            for index, child in enumerate(children)) and len(children) % 2 == 1:
            return symbol_name(children[-1], source)
        return ''
    return symbol_name(node, source) if node.type == 'identifier' else ''


def sleep_details(source, language, *, whole_file=True, budget=None):
    """Return syntax candidates, including Rust macro tokens and TS interpolated expressions.

    This is name-based recognition, not type or arbitrary alias analysis. Test
    declaration scope and annotation ownership are checked from syntax too.
    """
    rust = language == 'rust'
    budget = budget or SyntaxBudget()
    budget.check()
    grammar = tree_sitter_rust.language() if rust else tree_sitter_typescript.language_typescript()
    tree = Parser(Language(grammar)).parse(source)
    budget.check()
    if tree.root_node.has_error:
        raise ValueError('source has syntax errors under the pinned grammar')
    nodes = list(walk(tree.root_node, budget.check))
    names = {'sleep', 'sleep_until'} if rust else {'setTimeout', 'setInterval', 'waitForTimeout'}
    # Explicit import renames are visible syntax. Track them so renaming sleep
    # to pause cannot bypass the check; dynamic aliases need author review.
    import_aliases(nodes, names, source, budget)
    found = {}
    comments = annotation_index(nodes, source, budget)
    for node in nodes:
        budget.check()
        if node.type not in {'call_expression', 'token_tree'}:
            continue
        if node.type == 'call_expression':
            function = node.child_by_field_name('function')
            if function is None:
                continue
            if callee_name(function, source) in names and (not rust or whole_file or in_test_scope(node, source)):
                budget.candidate()
                found[function.start_byte] = (function, node)
        elif rust and node.type == 'token_tree':
            children = [child for child in node.children if child.type not in {'line_comment', 'block_comment'}]
            for index, child in enumerate(children[:-1]):
                following = children[index + 1]
                if (macro_callee(child, source) in names
                        and (index == 0 or children[index - 1].type != 'fn')
                        and following.type == 'token_tree' and text(following, source).startswith('(')
                        and (whole_file or in_test_scope(node, source))):
                    budget.candidate()
                    found[child.start_byte] = (child, following)
    result = []
    for start, end in (found[key] for key in sorted(found)):
        budget.check()
        item = {'line': start.start_point.row + 1, 'call': text(start, source),
                'rationale': rationale(start, end, comments, source)}
        budget.retain(item)
        result.append(item)
    return result


def sleep_calls(source, language, **options):
    """Return just call locations for consumers that do not need annotation details."""
    return [(item['line'], item['call']) for item in sleep_details(source, language, **options)]
