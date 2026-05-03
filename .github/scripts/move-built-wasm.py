#!/usr/bin/env python3
"""
Script para mover los archivos WASM compilados a la estructura del repositorio.
Organiza los WASMs por idioma: wasm/es/extension-id.wasm
"""

from pathlib import Path
import shutil
import json

REPO_WASM_DIR = Path("repo/wasm")
REPO_ICON_DIR = Path("repo/icon")

# Limpiar directorios anteriores
shutil.rmtree(REPO_WASM_DIR, ignore_errors=True)
shutil.rmtree(REPO_ICON_DIR, ignore_errors=True)

REPO_WASM_DIR.mkdir(parents=True, exist_ok=True)
REPO_ICON_DIR.mkdir(parents=True, exist_ok=True)

# Buscar todos los WASMs compilados
artifacts_dir = Path.home() / "wasm-artifacts"

if not artifacts_dir.exists():
    print("No WASM artifacts found")
    exit(0)

for wasm in artifacts_dir.glob("**/*.wasm"):
    wasm_name = wasm.stem  # nombre sin extensión
    
    # Buscar la extensión correspondiente en src/
    found = False
    for lang_dir in Path("src").iterdir():
        if not lang_dir.is_dir():
            continue
        
        for ext_dir in lang_dir.iterdir():
            if not ext_dir.is_dir():
                continue
            
            # Verificar si el nombre del package coincide
            cargo_toml = ext_dir / "Cargo.toml"
            if cargo_toml.exists():
                with open(cargo_toml, "r", encoding="utf-8") as f:
                    content = f.read()
                    if f'name = "{wasm_name}"' in content:
                        # Encontramos la extensión
                        lang = lang_dir.name
                        
                        # Crear directorio por idioma
                        lang_wasm_dir = REPO_WASM_DIR / lang
                        lang_wasm_dir.mkdir(exist_ok=True)
                        
                        # Mover WASM
                        dest_wasm = lang_wasm_dir / f"{wasm_name}.wasm"
                        shutil.copy2(wasm, dest_wasm)
                        print(f"Moved: {wasm.name} -> {dest_wasm.relative_to(Path('repo'))}")
                        
                        # Copiar icono si existe
                        icon_path = ext_dir / "icon.png"
                        if icon_path.exists():
                            icon_dest = REPO_ICON_DIR / f"{lang}.{wasm_name}.png"
                            shutil.copy2(icon_path, icon_dest)
                            print(f"Copied icon: {icon_path} -> {icon_dest.relative_to(Path('repo'))}")
                        
                        found = True
                        break
        
        if found:
            break
    
    if not found:
        print(f"Warning: Could not find source for {wasm_name}.wasm")

print(f"\nTotal WASMs moved: {len(list(REPO_WASM_DIR.rglob('*.wasm')))}")
print(f"Total icons copied: {len(list(REPO_ICON_DIR.glob('*.png')))}")
