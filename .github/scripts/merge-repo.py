#!/usr/bin/env python3
"""
Script para hacer merge del repositorio local con el remoto.
Elimina extensiones obsoletas y actualiza el índice.
"""

import sys
import json
from pathlib import Path
import shutil

REMOTE_REPO = Path.cwd()
LOCAL_REPO = REMOTE_REPO.parent / "main" / "repo"

# Leer lista de módulos a eliminar
to_delete = json.loads(sys.argv[1]) if len(sys.argv) > 1 else []

print(f"Modules to delete: {to_delete}")

# Eliminar WASMs e iconos obsoletos
for module in to_delete:
    # module format: "es.extension-id"
    lang, ext_id = module.split(".", 1)
    
    # Eliminar WASM
    wasm_pattern = f"{ext_id}.wasm"
    wasm_dir = REMOTE_REPO / "wasm" / lang
    if wasm_dir.exists():
        for wasm_file in wasm_dir.glob(wasm_pattern):
            print(f"Deleting WASM: {wasm_file.relative_to(REMOTE_REPO)}")
            wasm_file.unlink(missing_ok=True)
    
    # Eliminar icono
    icon_pattern = f"{lang}.{ext_id}.png"
    icon_dir = REMOTE_REPO / "icon"
    if icon_dir.exists():
        for icon_file in icon_dir.glob(icon_pattern):
            print(f"Deleting icon: {icon_file.relative_to(REMOTE_REPO)}")
            icon_file.unlink(missing_ok=True)

# Copiar nuevos WASMs e iconos
print("\nCopying new files...")

if (LOCAL_REPO / "wasm").exists():
    shutil.copytree(
        src=LOCAL_REPO / "wasm",
        dst=REMOTE_REPO / "wasm",
        dirs_exist_ok=True
    )
    print("✓ Copied WASM files")

if (LOCAL_REPO / "icon").exists():
    shutil.copytree(
        src=LOCAL_REPO / "icon",
        dst=REMOTE_REPO / "icon",
        dirs_exist_ok=True
    )
    print("✓ Copied icon files")

# Merge de índices
print("\nMerging indexes...")

# Leer índice remoto (si existe)
remote_index_file = REMOTE_REPO / "index.json"
if remote_index_file.exists():
    with open(remote_index_file, "r", encoding="utf-8") as f:
        remote_data = json.load(f)
        remote_index = remote_data.get("extensions", [])
else:
    remote_index = []

# Leer índice local
local_index_file = LOCAL_REPO / "index.json"
if local_index_file.exists():
    with open(local_index_file, "r", encoding="utf-8") as f:
        local_data = json.load(f)
        local_index = local_data.get("extensions", [])
else:
    local_index = []

# Filtrar extensiones obsoletas del índice remoto
index = [
    item for item in remote_index
    if not any([f"{item['lang']}.{item['id']}" == module for module in to_delete])
]

# Agregar nuevas extensiones
index.extend(local_index)

# Ordenar por lang.id
index.sort(key=lambda x: f"{x['lang']}.{x['id']}")

# Escribir index.json
with open(REMOTE_REPO / "index.json", "w", encoding="utf-8") as f:
    json.dump({"version": 1, "extensions": index}, f, ensure_ascii=False, indent=2)

print(f"✓ Created index.json with {len(index)} extensions")

# Escribir index.min.json (minificado)
with open(REMOTE_REPO / "index.min.json", "w", encoding="utf-8") as f:
    json.dump({"version": 1, "extensions": index}, f, ensure_ascii=False, separators=(",", ":"))

print(f"✓ Created index.min.json")

# Generar index.html
print("\nGenerating index.html...")

with open(REMOTE_REPO / "index.html", "w", encoding="utf-8") as f:
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
    f.write('<h1>WASM Extensions Repository</h1>\n')
    f.write(f'<div class="stats">Total extensions: <strong>{len(index)}</strong></div>\n')
    f.write('<input type="text" class="search" id="search" placeholder="Search extensions..." onkeyup="filterTable()">\n')
    f.write('<table id="extensionsTable">\n')
    f.write('<thead><tr><th>Icon</th><th>Name</th><th>Language</th><th>Version</th><th>Size</th><th>Download</th></tr></thead>\n')
    f.write('<tbody>\n')
    
    for entry in index:
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

print("\n✅ Merge completed successfully!")
