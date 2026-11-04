#!/usr/bin/env bash
# Resolves `cargo audit` findings in the working tree. `cargo audit fix` bumps
# every crate a semver-compatible update can patch, and the advisories left
# over go on the ignore list in .cargo/audit.toml. Writes the pull request
# title to $GITHUB_OUTPUT and the body, with the reasoning for each ignore, to
# the file named by $1.
#
# Leaves the tree untouched when every advisory is already mentioned by an
# open pull request from a branch other than $BRANCH.
set -euo pipefail
shopt -s inherit_errexit
export LC_ALL=C

body=$1
config=.cargo/audit.toml
tmp=$(mktemp -d)
: >"$body"

# Prints the JSON report. Fails when the audit cannot run, not on findings.
audit() {
  local report
  report=$(cargo audit --json) || true
  jq -e .vulnerabilities <<<"$report" >/dev/null
  echo "$report"
}

lock_versions() {
  awk -F'"' '/^name = /{name=$2} /^version = /{print name, $2}' Cargo.lock | sort
}

before=$(audit)
ids=$(jq -r '.vulnerabilities.list[].advisory.id' <<<"$before" | sort -u)
[[ -n $ids ]] || exit 0

uncovered=0
for id in $ids; do
  prs=$(gh pr list --state open --search "\"$id\" in:title,body" --json headRefName \
    --jq "map(select(.headRefName != \"$BRANCH\")) | length")
  if ((prs > 0)); then
    echo "$id: an open pull request already mentions it"
  else
    uncovered=1
  fi
done
((uncovered)) || exit 0

lock_versions >"$tmp/before"
cargo audit fix
lock_versions >"$tmp/after"
after=$(audit)

echo "title=Fix cargo audit: $(paste -sd, <<<"$ids" | sed 's/,/, /g')" >>"${GITHUB_OUTPUT:-/dev/null}"

{
  echo "# Description"
  echo "The cargo-audit-fix workflow found advisories on main."

  bumps=$(join <(comm -23 "$tmp/before" "$tmp/after") <(comm -13 "$tmp/before" "$tmp/after"))
  if [[ -n $bumps ]]; then
    echo
    echo "## Bumped by \`cargo audit fix\`"
    for id in $(comm -23 <(echo "$ids") <(jq -r '.vulnerabilities.list[].advisory.id' <<<"$after" | sort -u)); do
      echo "Fixes [$id](https://rustsec.org/advisories/$id)."
    done
    echo
    while read -r name old new; do
      echo "- \`$name\` $old to $new"
    done <<<"$bumps"
  fi

  if jq -e '.vulnerabilities.list != []' <<<"$after" >/dev/null; then
    cargo metadata --format-version 1 --all-features >"$tmp/metadata.json"
    echo
    echo "## Ignored"
    echo "No semver-compatible update reaches a patched version for these, so they go on the ignore list in" \
      "\`.cargo/audit.toml\`. An ignore is a security decision: check each one before merging, and drop it" \
      "once its dependents allow a patched version."

    while read -r group; do
      id=$(jq -r '.[0].advisory.id' <<<"$group")
      name=$(jq -r '.[0].package.name' <<<"$group")
      title=$(jq -r '.[0].advisory.title' <<<"$group")
      patched=$(jq -r '.[0].versions.patched | if . == [] then "none" else join(", ") end' <<<"$group")

      echo
      echo "### [$id](https://rustsec.org/advisories/$id): $name"
      echo "$title"
      echo
      echo "- Patched: $patched"
      for version in $(jq -r '.[].package.version' <<<"$group"); do
        dependents=$(jq -r --arg name "$name" --arg version "$version" '
          . as $m
          | ($m.packages[] | select(.name == $name and .version == $version) | .id) as $id
          | $m.resolve.nodes[] | select(any(.deps[]; .pkg == $id)) | .id as $parent
          | $m.packages[] | select(.id == $parent)
          | "- \(.name) \(.version) requires `\($name) \([.dependencies[] | select(.name == $name) | .req] | unique | join(", "))`"
        ' "$tmp/metadata.json")
        echo "${dependents:-- Nothing in the resolved graph depends on $name $version, it is likely an unused optional dependency}"
        echo
        echo "<details><summary>Dependency path of $name $version</summary>"
        echo
        echo '```'
        cargo tree --workspace --all-features -e normal,build,dev -i "$name@$version" --depth 3 2>&1 |
          sed "s| ($PWD/[^)]*)||" || true
        echo '```'
        echo
        echo "</details>"
      done

      ENTRY=$(printf '\n  # %s - %s (%s).\n  # No semver-compatible fix (patched: %s).\n  "%s",' \
        "$name" "$title" "$id" "$patched" "$id") \
        awk '/^]/ && !done { print ENVIRON["ENTRY"]; done = 1 } { print }' "$config" >"$tmp/audit.toml"
      mv "$tmp/audit.toml" "$config"
    done < <(jq -c '.vulnerabilities.list | group_by(.advisory.id)[]' <<<"$after")
  fi

  echo
  echo "# How to test"
  echo "\`cargo audit\` passes on this branch."
} >"$body"

jq -e '.vulnerabilities.list == []' <<<"$(audit)" >/dev/null || {
  echo "advisories remain after the fix and the ignores" >&2
  exit 1
}
