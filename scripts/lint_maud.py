#!/usr/bin/env python3
"""Guard against the maud trap that broke two builds.

maud expands a shorthand class (`.nav-item`) into `class="nav-item"`, so an
element that also carries an explicit `class=` ends up with a duplicate
attribute and the crate will not compile. This walks every `.rs` file and
flags any tag whose shorthand classes are followed by `class=` before the
opening brace — across line breaks.

Exit code 1 on findings, so CI fails before a build does.
"""
import pathlib
import re
import sys

TAG = re.compile(r'(?<![\w.])([a-z][a-z0-9]*)\.([a-z][\w-]*(?:\.[a-z][\w-]*)*)')

def statements(src: str):
    """Yield (offset, chunk) for each tag that has shorthand classes."""
    for match in TAG.finditer(src):
        i = match.end()
        depth = 0
        while i < len(src):
            ch = src[i]
            if ch in '([{':
                if ch == '{' and depth == 0:
                    break
                depth += 1
            elif ch in ')]}':
                depth -= 1
                if depth < 0:
                    break
            i += 1
        chunk = src[match.end():i]
        # only look at the attribute region (stop at the previous statement end)
        yield match.start(), src[max(0, match.start() - 40):match.start()], chunk


def option_renders(src: str, path):
    """maud 0.26 has no Render impl for Option<T>: `(maybe)` will not compile.

    Heuristic: a binding initialised with `.map(`, `Option::`, or typed
    `Option<...>` that is later rendered as a bare `(name)` in a maud block.
    """
    findings = []
    # functions in this file that hand back an Option
    option_fns = set(
        re.findall(r'fn\s+([a-z_][a-z0-9_]*)\s*\([^)]*\)\s*->\s*(?:Result<)?Option<', src)
    )
    for match in re.finditer(r'let\s+(?:mut\s+)?([a-z_][a-z0-9_]*)\s*(?::\s*Option<[^>]*>)?\s*=\s*([^;]+);', src):
        name, init = match.group(1), match.group(2)
        calls_option_fn = any(re.search(r'\b' + fn + r'\s*\(', init) for fn in option_fns)
        if not (
            '.map(' in init
            or 'Option<' in match.group(0)
            or init.strip().startswith('Some(')
            or calls_option_fn
        ):
            continue
        if re.search(r'^\s*\(' + name + r'\)\s*$', src, re.M):
            line = src.count('\n', 0, match.start()) + 1
            findings.append(f"{path}:{line}: `{name}` looks like Option<_> but is rendered as ({name})")
    return findings


def main() -> int:
    problems = []
    for path in sorted(pathlib.Path('.').rglob('*.rs')):
        if 'target' in path.parts:
            continue
        src = path.read_text(encoding='utf-8')
        problems.extend(option_renders(src, path))
        for offset, before, chunk in statements(src):
            if re.search(r'\bclass\s*=', chunk):
                line = src.count('\n', 0, offset) + 1
                tag = src[offset:offset + 40].split('{')[0].strip()
                problems.append(f"{path}:{line}: shorthand class + class= -> {tag}")

    for problem in problems:
        print(problem)
    if problems:
        print(f"\n{len(problems)} maud problem(s) — the crate will not compile.")
        return 1
    print("maud class check: clean")
    return 0


if __name__ == '__main__':
    sys.exit(main())
