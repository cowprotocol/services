#!/usr/bin/env bash
# Selects the test groups that the changes between a base revision and HEAD
# can affect, in a Cargo workspace.
#
# Usage: select-tests.sh <base> <groups> [<ignore>]
#
#   <base>    The revision to compare with HEAD.
#   <groups>  One `<group>=<package>` line for each test group. The group
#             tests the workspace package <package>.
#   <ignore>  One path prefix on each line. No test group uses the files with
#             these prefixes.
#
# Writes a JSON object to stdout, for example {"driver":"false","e2e":"true"},
# and the reasons to stderr. A group is "true" when a changed file is in
# <package> or in a workspace package that <package> depends on. All the
# dependency kinds count, also dev-dependencies and optional dependencies.
# A changed file in no workspace package (for example `Cargo.lock`) selects
# all the groups. Run the script in the workspace.
set -euo pipefail

base=$1
groups=$2
ignore=${3-}

changed=$(git diff --no-renames --name-only "$base" HEAD)
metadata=$(cargo metadata --no-deps --offline --format-version 1)

# One `<directory> <name>` line for each workspace package. The directory is
# relative to the workspace root and ends with `/`. The longest directory is
# first, so that a package in the directory of another package matches first.
packages=$(jq -r '
  .workspace_root as $root
  | .packages
  | map({dir: (.manifest_path | ltrimstr("\($root)/") | rtrimstr("Cargo.toml")), name})
  | sort_by(-(.dir | length))[]
  | "\(.dir) \(.name)"' <<< "$metadata")

# One `<name> <dependencies>` line for each workspace package. The
# dependencies are the workspace packages that it depends on.
edges=$(jq -r '
  .packages[]
  | "\(.name) \([.dependencies[] | select(.path) | .name] | join(" "))"' <<< "$metadata")

declare -A deps
while read -r name names; do
  deps[$name]=$names
done <<< "$edges"

# Prints the package and each workspace package that it depends on, with a
# space before and after each name.
closure() {
  local -A seen=(["$1"]=1)
  local queue=("$1") package dep
  while ((${#queue[@]})); do
    package=${queue[0]}
    queue=("${queue[@]:1}")
    for dep in ${deps[$package]-}; do
      if [[ -z ${seen[$dep]-} ]]; then
        seen[$dep]=1
        queue+=("$dep")
      fi
    done
  done
  printf ' %s' "${!seen[@]}"
  printf ' '
}

declare -A closures selected
while IFS='=' read -r group package; do
  [[ -n $group ]] || continue
  if [[ -z ${deps[$package]+set} ]]; then
    echo "Package $package of group $group is not in the workspace" >&2
    exit 1
  fi
  closures[$group]=$(closure "$package")
  selected[$group]=false
done <<< "$groups"

select_all() {
  local group
  for group in "${!selected[@]}"; do
    selected[$group]=true
  done
}

is_ignored() {
  local prefix
  while read -r prefix; do
    if [[ -n $prefix && $1 == "$prefix"* ]]; then
      return 0
    fi
  done <<< "$ignore"
  return 1
}

# Prints the name of the workspace package that contains the path, or nothing.
package_of() {
  local dir name
  while read -r dir name; do
    if [[ $1 == "$dir"* ]]; then
      echo "$name"
      return
    fi
  done <<< "$packages"
}

if [[ -z $changed ]]; then
  echo "No changed files: select all groups" >&2
  select_all
fi
while read -r path; do
  if [[ -z $path ]] || is_ignored "$path"; then
    continue
  fi
  package=$(package_of "$path")
  if [[ -z $package ]]; then
    echo "$path: in no workspace package, select all groups" >&2
    select_all
    break
  fi
  for group in "${!closures[@]}"; do
    if [[ ${selected[$group]} == false && ${closures[$group]} == *" $package "* ]]; then
      echo "$path: package $package selects group $group" >&2
      selected[$group]=true
    fi
  done
done <<< "$changed"

args=()
for group in "${!selected[@]}"; do
  args+=(--arg "$group" "${selected[$group]}")
done
jq -cnS '$ARGS.named' "${args[@]}"
