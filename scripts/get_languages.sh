#!/bin/bash

RESPONSE=$(curl -s "https://api.github.com/repos/zed-industries/zed/git/trees/main?recursive=1" -H "Authorization: Bearer ${GITHUB_TOKEN}")
FILES=$(echo "$RESPONSE" | jq -c -r '.tree[] | .path')

LANGUAGES="languages = [\n"
LANGUAGE_IDS="language_ids = {"

for file in $FILES; do
    if ! [[ $file == crates/languages/src/**/config.toml || $file == extensions/**/languages/**/config.toml ]]; then
        continue
    fi

    if [[ $file == extensions/test-extension/* ]]; then
        continue
    fi

    name=$(curl -s "https://raw.githubusercontent.com/zed-industries/zed/main/${file}" | head -n 1 | sed 's/^name = "\(.*\)"$/\1/')
    id="${file%/config.toml}"
    id="${id##*/}"

    echo "Adding language $name with id $id"

    LANGUAGES+="    \"$name\",\n"
    LANGUAGE_IDS+=" \"$name\" = \"$id\","
done

LANGUAGES+="]"
LANGUAGE_IDS+="}"

echo -e $LANGUAGES
echo -e $LANGUAGE_IDS
