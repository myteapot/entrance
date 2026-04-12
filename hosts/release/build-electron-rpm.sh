#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

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

cd "$repo_root"

pnpm build:electron:prereq
ensure_libcrypt_compat
ensure_rpmbuild
pnpm exec electron-builder --config hosts/release/electron-builder.json --linux rpm --publish never
