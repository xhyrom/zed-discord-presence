#!/bin/bash

fetch_files() {
    local repo_owner=$1
    local repo_name=$2
    curl -Ls "https://api.github.com/repos/$repo_owner/$repo_name/git/trees/HEAD?recursive=1" -H "Authorization: Bearer ${GITHUB_TOKEN}" | jq -c -r '.tree[] | .path'
}

process_language_files() {
    local files=$1
    local repo_owner=$2
    local repo_name=$3
    local file_patterns=("${@:4}")

    for file in $files; do
        local match=false
        for pattern in "${file_patterns[@]}"; do
            if [[ $file == $pattern ]]; then
                match=true
                break
            fi
        done

        if ! $match; then
            continue
        fi

        name=$(curl -s "https://raw.githubusercontent.com/$repo_owner/$repo_name/HEAD/${file}" | grep -oP '^name = "\K[^"]*')
        id="${file%/config.toml}"
        id="${id##*/}"

        if [[ $LANGUAGES == *"$name\","* && $LANGUAGE_IDS == *"$name\" = \"$id\","* ]]; then
            echo "Skipping language $name with id $id (already added)"
        else
            echo "Adding language $name with id $id"
            LANGUAGES+="    \"$name\",\n"
            LANGUAGE_IDS+=" \"$name\" = \"$id\","
        fi
    done
}

RESPONSE=$(curl -s "https://api.github.com/repos/zed-industries/zed/git/trees/main?recursive=1" -H "Authorization: Bearer ${GITHUB_TOKEN}")
EXTENSIONS_RESPONSE=$(curl -s "https://raw.githubusercontent.com/zed-industries/extensions/main/.gitmodules")
REPOSITORIES=($(echo "$EXTENSIONS_RESPONSE" | grep -oP 'url = \K.*'))
FILES=$(echo "$RESPONSE" | jq -c -r '.tree[] | .path')

LANGUAGES="languages = [\n"
LANGUAGE_IDS="language_ids = {"

for repository in "${REPOSITORIES[@]}"; do
    repo_owner=$(echo "$repository" | cut -d'/' -f4)
    repo_name=$(echo "$repository" | cut -d'/' -f5 | sed 's/.git$//')

    echo "Checking $repository ($repo_owner/$repo_name)"
    files=$(fetch_files "$repo_owner" "$repo_name")
    process_language_files "$files" "$repo_owner" "$repo_name" "languages/**/config.toml"
done

echo "Processing zed-industries/zed"
process_language_files "$FILES" "zed-industries" "zed" "crates/languages/src/**/config.toml" "extensions/**/languages/**/config.toml"

LANGUAGES+="]"
LANGUAGE_IDS+="}"

echo -e $LANGUAGES
echo -e $LANGUAGE_IDS
