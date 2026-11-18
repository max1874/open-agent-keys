#!/bin/sh
# install.sh — install or update open-agent-keys from a GitHub release: the binary and the
# agent skill, both from the same tag. Rerun it to update.
#
#   curl -fsSL https://raw.githubusercontent.com/max1874/open-agent-keys/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/max1874/open-agent-keys/main/install.sh | sh -s -- v0.3.0
#
# BIN_DIR (default ~/.local/bin) and SKILL_DIRS (default ~/.claude/skills for Claude Code and ~/.agents/skills,
# which Codex reads, whichever exist) change where it goes. A skill directory that is a link, such as one pointing
# into a clone of this repository, is left alone. Releases from v0.3.0 on.
set -eu

repo=max1874/open-agent-keys
tag=${1:-$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$repo/releases/latest")}
tag=${tag##*/}
case $tag in v*) ;; *) echo "install.sh: no release found" >&2; exit 1 ;; esac

asset="open-agent-keys-$(uname -s)-$(uname -m).tar.gz"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"
curl -fsSLO "https://github.com/$repo/releases/download/$tag/$asset"
curl -fsSLO "https://github.com/$repo/releases/download/$tag/$asset.sha256"
if command -v sha256sum >/dev/null; then sha256sum -c "$asset.sha256" >/dev/null; else shasum -a 256 -c "$asset.sha256" >/dev/null; fi
tar -xzf "$asset"

bin=${BIN_DIR:-$HOME/.local/bin}
mkdir -p "$bin"
# Copy beside the old binary, then rename over it: a running copy keeps its file.
cp open-agent-keys "$bin/.open-agent-keys.new"
mv -f "$bin/.open-agent-keys.new" "$bin/open-agent-keys"
echo "binary: $bin/open-agent-keys"

for dir in ${SKILL_DIRS:-"$HOME/.claude/skills" "$HOME/.agents/skills"}; do
  [ -d "$dir" ] || continue
  if [ -L "$dir/open-agent-keys" ]; then
    echo "skill: $dir/open-agent-keys is a link, left as is"
    continue
  fi
  mkdir -p "$dir/open-agent-keys"
  curl -fsSL -o "$dir/open-agent-keys/SKILL.md" \
    "https://raw.githubusercontent.com/$repo/$tag/skills/open-agent-keys/SKILL.md"
  echo "skill: $dir/open-agent-keys"
done

"$bin/open-agent-keys" --version
