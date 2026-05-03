#!/usr/bin/env python3
"""
Script para crear el índice del repositorio de extensiones WASM.
Lee metadata de extension.json y genera index.json con checksums SHA256.
Genera IDs automáticamente usando SHA256 del nombre + lang.
"""

import json
import hashlib
from pathlib import Path
import re

REPO_DIR = Path("repo")
REPO_WASM_DIR = REPO_DIR / "wasm"
REPO_ICON_DIR = REPO_DIR / "icon"


def calculate_sha256(file_path: Path) -> str:
    """Calcula el hash SHA256 de un archivo"""
    sha256_hash = hashlib.sha256()
    with open(file_path, "rb") as f:
        for byte_block in iter(lambda: f.read(4096), b""):
            sha256_hash.update(byte_block)
    return sha256_hash.hexdigest()


def generate_source_id(name: str, lang: str) -> int:
    """
    Genera un ID numérico único basado en name + lang usando SHA256.
    
    Formato: "{name.lowercase()}/{lang}"
    Retorna: Primeros 64 bits del SHA256 como entero positivo
    """
    key = f"{name.lower()}/{lang}"
    hash_bytes = hashlib.sha256(key.encode()).digest()
    
    # Toma los primeros 8 bytes (64 bits)
    id_value = int.from_bytes(hash_bytes[:8], byteorder='big')
    
    # Asegura que sea positivo (bit de signo en 0)
    return id_value & 0x7FFFFFFFFFFFFFFF


def get_cargo_package_name(cargo_toml: Path) -> str:
    """Extrae el nombre del package desde Cargo.toml"""
    with open(cargo_toml, "r", encoding="utf-8") as f:
        for line in f:
            if line.strip().startswith("name"):
                match = re.search(r'name\s*=\s*"([^"]+)"', line)
                if match:
                    return match.group(1)
    return None


def get_cargo_version(cargo_toml: Path) -> str:
    """Extrae la versión desde Cargo.toml"""
    with open(cargo_toml, "r", encoding="utf-8") as f:
        for line in f:
            if line.strip().startswith("version") and "workspace" not in line:
                match = re.search(r'version\s*=\s*"([^"]+)"', line)
                if match:
                    return match.group(1)
    
    # Si usa workspace version, buscar en el Cargo.toml raíz
    root_cargo = Path("Cargo.toml")
    if root_cargo.exists():
        in_workspace_package = False
        with open(root_cargo, "r", encoding="utf-8") as f:
            for line in f:
                if "[workspace.package]" in line:
                    in_workspace_package = True
                elif line.startswith("[") and in_workspace_package:
                    break
                elif in_workspace_package and line.strip().startswith("version"):
                    match = re.search(r'version\s*=\s*"([^"]+)"', line)
                    if match:
                        return match.group(1)
    
    return "0.1.0"


index_data = []

# Procesar cada WASM encontrado
for wasm_file in REPO_WASM_DIR.rglob("*.wasm"):
    wasm_name = wasm_file.stem
    lang = wasm_file.parent.name
    
    print(f"Processing: {wasm_file.relative_to(REPO_DIR)}")
    
    # Buscar la extensión en src/
    extension_found = False
    for lang_dir in Path("src").iterdir():
        if not lang_dir.is_dir() or lang_dir.name != lang:
            continue
        
        for ext_dir in lang_dir.iterdir():
            if not ext_dir.is_dir():
                continue
            
            cargo_toml = ext_dir / "Cargo.toml"
            if not cargo_toml.exists():
                continue
            
            pkg_name = get_cargo_package_name(cargo_toml)
            if pkg_name != wasm_name:
                continue
            
            # Encontramos la extensión
            extension_found = True
            
            # Leer metadata
            metadata_file = ext_dir / "extension.json"
            if not metadata_file.exists():
                print(f"  Warning: No extension.json found for {wasm_name}")
                continue
            
            with open(metadata_file, "r", encoding="utf-8") as f:
                metadata = json.load(f)
            
            # Obtener versión desde Cargo.toml
            version = get_cargo_version(cargo_toml)
            
            # Generar ID automáticamente
            source_id = generate_source_id(metadata.get("name", wasm_name), lang)
            
            # Calcular SHA256 y tamaño
            sha256 = calculate_sha256(wasm_file)
            size = wasm_file.stat().st_size
            
            # Construir URL relativa
            wasm_relative_path = wasm_file.relative_to(REPO_DIR).as_posix()
            
            # Datos para index.json (formato simple y limpio)
            extension_data = {
                "id": f"{lang}.{wasm_name}",
                "sourceId": source_id,
                "name": metadata.get("name", wasm_name),
                "lang": metadata.get("lang", lang),
                "version": version,
                "nsfw": metadata.get("nsfw", False),
                "url": f"https://raw.githubusercontent.com/Gekkoya/extensions/main/{wasm_relative_path}",
                "icon": f"https://raw.githubusercontent.com/Gekkoya/extensions/main/icon/{lang}.{wasm_name}.png",
                "sha256": sha256,
                "size": size,
                "baseUrl": metadata.get("baseUrl", "")
            }
            
            index_data.append(extension_data)
            
            print(f"  ✓ Added: {extension_data['name']} v{extension_data['version']} (ID: {source_id})")
            break
        
        if extension_found:
            break
    
    if not extension_found:
        print(f"  ✗ Warning: Could not find source for {wasm_name}")

# Ordenar por id
index_data.sort(key=lambda x: x['id'])

# Escribir index.json
with REPO_DIR.joinpath("index.json").open("w", encoding="utf-8") as f:
    json.dump({"version": 1, "extensions": index_data}, f, ensure_ascii=False, indent=2)

print(f"\n✓ Created index.json with {len(index_data)} extensions")

# Escribir index.min.json (minificado para producción)
with REPO_DIR.joinpath("index.min.json").open("w", encoding="utf-8") as f:
    json.dump({"version": 1, "extensions": index_data}, f, ensure_ascii=False, separators=(",", ":"))

print(f"✓ Created index.min.json")


print("\nGenerating index.html...")

with open(REPO_DIR / "index.html", "w", encoding="utf-8") as f:
    f.write('<!DOCTYPE html>\n<html>\n<head>\n')
    f.write('<meta charset="UTF-8">\n')
    f.write('<meta name="viewport" content="width=device-width, initial-scale=1.0">\n')
    f.write('<title>WASM Extensions Repository</title>\n')
    f.write('<style>\n')
    f.write('* { margin: 0; padding: 0; box-sizing: border-box; }\n')
    f.write('body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #f5f5f5; padding: 20px; }\n')
    f.write('.container { max-width: 1200px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 8px rgba(0,0,0,0.1); }\n')
    f.write('h1 { color: #333; margin-bottom: 10px; }\n')
    f.write('.stats { color: #666; margin-bottom: 30px; font-size: 14px; }\n')
    f.write('.search { width: 100%; padding: 12px; margin-bottom: 20px; border: 2px solid #ddd; border-radius: 6px; font-size: 16px; }\n')
    f.write('.search:focus { outline: none; border-color: #4CAF50; }\n')
    f.write('table { width: 100%; border-collapse: collapse; }\n')
    f.write('th, td { padding: 12px; text-align: left; border-bottom: 1px solid #eee; }\n')
    f.write('th { background: #4CAF50; color: white; font-weight: 600; position: sticky; top: 0; }\n')
    f.write('tr:hover { background: #f9f9f9; }\n')
    f.write('.icon { width: 40px; height: 40px; border-radius: 6px; object-fit: cover; }\n')
    f.write('.badge { display: inline-block; padding: 4px 8px; border-radius: 4px; font-size: 12px; font-weight: 600; }\n')
    f.write('.badge-nsfw { background: #ff4444; color: white; }\n')
    f.write('.badge-lang { background: #2196F3; color: white; }\n')
    f.write('a { color: #4CAF50; text-decoration: none; font-weight: 500; }\n')
    f.write('a:hover { text-decoration: underline; }\n')
    f.write('.version { color: #666; font-family: monospace; }\n')
    f.write('</style>\n')
    f.write('</head>\n<body>\n')
    f.write('<div class="container">\n')
    f.write('<h1>🚀 WASM Extensions Repository</h1>\n')
    f.write(f'<div class="stats">Total extensions: <strong>{len(index_data)}</strong></div>\n')
    f.write('<input type="text" class="search" id="search" placeholder="Search extensions..." onkeyup="filterTable()">\n')
    f.write('<table id="extensionsTable">\n')
    f.write('<thead><tr><th>Icon</th><th>Name</th><th>Language</th><th>Version</th><th>Size</th><th>Download</th></tr></thead>\n')
    f.write('<tbody>\n')
    
    for entry in index_data:
        name_escaped = entry["name"].replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
        lang_escaped = entry["lang"]
        version_escaped = entry["version"]
        size_kb = entry["size"] / 1024
        
        icon_url = f"icon/{entry['lang']}.{entry['id'].split('.')[1]}.png"
        wasm_url = f"wasm/{entry['lang']}/{entry['id'].split('.')[1]}.wasm"
        
        nsfw_badge = '<span class="badge badge-nsfw">NSFW</span> ' if entry.get("nsfw") else ''
        
        f.write(f'<tr>')
        f.write(f'<td><img src="{icon_url}" class="icon" alt="{name_escaped}" onerror="this.style.display=\'none\'"></td>')
        f.write(f'<td><strong>{name_escaped}</strong> {nsfw_badge}</td>')
        f.write(f'<td><span class="badge badge-lang">{lang_escaped}</span></td>')
        f.write(f'<td class="version">{version_escaped}</td>')
        f.write(f'<td>{size_kb:.1f} KB</td>')
        f.write(f'<td><a href="{wasm_url}" download>⬇️ Download</a></td>')
        f.write(f'</tr>\n')
    
    f.write('</tbody>\n</table>\n')
    f.write('<script>\n')
    f.write('function filterTable() {\n')
    f.write('  const input = document.getElementById("search");\n')
    f.write('  const filter = input.value.toUpperCase();\n')
    f.write('  const table = document.getElementById("extensionsTable");\n')
    f.write('  const tr = table.getElementsByTagName("tr");\n')
    f.write('  for (let i = 1; i < tr.length; i++) {\n')
    f.write('    const td = tr[i].getElementsByTagName("td")[1];\n')
    f.write('    if (td) {\n')
    f.write('      const txtValue = td.textContent || td.innerText;\n')
    f.write('      tr[i].style.display = txtValue.toUpperCase().indexOf(filter) > -1 ? "" : "none";\n')
    f.write('    }\n')
    f.write('  }\n')
    f.write('}\n')
    f.write('</script>\n')
    f.write('</div>\n</body>\n</html>\n')

print(f"✓ Created index.html")

print("\n✅ Repository index created successfully!")
