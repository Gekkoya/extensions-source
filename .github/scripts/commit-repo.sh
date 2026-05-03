#!/bin/bash
set -e

# Configurar git con el bot
git config --global user.email "281276080+gekkoya-bot@users.noreply.github.com"
git config --global user.name "gekkoya-bot"

# Mostrar estado
git status

# Verificar si hay cambios
if [ -n "$(git status --porcelain)" ]; then
    echo "Changes detected, committing..."
    
    # Agregar todos los cambios
    git add .
    
    # Commit
    git commit -m "Update extensions repository

- Updated WASM files
- Updated index.json and metadata
- Updated icons"
    
    # Push al repositorio
    git push
    
    echo "✅ Changes pushed successfully!"
    
    # curl -s "https://purge.jsdelivr.net/gh/Gekkoya/extensions@main/index.min.json" || true
else
    echo "No changes to commit"
fi
