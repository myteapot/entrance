#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
custom_linux_dir="$script_dir/linux"
target="${1:-rpm}"

case "$target" in
  rpm|dir) ;;
  *)
    echo "unsupported electron-builder target: $target" >&2
    exit 2
    ;;
esac

ensure_libcrypt_compat() {
  if command -v ldconfig >/dev/null 2>&1 && ldconfig -p 2>/dev/null | grep -q "libcrypt.so.1"; then
    return 0
  fi

  local cache_root="${HOME}/.cache/entrance-electron/libxcrypt-compat"
  local rpm_file="$cache_root/libxcrypt-compat.rpm"
  local lib_dir="$cache_root/usr/lib64"
  local rpm_url

  if [[ ! -f "$lib_dir/libcrypt.so.1" ]]; then
    mkdir -p "$cache_root"
    rpm_url="$(dnf -q repoquery --location libxcrypt-compat.x86_64 | tail -n 1)"
    curl -fsSL "$rpm_url" -o "$rpm_file"

    if command -v bsdtar >/dev/null 2>&1; then
      (
        cd "$cache_root"
        bsdtar -xf "$rpm_file"
      )
    elif command -v rpm2cpio >/dev/null 2>&1 && command -v cpio >/dev/null 2>&1; then
      (
        cd "$cache_root"
        rpm2cpio "$rpm_file" | cpio -idmv >/dev/null 2>&1
      )
    else
      echo "missing extractor: need bsdtar or rpm2cpio+cpio for libxcrypt-compat bootstrap" >&2
      exit 2
    fi
  fi

  if [[ ! -f "$lib_dir/libcrypt.so.1" ]]; then
    echo "libcrypt.so.1 bootstrap failed at $lib_dir" >&2
    exit 2
  fi

  export LD_LIBRARY_PATH="$lib_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
}

ensure_rpmbuild() {
  if command -v rpmbuild >/dev/null 2>&1; then
    return 0
  fi

  local cache_root="${HOME}/.cache/entrance-electron/rpm-build"
  local rpm_file="$cache_root/rpm-build.rpm"
  local bin_path="$cache_root/usr/bin"
  local rpm_url

  mkdir -p "$cache_root"
  if [[ ! -x "$bin_path/rpmbuild" ]]; then
    rpm_url="$(dnf -q repoquery --location rpm-build.x86_64 | tail -n 1)"
    curl -fsSL "$rpm_url" -o "$rpm_file"
    (
      cd "$cache_root"
      rpm2cpio "$rpm_file" | cpio -idmv >/dev/null 2>&1
    )
  fi

  if [[ ! -x "$bin_path/rpmbuild" ]]; then
    echo "rpmbuild bootstrap failed at $bin_path" >&2
    exit 2
  fi

  export PATH="$bin_path:$PATH"
}

prepare_release_stage() {
  local stage_dir="$1"
  local output_dir="$2"
  local app_version electron_version homepage renderer_dist_dir

  app_version="$(node -e 'console.log(require(process.argv[1]).version)' "$repo_root/package.json")"
  electron_version="$(node -e 'console.log(require(process.argv[1]).version)' "$repo_root/node_modules/electron/package.json")"
  homepage="$(node -e 'console.log(require(process.argv[1]).homepage ?? "")' "$repo_root/package.json")"
  renderer_dist_dir="$repo_root/shell/gui/dist"

  mkdir -p "$stage_dir/dist" "$stage_dir/electron" "$stage_dir/icons" "$stage_dir/scripts/linux"
  cp -a "$renderer_dist_dir/." "$stage_dir/dist/"
  cp -a "$repo_root/shell/gui/electron/." "$stage_dir/electron/"
  cp -a "$repo_root/shell/gui/icons/." "$stage_dir/icons/"
  cp -a "$repo_root/target/release/entrance" "$stage_dir/entrance"
  cp -a "$custom_linux_dir/after-remove.tpl" "$stage_dir/scripts/linux/after-remove.tpl"

  cat >"$stage_dir/package.json" <<EOF
{
  "name": "entrance",
  "version": "$app_version",
  "productName": "Entrance",
  "description": "Entrance Electron release shell",
  "homepage": "$homepage",
  "main": "electron/main.mjs",
  "author": {
    "name": "Entrance Maintainers",
    "email": "maintainers@entrance.local"
  },
  "license": "SEE LICENSE IN LICENSE"
}
EOF

  cat >"$stage_dir/electron-builder.json" <<EOF
{
  "appId": "com.entrance.desktop",
  "productName": "Entrance",
  "artifactName": "Entrance-\${version}-\${os}-\${arch}.\${ext}",
  "directories": {
    "output": "$output_dir"
  },
  "files": [
    "dist/**/*",
    "electron/**/*",
    "package.json"
  ],
  "extraResources": [
    {
      "from": "entrance",
      "to": "entrance"
    }
  ],
  "linux": {
    "target": [
      {
        "target": "$target",
        "arch": [
          "x64"
        ]
      }
    ],
    "category": "Development",
    "maintainer": "Entrance Maintainers <maintainers@entrance.local>",
    "icon": "icons"
  },
  "rpm": {
    "afterRemove": "scripts/linux/after-remove.tpl"
  },
  "electronVersion": "$electron_version",
  "npmRebuild": false
}
EOF
}

cd "$repo_root"

pnpm build:electron:prereq
ensure_libcrypt_compat
ensure_rpmbuild

stage_dir="$(mktemp -d "${TMPDIR:-/tmp}/entrance-electron-release.XXXXXX")"
trap 'rm -rf "$stage_dir"' EXIT

prepare_release_stage "$stage_dir" "$repo_root/dist-electron"

npm_config_user_agent=traversal \
npm_execpath=traversal \
"$repo_root/node_modules/.bin/electron-builder" \
  --projectDir "$stage_dir" \
  --config "$stage_dir/electron-builder.json" \
  --publish never
