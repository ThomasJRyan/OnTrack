#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
mkdir -p "$work/mock" "$work/install"

cat > "$work/mock/uname" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  -s) printf '%s\n' "${FAKE_OS:-Linux}" ;;
  -m) printf '%s\n' "${FAKE_ARCH:-x86_64}" ;;
esac
EOF
cat > "$work/mock/curl" <<'EOF'
#!/usr/bin/env bash
[[ "${FAKE_CURL_FAIL:-}" != yes ]] || exit 22
while (($#)); do
  if [[ "$1" == --output ]]; then output=$2; shift 2; else url=$1; shift; fi
done
case "$url" in
  https://api.github.com/*) cp "$FAKE_ROOT/release.json" "$output" ;;
  */ontrack-*.tar.gz) cp "$FAKE_ROOT/asset" "$output" ;;
  */ontrack-*.tar.gz.sha256) cp "$FAKE_ROOT/checksum" "$output" ;;
  *) exit 22 ;;
esac
EOF
cat > "$work/mock/bd" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
cp "$work/mock/bd" "$work/mock/herdr"
chmod +x "$work/mock/"*

export FAKE_ROOT="$work" PATH="$work/mock:$PATH"
export FAKE_OS=Linux FAKE_ARCH=x86_64
python3 - "$work" <<'PY'
import hashlib
import io
import json
import pathlib
import sys
import tarfile

root = pathlib.Path(sys.argv[1])
name = "ontrack-v0.1.0-x86_64-unknown-linux-gnu.tar.gz"
with tarfile.open(root / "asset", "w:gz") as archive:
    data = b"#!/bin/sh\nprintf 'fixture ontrack\\n'\n"
    member = tarfile.TarInfo("ontrack")
    member.size = len(data)
    archive.addfile(member, io.BytesIO(data))
digest = hashlib.sha256((root / "asset").read_bytes()).hexdigest()
(root / "checksum").write_text(f"{digest}  {name}\n")
base = "https://github.com/ThomasJRyan/OnTrack/releases/download/v0.1.0/"
release = {"tag_name": "v0.1.0", "draft": False, "prerelease": False,
           "assets": [{"name": name, "browser_download_url": base + name,
                       "digest": "sha256:" + digest},
                      {"name": name + ".sha256", "browser_download_url": base + name + ".sha256"}]}
(root / "release.json").write_text(json.dumps(release))
PY

expect_failure() {
  local expected=$1
  shift
  if "$@" >"$work/stdout" 2>"$work/stderr"; then
    printf 'expected failure containing: %s\n' "$expected" >&2
    exit 1
  fi
  if ! grep -q "$expected" "$work/stderr"; then
    cat "$work/stderr" >&2
    exit 1
  fi
}

installer="$root/scripts/install.sh"
bash "$installer" --install-dir "$work/install" >"$work/stdout" 2>"$work/stderr"
[[ "$("$work/install/ontrack")" == 'fixture ontrack' ]]
expect_failure 'pass --replace' bash "$installer" --install-dir "$work/install"
bash "$installer" --install-dir "$work/install" --replace >"$work/stdout" 2>"$work/stderr"
[[ "$("$work/install/ontrack")" == 'fixture ontrack' ]]

cp "$work/checksum" "$work/checksum.good"
printf '%064d  %s\n' 0 'ontrack-v0.1.0-x86_64-unknown-linux-gnu.tar.gz' > "$work/checksum"
expect_failure 'SHA-256 mismatch' bash "$installer" --install-dir "$work/install" --replace
[[ "$("$work/install/ontrack")" == 'fixture ontrack' ]]
mv "$work/checksum.good" "$work/checksum"

# The published sidecar remains sufficient when GitHub omits its API digest.
cp "$work/release.json" "$work/release.good"
python3 - "$work/release.json" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
data = json.loads(path.read_text())
del data["assets"][0]["digest"]
path.write_text(json.dumps(data))
PY
bash "$installer" --install-dir "$work/install" --replace >"$work/stdout" 2>"$work/stderr"
mv "$work/release.good" "$work/release.json"

cp "$work/release.json" "$work/release.good"
python3 - "$work/release.json" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
data = json.loads(path.read_text())
data["assets"] = data["assets"][1:]
path.write_text(json.dumps(data))
PY
expect_failure 'no compatible asset' bash "$installer" --install-dir "$work/install" --replace
mv "$work/release.good" "$work/release.json"

mkdir -p "$work/linked"
ln -s "$work/install/ontrack" "$work/linked/ontrack"
expect_failure 'refusing to replace symlink' bash "$installer" --install-dir "$work/linked" --replace

expect_failure 'could not download release metadata' env FAKE_CURL_FAIL=yes bash "$installer" --install-dir "$work/install" --replace
expect_failure 'unsupported operating system' env FAKE_OS=FreeBSD bash "$installer"
expect_failure 'unsupported CPU architecture' env FAKE_ARCH=riscv64 bash "$installer"
expect_failure 'cannot create install directory' bash "$installer" --install-dir /dev/null/ontrack-installer-test
mkdir -p "$work/no-deps"
for executable in bash uname getconf curl python3; do
  ln -s "$(command -v "$executable")" "$work/no-deps/$executable"
done
expect_failure 'bd is missing' env PATH="$work/no-deps" "$(command -v bash)" "$installer" --install-dir "$work/install" --replace

printf 'installer tests passed\n'
