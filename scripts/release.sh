#!/usr/bin/env bash
# Sortir une nouvelle version d'OneClick depuis ta machine.
#
#   ./scripts/release.sh            # construit, empaquette, tague (GitHub publie)
#   ./scripts/release.sh --dry-run  # construit et empaquette seulement
#
# Il faut : Rust installé (https://rustup.rs) et GitHub CLI connecté (gh auth login).
set -euo pipefail

cd "$(dirname "$0")/.."

DRY=0
[ "${1:-}" = "--dry-run" ] && DRY=1

VERSION=$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')
TAG="v$VERSION"
ZIP="dist/OneClick-$VERSION-windows-x64.zip"

echo "OneClick $VERSION  ->  tag $TAG"

if [ "$DRY" = 0 ]; then
  if [ -n "$(git status --porcelain)" ]; then
    echo "Il reste des fichiers non commités. Commit d'abord (et bump la version)." >&2
    exit 1
  fi
  if git rev-parse "$TAG" >/dev/null 2>&1; then
    echo "Le tag $TAG existe déjà." >&2
    exit 1
  fi
fi

echo "→ tests"
cargo test --all

echo "→ build release"
cargo build --release --bin store

EXE="target/release/store"
[ -f target/release/store.exe ] && EXE="target/release/store.exe"

STAGE="dist/OneClick-$VERSION-windows-x64"
rm -rf "$STAGE" "$ZIP"
mkdir -p "$STAGE"
cp "$EXE" "$STAGE/store.exe"
cp README.md "$STAGE/LISEZMOI.md"
cat > "$STAGE/OneClick.bat" <<'BAT'
@echo off
title OneClick
cd /d "%~dp0"
start "" store.exe
ping -n 3 127.0.0.1 >nul
start "" http://localhost:8642
exit
BAT
cat > "$STAGE/A_LIRE.txt" <<'TXT'
OneClick

1. Dezipper ce dossier ou vous voulez (ex: C:\OneClick).
2. Double-cliquer sur OneClick.bat.
3. Le navigateur s'ouvre sur http://localhost:8642

Vos donnees restent dans le dossier "data" cree a cote de store.exe.
Copiez ce dossier pour faire une sauvegarde.
TXT

( cd "$STAGE" && zip -qr "../OneClick-$VERSION-windows-x64.zip" . ) 2>/dev/null \
  || ( cd "$STAGE" && powershell -NoProfile -Command "Compress-Archive -Path * -DestinationPath '../OneClick-$VERSION-windows-x64.zip' -Force" )

mkdir -p dist
sha256sum "$ZIP" > dist/SHA256SUMS.txt 2>/dev/null || true
echo "→ $ZIP"

if [ "$DRY" = 1 ]; then
  echo "(--dry-run : rien n'a été publié)"
  exit 0
fi

echo "→ tag + push"
git tag -a "$TAG" -m "OneClick $VERSION"
git push origin "$TAG"

# C'est GitHub qui construit et publie : le tag vient de lancer le workflow
# "release". Pas besoin de créer la release à la main (ça ferait doublon).
echo "Terminé. GitHub construit le .exe maintenant (~3 min) :"
echo "  https://github.com/Nour-yahyaoui/desktop/actions"
echo "  https://github.com/Nour-yahyaoui/desktop/releases/tag/$TAG"
