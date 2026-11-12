"""Selects the tests that the changes between BASE and HEAD can affect.

The rules, the inputs and the outputs are described in action.yml. Reads the
inputs from the BASE, GROUP_LINES, PATH_LINES and IGNORE_LINES environment
variables and appends the outputs to $GITHUB_OUTPUT.
"""

import json
import os
import re
import subprocess
import sys
import tomllib


def run(*args):
    return subprocess.run(args, check=True, stdout=subprocess.PIPE, text=True).stdout


def lines(text):
    """The lines of a text that are not empty."""
    return [line for line in text.split("\n") if line != ""]


def package_lines(text, names):
    """Reads `<key>=<packages>` lines into a list of (key, packages)."""
    result = []
    for line in lines(text):
        match = re.fullmatch(r"([^=\s]+)=([^=]*)", line)
        if not match:
            sys.exit(f"Not a <key>=<packages> line: {line}")
        key, words = match.groups()
        packages = [name for word in words.split() for name in (names if word == "*" else [word])]
        if not packages:
            sys.exit(f"{key}: no packages")
        unknown = [name for name in packages if name not in names]
        if unknown:
            sys.exit(f"{key}: not workspace packages: {' '.join(unknown)}")
        result.append((key, packages))
    return result


def unique_keys(items):
    """Stops when a key is on more than one line."""
    keys = [key for key, _ in items]
    repeated = sorted({key for key in keys if keys.count(key) > 1})
    if repeated:
        sys.exit(f"On more than one line: {' '.join(repeated)}")
    return items


def with_dependents(names, dependents):
    """The names, the names that depend on them, the names that depend on
    those, and so on."""
    result = set(names)
    todo = list(result)
    while todo:
        for dependent in dependents.get(todo.pop(), ()):
            if dependent not in result:
                result.add(dependent)
                todo.append(dependent)
    return result


def longest_match(items, file):
    """The (prefix, value) item whose prefix is the longest prefix of the file, or None."""
    matches = [item for item in items if file.startswith(item[0])]
    return max(matches, key=lambda item: len(item[0]), default=None)


def lock_packages(base, workspace):
    """The workspace packages whose Cargo.lock entry changed or that depend,
    directly or not, on a crate whose entry changed. The dependencies of both
    lockfiles count, so that a removed dependency counts too."""
    old = tomllib.loads(run("git", "show", f"{base}:Cargo.lock"))["package"]
    with open("Cargo.lock", "rb") as f:
        new = tomllib.load(f)["package"]
    dependents = {}
    for entry in old + new:
        # `name`, `name version` or `name version (source)`.
        for dependency in entry.get("dependencies", []):
            dependents.setdefault(dependency.split(" ")[0], set()).add(entry["name"])
    old_entries = {json.dumps(entry, sort_keys=True) for entry in old}
    new_entries = {json.dumps(entry, sort_keys=True) for entry in new}
    changed = {json.loads(entry)["name"] for entry in old_entries ^ new_entries}
    return sorted(with_dependents(changed, dependents) & workspace)


def main():
    base = os.environ["BASE"]

    # A moved file gives its old path and its new path.
    files = run("git", "diff", "--no-renames", "--name-only", base, "HEAD").splitlines()
    print(f"{len(files)} changed files")

    metadata = json.loads(
        run("cargo", "metadata", "--no-deps", "--offline", "--format-version", "1")
    )
    root = metadata["workspace_root"] + "/"
    # The directory of each workspace package, relative to the workspace root.
    # Its `/` at the end keeps `crates/driver/` from matching
    # `crates/driver-old/`.
    directories = {
        package["name"]: package["manifest_path"].removeprefix(root).removesuffix("Cargo.toml")
        for package in metadata["packages"]
    }
    # A dependency with a `path` is a workspace package.
    dependents = {}
    for package in metadata["packages"]:
        for dependency in package["dependencies"]:
            if dependency.get("path"):
                dependents.setdefault(dependency["name"], set()).add(package["name"])
    names = list(directories)
    print(f"{len(names)} workspace packages")

    groups = dict(unique_keys(package_lines(os.environ["GROUP_LINES"], names)))
    # An ignored prefix is a path rule with no packages.
    ignored = []
    for line in lines(os.environ["IGNORE_LINES"]):
        if not re.fullmatch(r"\S+", line):
            sys.exit(f"Not a prefix line: {line}")
        ignored.append((line, []))
    rules = unique_keys(package_lines(os.environ["PATH_LINES"], names) + ignored)
    print(f"{len(groups)} groups, {len(rules)} path rules")

    # The packages that a file changes also affect their dependents, the
    # packages of a path rule do not.
    changed, rule_packages = set(), set()
    package_dirs = [(directory, name) for name, directory in directories.items()]
    for file in files:
        rule = longest_match(rules, file)
        package = longest_match(package_dirs, file)
        if rule and not rule[1]:
            effect = f"ignored ({rule[0]})"
        elif rule:
            rule_packages.update(rule[1])
            effect = f"the tests of {' '.join(rule[1])} (path rule {rule[0]})"
        elif file == "Cargo.lock":
            lock = lock_packages(base, set(names))
            changed.update(lock)
            effect = (
                "the packages that use the changed entries and their dependents: "
                + (" ".join(lock) or "none")
            )
        elif package:
            changed.add(package[1])
            effect = f"{package[1]} and its dependents"
        else:
            changed.update(names)
            effect = "all the packages (no path rule and no workspace package)"
        print(f"{file}: {effect}")

    # An empty diff changes all the packages, so that a wrong base cannot skip
    # the tests.
    if not files:
        print("No changed files: all the packages")
        changed = set(names)

    affected = sorted(with_dependents(changed, dependents) | rule_packages)
    if len(affected) == len(names):
        print("All the packages")
    elif not affected:
        print("No packages")
    else:
        print("\n".join(affected))

    selected = {}
    for group, packages in groups.items():
        is_affected = any(name in affected for name in packages)
        selected[group] = "true" if is_affected else "false"
        if is_affected:
            print(f"{group}: true")
        else:
            print(f"{group}: false (not affected: {' '.join(packages)})")
    if len(affected) == len(names):
        nextest_filter = "all()"
    elif not affected:
        nextest_filter = "none()"
    else:
        nextest_filter = " | ".join(f"package(={name})" for name in affected)
    print(f"Nextest filter: {nextest_filter}")

    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        output.write(f"groups={json.dumps(selected, separators=(',', ':'))}\n")
        output.write(f"nextest-filter={nextest_filter}\n")


if __name__ == "__main__":
    main()
